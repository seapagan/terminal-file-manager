use super::Severity;
use crate::operations::{Job, JobState, Operation, Outcome};

/// Concise display copy is separate from the retained diagnostic text.
pub struct Presentation {
    pub title: String,
    pub message: String,
}

impl Presentation {
    pub(super) fn application(text: &str, severity: Severity) -> Self {
        let summary = text
            .strip_prefix("Pane 1: ")
            .or_else(|| text.strip_prefix("Pane 2: "))
            .unwrap_or(text);
        Self {
            title: severity.label().into(),
            message: readable(summary),
        }
    }

    pub(super) fn job(job: &Job) -> Self {
        let action = action(job.plan.operation);
        let title = match job.state {
            JobState::Completed => format!("{action} complete"),
            JobState::Critical => "CRITICAL safety incident".into(),
            JobState::Unknown => "Outcome unknown".into(),
            JobState::Partial => format!("{action} partially complete"),
            JobState::Cancelled => format!("{action} cancelled"),
            JobState::Skipped => format!("{action} skipped"),
            _ => format!("{action} failed"),
        };
        let name = target(job);
        let message = match job.state {
            JobState::Completed => name,
            JobState::Critical => {
                "OS reported permanent deletion during Trash. Stop and report.".into()
            }
            JobState::Unknown => {
                "Inspect Jobs evidence. Safe exit and retry remain blocked.".into()
            }
            _ => failure(job, &name),
        };
        Self { title, message }
    }
}

fn action(operation: Operation) -> &'static str {
    match operation {
        Operation::Copy => "Copy",
        Operation::Move => "Move",
        Operation::Mkdir => "Create directory",
        Operation::CreateFile => "Create file",
        Operation::Rename => "Rename",
        Operation::Trash => "Trash",
        Operation::Delete => "Permanent delete",
        Operation::Permissions(_) | Operation::ReadOnly(_) => "Update permissions",
    }
}

fn target(job: &Job) -> String {
    if job.plan.operands.len() > 1 {
        return format!("{} selected entries", job.plan.operands.len());
    }
    let path = if job.plan.operation.creates_entry() {
        job.plan.destination.as_deref()
    } else {
        job.plan.operands.first().map(|entry| entry.path.as_path())
    };
    path.map_or_else(String::new, display_name)
}

fn display_name(path: &std::path::Path) -> String {
    format!(
        "“{}”",
        path.file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
    )
}

fn failure(job: &Job, name: &str) -> String {
    let failed = job.record.as_ref().and_then(|record| {
        record
            .entries
            .iter()
            .find(|entry| !matches!(entry.outcome, Outcome::Completed | Outcome::Skipped))
    });
    let detail = failed.map_or("Inspect Jobs for the result.", |entry| {
        entry.detail.as_str()
    });
    let name = failed
        .filter(|_| !job.plan.operation.creates_entry())
        .map_or_else(|| name.to_owned(), |entry| display_name(&entry.source));
    let directory = failed.is_some_and(|entry| entry.kind == crate::fs::EntryKind::Directory);
    if let Some(message) = overlap_message(job.plan.operation, &name, detail, directory) {
        return message;
    }
    let reason = readable(detail);
    if reason == "Source and destination are the same." {
        format!("{name}: choose a different destination.")
    } else if reason == "An entry already exists." {
        format!("{name} already exists. Choose a different name.")
    } else if reason == "Permission denied." {
        format!("Permission denied for {name}.")
    } else {
        format!("{name}: {reason}")
    }
}

fn overlap_message(
    operation: Operation,
    name: &str,
    detail: &str,
    directory: bool,
) -> Option<String> {
    let verb = match operation {
        Operation::Copy => "copy",
        Operation::Move => "move",
        _ => return None,
    };
    let relation = match detail {
        "Source and destination are the same."
        | "Source and destination are the same folder."
        | "Source and destination refer to the same item." => "onto itself",
        "Destination is inside the source folder." => "into one of its descendants",
        _ => return None,
    };
    let kind = if directory { "directory " } else { "" };
    Some(format!("Cannot {verb} {kind}{name} {relation}."))
}

fn readable(text: &str) -> String {
    if text.starts_with("Source and destination ") {
        return "Source and destination are the same.".into();
    }
    if text.starts_with("Permission denied") || text.starts_with("Access is denied") {
        return "Permission denied.".into();
    }
    if [
        "File exists",
        "The file exists",
        "Is a directory",
        "Cannot create a file when that file already exists",
        "Rename destination exists",
    ]
    .iter()
    .any(|prefix| text.starts_with(prefix))
    {
        return "An entry already exists.".into();
    }
    text.lines()
        .map(|line| {
            let line = line.split(" (os error ").next().unwrap_or(line);
            line.split_whitespace()
                .map(|word| {
                    if word.starts_with('/') || word.as_bytes().get(1..3) == Some(b":\\") {
                        word.rsplit(['/', '\\']).next().unwrap_or(word)
                    } else {
                        word
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn job_key(job: &Job) -> String {
    if job.state != JobState::Failed {
        return format!("protected job {}", job.id);
    }
    let entries = job.record.as_ref().map(|record| {
        record
            .entries
            .iter()
            .map(|entry| {
                (
                    &entry.source,
                    &entry.destination,
                    entry.outcome,
                    &entry.detail,
                )
            })
            .collect::<Vec<_>>()
    });
    format!(
        "{:?}:{:?}:{:?}:{:?}:{entries:?}",
        job.plan.operation, job.state, job.plan.destination, job.plan.incoming_name
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_and_windows_collision_text_share_plain_language() {
        for detail in [
            "File exists (os error 17)",
            "The file exists. (os error 80)",
            "Is a directory (os error 21)",
            "Cannot create a file when that file already exists. (os error 183)",
        ] {
            assert_eq!(readable(detail), "An entry already exists.");
        }
    }
}
