//! Existing status messages retain shortcut intent separately from untrusted prose.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Status {
    Welcome,
    Text(String),
    JobsDetails(String),
}

impl From<String> for Status {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}
impl From<&str> for Status {
    fn from(text: &str) -> Self {
        Self::Text(text.into())
    }
}
impl std::ops::Deref for Status {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            Self::Welcome => super::WELCOME,
            Self::Text(text) | Self::JobsDetails(text) => text,
        }
    }
}
impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self)
    }
}
