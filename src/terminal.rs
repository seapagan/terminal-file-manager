//! Terminal lifecycle adapted from Labeldeck's UI guard; matched protocol cleanup.
use crossterm::{
    cursor::{Hide, Show},
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        supports_keyboard_enhancement,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io,
    panic::{self, AssertUnwindSafe, PanicHookInfo},
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};

/// Real terminal type; deterministic rendering tests use TestBackend separately.
pub type NativeTerminal = Terminal<CrosstermBackend<io::Stdout>>;
type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;
const RAW: u8 = 1;
const ALTERNATE: u8 = 2;
const MOUSE: u8 = 4;
const PASTE: u8 = 8;
const KEYBOARD: u8 = 16;

#[derive(Default)]
struct Modes(AtomicU8);

impl Modes {
    fn attempting(&self, mode: u8) {
        self.0.fetch_or(mode, Ordering::Relaxed);
    }

    fn enter(&self, mouse: bool) -> io::Result<NativeTerminal> {
        self.attempting(RAW);
        enable_raw_mode()?;
        self.attempting(ALTERNATE);
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        self.attempting(PASTE);
        execute!(io::stdout(), EnableBracketedPaste)?;
        if mouse {
            self.attempting(MOUSE);
            execute!(io::stdout(), EnableMouseCapture)?;
        }
        if supports_keyboard_enhancement().unwrap_or(false) {
            self.attempting(KEYBOARD);
            execute!(
                io::stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }
        Terminal::new(CrosstermBackend::new(io::stdout()))
    }

    fn restore(&self) -> io::Result<()> {
        let modes = self.0.swap(0, Ordering::Relaxed);
        let mut error = None;
        if modes & KEYBOARD != 0 {
            remember(
                &mut error,
                execute!(io::stdout(), PopKeyboardEnhancementFlags),
            );
        }
        if modes & MOUSE != 0 {
            remember(&mut error, execute!(io::stdout(), DisableMouseCapture));
        }
        if modes & PASTE != 0 {
            remember(&mut error, execute!(io::stdout(), DisableBracketedPaste));
        }
        if modes & ALTERNATE != 0 {
            remember(
                &mut error,
                execute!(io::stdout(), Show, LeaveAlternateScreen),
            );
        }
        if modes & RAW != 0 {
            remember(&mut error, disable_raw_mode());
        }
        error.map_or(Ok(()), Err)
    }
}

impl Drop for Modes {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

struct PanicHookGuard {
    previous: Option<Arc<Hook>>,
}

impl PanicHookGuard {
    fn install(modes: Arc<Modes>) -> Self {
        let previous = Arc::new(panic::take_hook());
        let hook = previous.clone();
        panic::set_hook(Box::new(move |info| {
            let _ = modes.restore();
            hook(info);
        }));
        Self {
            previous: Some(previous),
        }
    }
}

impl Drop for PanicHookGuard {
    fn drop(&mut self) {
        drop(panic::take_hook());
        if let Some(previous) = self.previous.take() {
            let hook =
                Arc::try_unwrap(previous).unwrap_or_else(|hook| Box::new(move |info| hook(info)));
            panic::set_hook(hook);
        }
    }
}

/// Restore every attempted mode on normal exit, I/O error, partial setup or unwind.
///
/// # Errors
/// Returns setup, application or cleanup I/O errors. Panics resume after cleanup.
pub fn with_terminal<T>(
    mouse: bool,
    operation: impl FnOnce(&mut NativeTerminal) -> io::Result<T>,
) -> io::Result<T> {
    let modes = Arc::new(Modes::default());
    let hook = PanicHookGuard::install(modes.clone());
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        let mut terminal = modes.enter(mouse)?;
        operation(&mut terminal)
    }));
    let cleanup = modes.restore();
    drop(hook);
    match result {
        Ok(result) => result.and_then(|value| cleanup.map(|()| value)),
        Err(payload) => panic::resume_unwind(payload),
    }
}

fn remember(first: &mut Option<io::Error>, result: io::Result<()>) {
    if first.is_none()
        && let Err(error) = result
    {
        *first = Some(error);
    }
}
