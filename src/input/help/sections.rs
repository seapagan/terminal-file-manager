/// Ordered categories in the continuous Help document.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HelpSection {
    Navigation,
    Selection,
    Operations,
    Jobs,
    Panes,
    Views,
    Application,
}

impl HelpSection {
    pub const ALL: [Self; 7] = [
        Self::Navigation,
        Self::Selection,
        Self::Operations,
        Self::Jobs,
        Self::Panes,
        Self::Views,
        Self::Application,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::Navigation => "Navigation",
            Self::Selection => "Selection & Marking",
            Self::Operations => "File Operations",
            Self::Jobs => "Jobs & Progress",
            Self::Panes => "Panes & Layout",
            Self::Views => "Views & Metadata",
            Self::Application => "Application & Diagnostics",
        }
    }
}
