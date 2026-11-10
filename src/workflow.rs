//! Clock-free staged footer commands, independent of consumer context and effects.
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// A measured mouse affordance retains semantic identity rather than fabricating a key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Choice(&'static str),
    Back,
    Cancel,
}

/// Chronological physical input, including discarded stage selections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Input {
    Key(KeyEvent),
    Click(Target),
}

/// Consumer-defined semantic option and responsive labels (wide, medium, narrow).
#[derive(Clone, Debug)]
pub struct Choice {
    pub id: &'static str,
    pub key: char,
    pub labels: [String; 3],
    pub disabled: Option<String>,
}

impl Choice {
    pub fn new(id: &'static str, key: char, labels: [&str; 3]) -> Self {
        Self {
            id,
            key,
            labels: labels.map(str::to_owned),
            disabled: None,
        }
    }

    pub fn key_label(&self) -> String {
        crate::input::key_name(KeyCode::Char(self.key), KeyModifiers::NONE)
    }

    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.disabled = Some(reason.into());
        self
    }
}

type Condition = Box<dyn Fn(&[Resolved]) -> bool>;

/// Ordered stage with a semantic default; fallback is the first enabled option.
pub struct Stage {
    pub label: String,
    pub choices: Vec<Choice>,
    selected: Option<usize>,
    condition: Option<Condition>,
}

impl Stage {
    pub fn new(label: impl Into<String>, choices: Vec<Choice>, default: &str) -> Self {
        let selected = choices.iter().position(|c| c.id == default);
        let mut stage = Self {
            label: label.into(),
            choices,
            selected,
            condition: None,
        };
        stage.select_enabled();
        stage
    }

    fn select_enabled(&mut self) -> bool {
        self.selected = self
            .selected
            .filter(|i| self.choices.get(*i).is_some_and(|c| c.disabled.is_none()))
            .or_else(|| self.choices.iter().position(|c| c.disabled.is_none()));
        self.selected.is_some()
    }

    /// Consumer-owned eligibility evaluated against the accepted semantic path.
    pub fn when(mut self, condition: impl Fn(&[Resolved]) -> bool + 'static) -> Self {
        self.condition = Some(Box::new(condition));
        self
    }

    fn available(&self, choices: &[Resolved]) -> bool {
        self.condition
            .as_ref()
            .is_none_or(|condition| condition(choices))
    }
}

/// Final effective path exposes semantic IDs alongside effective choice keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolved {
    pub id: &'static str,
    pub key: Option<char>,
}

/// Owned callback data cannot outlive a cancelled flow or execute twice.
pub struct Completion<C> {
    pub context: C,
    pub inputs: Vec<Input>,
    pub choices: Vec<Resolved>,
}

/// Transition outcome; the caller owns normal application status presentation.
pub enum Progress<O> {
    Active,
    Disabled(String),
    Cancelled,
    Complete(O),
    Inactive,
}

type Handler<C, O> = Box<dyn FnOnce(Completion<C>) -> O>;

/// One isolated flow with opaque consumer context and a one-shot completion handler.
pub struct Flow<C, O> {
    pub label: String,
    stages: Vec<Stage>,
    stage: usize,
    history: Vec<usize>,
    inputs: Vec<Input>,
    choices: Vec<Resolved>,
    context: Option<C>,
    handler: Option<Handler<C, O>>,
}

impl<C, O> Flow<C, O> {
    /// Reject malformed definitions before activation.
    pub fn new(
        action: &'static str,
        label: impl Into<String>,
        trigger: Input,
        context: C,
        mut stages: Vec<Stage>,
        handler: impl FnOnce(Completion<C>) -> O + 'static,
    ) -> Result<Self, &'static str> {
        if stages.is_empty() || stages.iter().any(|s| !valid_choices(&s.choices)) {
            return Err("Workflow requires stages with unique semantic IDs and letter keys");
        }
        let trigger_key = match &trigger {
            Input::Key(k) => match k.code {
                KeyCode::Char(c) => Some(c),
                _ => None,
            },
            _ => None,
        };
        let choices = vec![Resolved {
            id: action,
            key: trigger_key,
        }];
        let stage = stages
            .iter()
            .position(|s| s.available(&choices))
            .ok_or("Workflow requires an available starting stage")?;
        if !stages[stage].select_enabled() {
            return Err("Workflow starting stage has no enabled choices");
        }
        Ok(Self {
            label: label.into(),
            stages,
            stage,
            history: Vec::new(),
            inputs: vec![trigger],
            choices,
            context: Some(context),
            handler: Some(Box::new(handler)),
        })
    }

    pub fn stage(&self) -> &Stage {
        &self.stages[self.stage]
    }
    pub fn stage_index(&self) -> usize {
        self.stage
    }
    pub fn can_back(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn context(&self) -> Option<&C> {
        self.context.as_ref()
    }
    pub fn highlighted(&self) -> Option<&Choice> {
        self.stage()
            .selected
            .and_then(|i| self.stage().choices.get(i))
    }

    /// Every event is consumed by the caller while active, even when unsupported.
    pub fn input(&mut self, input: Input) -> Progress<O> {
        if self.handler.is_none() {
            return Progress::Inactive;
        }
        self.inputs.push(input.clone());
        let target = match input {
            Input::Click(target) => Some(target),
            Input::Key(key) => self.key_target(key),
        };
        match target {
            Some(Target::Cancel) => self.cancel(),
            Some(Target::Back) if !self.can_back() => self.cancel(),
            Some(Target::Back) => {
                self.stage = self.history.pop().expect("previous stage");
                self.choices.truncate(self.history.len() + 1);
                Progress::Active
            }
            Some(Target::Choice(id)) => self.choose(id),
            None => Progress::Active,
        }
    }

    fn cancel(&mut self) -> Progress<O> {
        self.handler = None;
        self.context = None;
        Progress::Cancelled
    }

    fn key_target(&mut self, key: KeyEvent) -> Option<Target> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        let (code, modifiers) = crate::input::normalize_chord(key.code, key.modifiers);
        if !modifiers.is_empty() {
            return None;
        }
        self.unmodified_key(code)
    }

    fn unmodified_key(&mut self, code: KeyCode) -> Option<Target> {
        match code {
            KeyCode::Esc => Some(Target::Cancel),
            KeyCode::Backspace => Some(Target::Back),
            KeyCode::Enter => self.highlighted().map(|c| Target::Choice(c.id)),
            KeyCode::Char(key) => self
                .stage()
                .choices
                .iter()
                .find(|c| c.key == key)
                .map(|c| Target::Choice(c.id)),
            _ => None,
        }
    }

    fn choose(&mut self, id: &'static str) -> Progress<O> {
        let stage = &mut self.stages[self.stage];
        let Some(index) = stage.choices.iter().position(|c| c.id == id) else {
            return Progress::Active;
        };
        let choice = &stage.choices[index];
        if let Some(reason) = &choice.disabled {
            return Progress::Disabled(reason.clone());
        }
        stage.selected = Some(index);
        self.choices.push(Resolved {
            id,
            key: Some(choice.key),
        });
        if let Some(next) =
            (self.stage + 1..self.stages.len()).find(|i| self.stages[*i].available(&self.choices))
        {
            if !self.stages[next].select_enabled() {
                self.choices.pop();
                return Progress::Disabled(format!(
                    "No enabled choices in {}",
                    self.stages[next].label
                ));
            }
            self.history.push(self.stage);
            self.stage = next;
            return Progress::Active;
        }
        let handler = self.handler.take().expect("active handler");
        Progress::Complete(handler(Completion {
            context: self.context.take().expect("active context"),
            inputs: std::mem::take(&mut self.inputs),
            choices: std::mem::take(&mut self.choices),
        }))
    }
}

fn valid_choices(choices: &[Choice]) -> bool {
    !choices.is_empty()
        && choices.iter().enumerate().all(|(i, c)| {
            c.key.is_ascii_alphabetic()
                && !choices[..i]
                    .iter()
                    .any(|old| old.id == c.id || old.key == c.key)
        })
}
