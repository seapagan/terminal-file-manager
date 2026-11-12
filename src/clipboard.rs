//! Clipboard provider routing adapted from seapagan/git-diff-out.
use std::{env, error::Error, ffi::OsStr, fmt};

const OSC52_MAX_BYTES: usize = 74_991;
pub(crate) const MAX_BYTES: usize = 16 * 1024 * 1024;

fn validate_payload(payload: &[u8]) -> Result<(), ClipboardError> {
    if payload.is_empty() || payload.len() > MAX_BYTES {
        return Err(ClipboardError(
            "Clipboard text must be nonempty and at most 16 MiB".into(),
        ));
    }
    let text = std::str::from_utf8(payload)
        .map_err(|e| ClipboardError(format!("Clipboard text is not valid UTF-8: {e}")))?;
    if text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(ClipboardError(
            "Clipboard text contains a control character".into(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum TerminalKind {
    Plain,
    Tmux,
    Zellij,
    Unsupported,
}

fn terminal_kind() -> TerminalKind {
    if env_nonempty("ZELLIJ") {
        TerminalKind::Zellij
    } else if env_nonempty("TMUX") {
        TerminalKind::Tmux
    } else if env_nonempty("STY")
        || env::var("TERM").map_or(true, |term| term.is_empty() || term == "dumb")
    {
        TerminalKind::Unsupported
    } else {
        TerminalKind::Plain
    }
}

fn terminal_sequence(payload: &[u8], terminal: &TerminalKind) -> Result<Vec<u8>, ClipboardError> {
    match terminal {
        TerminalKind::Unsupported => Err(ClipboardError("OSC 52 unavailable in this terminal; use a supported terminal or local clipboard provider".into())),
        // Zellij handles standard OSC52; tmux requires set-clipboard on and terminal support.
        TerminalKind::Plain | TerminalKind::Tmux | TerminalKind::Zellij => encode_osc52(payload),
    }
}
#[cfg(unix)]
mod command;
#[cfg(unix)]
use command::run_provider;
#[cfg(not(unix))]
fn run_provider(
    _name: &str,
    _program: &str,
    _args: &[&str],
    _payload: &[u8],
) -> Result<(), ClipboardError> {
    Err(ClipboardError(
        "External clipboard providers are unavailable on this platform; use the native backend"
            .into(),
    ))
}
pub mod runtime;

type BackendWriter<'a> = dyn FnMut(Backend, &[u8]) -> Result<(), ClipboardError> + 'a;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Backend {
    Osc52,
    WlCopy,
    Xclip,
    Xsel,
    Pbcopy,
    Windows,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Platform {
    Linux,
    Macos,
    Windows,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Selection {
    pub platform: Platform,
    pub ssh: bool,
    pub wayland: bool,
    pub x11: bool,
    pub wl_copy: bool,
    pub xclip: bool,
    pub xsel: bool,
    pub pbcopy: bool,
    pub osc52_fallback: bool,
}

#[derive(Debug)]
pub struct ClipboardError(pub(crate) String);

impl fmt::Display for ClipboardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for ClipboardError {}

pub(crate) fn select_backend(selection: &Selection) -> Result<Backend, ClipboardError> {
    if selection.ssh {
        return Ok(Backend::Osc52);
    }
    if let Some(backend) = local_backend(selection) {
        return Ok(backend);
    }
    if selection.osc52_fallback {
        return Ok(Backend::Osc52);
    }
    Err(ClipboardError(match selection.platform {
        Platform::Linux => "no supported local clipboard provider is available; install wl-clipboard for Wayland or xclip/xsel for X11".into(),
        Platform::Macos => "the macOS clipboard provider 'pbcopy' is not available".into(),
        Platform::Windows => unreachable!("Windows always has a native backend"),
    }))
}

fn local_backend(selection: &Selection) -> Option<Backend> {
    match selection.platform {
        Platform::Linux => linux_backend(selection),
        Platform::Macos => selection.pbcopy.then_some(Backend::Pbcopy),
        Platform::Windows => Some(Backend::Windows),
    }
}

fn linux_backend(selection: &Selection) -> Option<Backend> {
    if selection.wayland && selection.wl_copy {
        Some(Backend::WlCopy)
    } else if selection.x11 && selection.xclip {
        Some(Backend::Xclip)
    } else if selection.x11 && selection.xsel {
        Some(Backend::Xsel)
    } else {
        None
    }
}

pub(crate) fn encode_osc52(payload: &[u8]) -> Result<Vec<u8>, ClipboardError> {
    if payload.len() > OSC52_MAX_BYTES {
        return Err(ClipboardError(format!(
            "OSC 52 payload is too large ({} bytes; maximum is {OSC52_MAX_BYTES} bytes); clipboard content was not sent",
            payload.len()
        )));
    }

    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = Vec::with_capacity(payload.len().div_ceil(3) * 4 + 9);
    encoded.extend_from_slice(b"\x1b]52;c;");
    for chunk in payload.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        encoded.push(TABLE[((value >> 18) & 63) as usize]);
        encoded.push(TABLE[((value >> 12) & 63) as usize]);
        encoded.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize]
        } else {
            b'='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize]
        } else {
            b'='
        });
    }
    encoded.extend_from_slice(b"\x1b\\");
    Ok(encoded)
}

#[cfg(any(windows, test))]
fn clipboard_text(payload: &[u8]) -> Result<&str, ClipboardError> {
    let text = std::str::from_utf8(payload).map_err(|error| {
        ClipboardError(format!("clipboard content is not valid UTF-8: {error}"))
    })?;
    if text.contains('\0') {
        return Err(ClipboardError(
            "clipboard content contains an embedded NUL character".into(),
        ));
    }
    Ok(text)
}

fn provider_spec(backend: Backend) -> (&'static str, &'static [&'static str]) {
    match backend {
        Backend::WlCopy => ("wl-copy", &[]),
        Backend::Xclip => ("xclip", &["-selection", "clipboard", "-in"]),
        Backend::Xsel => ("xsel", &["--clipboard", "--input"]),
        Backend::Pbcopy => ("pbcopy", &[]),
        Backend::Osc52 | Backend::Windows => unreachable!("not a command provider"),
    }
}

/// A local provider confirmed its call, or a terminal sequence awaits serial emission.
#[derive(Debug)]
pub enum Delivery {
    Copied,
    Terminal(Vec<u8>),
}

pub(crate) fn copy(payload: &[u8], osc52_fallback: bool) -> Result<Delivery, ClipboardError> {
    validate_payload(payload)?;
    let selection = system_selection(osc52_fallback);
    let backend = select_backend(&selection)?;
    let mut sequence = None;
    let backend = copy_with_fallback(backend, payload, osc52_fallback, &mut |backend, payload| {
        if backend == Backend::Osc52 {
            sequence = Some(terminal_sequence(payload, &terminal_kind())?);
            Ok(())
        } else {
            copy_with_backend(backend, payload)
        }
    })?;
    Ok(if backend == Backend::Osc52 {
        Delivery::Terminal(sequence.expect("encoded terminal sequence"))
    } else {
        Delivery::Copied
    })
}

fn copy_with_fallback(
    backend: Backend,
    payload: &[u8],
    osc52_fallback: bool,
    copy_backend: &mut BackendWriter<'_>,
) -> Result<Backend, ClipboardError> {
    match copy_backend(backend, payload) {
        Err(provider_error) if osc52_fallback && backend != Backend::Osc52 => {
            copy_backend(Backend::Osc52, payload)
                .map(|()| Backend::Osc52)
                .map_err(|fallback_error| {
                    ClipboardError(format!(
                        "{provider_error}; OSC 52 fallback failed: {fallback_error}"
                    ))
                })
        }
        result => result.map(|()| backend),
    }
}

fn copy_with_backend(backend: Backend, payload: &[u8]) -> Result<(), ClipboardError> {
    match backend {
        Backend::Osc52 => unreachable!("OSC 52 is encoded for the terminal owner"),
        Backend::WlCopy | Backend::Xclip | Backend::Xsel | Backend::Pbcopy => {
            let (program, args) = provider_spec(backend);
            run_provider(program, program, args, payload)
        }
        Backend::Windows => copy_windows(payload),
    }
}

fn system_selection(osc52_fallback: bool) -> Selection {
    Selection {
        platform: current_platform(),
        ssh: ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
            .iter()
            .any(|name| env_nonempty(name)),
        wayland: env_nonempty("WAYLAND_DISPLAY")
            || env::var("XDG_SESSION_TYPE")
                .is_ok_and(|value| value.eq_ignore_ascii_case("wayland")),
        x11: env_nonempty("DISPLAY"),
        wl_copy: program_exists("wl-copy"),
        xclip: program_exists("xclip"),
        xsel: program_exists("xsel"),
        pbcopy: program_exists("pbcopy"),
        osc52_fallback,
    }
}

fn env_nonempty(name: &str) -> bool {
    env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn program_exists(program: impl AsRef<OsStr>) -> bool {
    let path = env::var_os("PATH");
    program_exists_in(path.as_deref(), program.as_ref())
}

fn program_exists_in(path: Option<&OsStr>, program: &OsStr) -> bool {
    let Some(path) = path else {
        return false;
    };
    env::split_paths(path).any(|directory| {
        let candidate = directory.join(program);
        if !candidate.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            candidate
                .metadata()
                .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
        }
        #[cfg(not(unix))]
        {
            true
        }
    })
}

fn current_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

#[cfg(not(windows))]
fn copy_windows(_payload: &[u8]) -> Result<(), ClipboardError> {
    unreachable!("Windows backend is only selected on Windows")
}

#[cfg(windows)]
fn copy_windows(payload: &[u8]) -> Result<(), ClipboardError> {
    let text = clipboard_text(payload)?;
    clipboard_win::set_clipboard_string(text)
        .map_err(|error| ClipboardError(format!("cannot set the Windows clipboard: {error}")))
}

#[cfg(test)]
mod fallback_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod tests;
