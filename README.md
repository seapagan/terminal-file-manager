# Terminal File Manager

`tfm` is a keyboard-first terminal file manager for Linux, macOS and Windows.
Browse in one or two panes, copy and move files, rename entries, filter and mark
selections, and inspect file information without leaving the terminal.

SSH/SFTP, file viewing and editing are planned. Enter opens directories; it does
not open or execute files.

## Installation

There is no public release yet. Build and install from source with Rust 1.95.0
or newer:

```sh
git clone https://github.com/seapagan/terminal-file-manager.git
cd terminal-file-manager
cargo install --path . --locked
```

The installed command is `tfm`. To try it without installing, use
`cargo run --locked` and put CLI arguments after `--`.

## Getting started

```sh
tfm                              # Browse the current directory
tfm /path/to/directory           # Browse a chosen directory
tfm -2                           # Start with two panes
tfm /path/to/left /path/to/right # Start with two different locations
tfm --help                       # See command-line options
```

A single directory argument sets both panes. Each pane keeps its own location
and view settings; narrow terminals show the active pane.

Click to focus a pane, double-click a folder to enter it and use the wheel to
scroll. Click a path component to jump to an ancestor, or use Alt+l to open
**Go to directory**.

Press `?` or F1 for searchable Help, including instructions for the current
dialog. Use F1 while editing a text field, where `?` stays literal.

## Keyboard shortcuts

These are the defaults; customise them in `config.toml`. Uppercase letter
bindings appear below as `Shift+` plus the lowercase letter.

### Navigation and layout

| Keys | Action |
| --- | --- |
| Up/Down or `k`/`j` | Move through the listing |
| Home / End / PageUp / PageDown | Jump or scroll through the listing |
| Enter / Backspace (or Alt+Up) | Enter a directory / go to its parent |
| Alt+Left / Alt+Right | Go back / forward in directory history |
| Alt+l | Open Go to directory |
| Alt+p | Toggle one or two panes |
| Tab / Shift+Tab | Switch active pane |
| Ctrl+Left / Ctrl+Right | Adjust the pane split |
| Alt+m | Maximise or restore the active pane |

### Selection and display

| Keys | Action |
| --- | --- |
| Space / Insert | Toggle a mark / toggle a mark and advance |
| Shift+Up / Shift+Down or Shift+click | Extend or shrink a marked range |
| Ctrl+a / Ctrl+u | Mark all / clear marks in the visible listing |
| Alt+b | Select by wildcard or regex |
| `/` | Filter the listing; Enter accepts, Esc cancels or clears |
| `h` | Show or hide hidden entries |
| `i` | Cycle Git-ignore display: dim, hide, off |
| `s` / `v` / `g` | Cycle sort order / reverse order / directory grouping |
| `r` | Refresh the directory |
| Shift+v / Alt+v | View options / view profiles |

Linux and macOS refresh automatically. **On Windows, press `r` to see external
changes.** If a directory becomes unavailable or is replaced, reopen it after
resolving the error.

### Information and exit

| Keys | Action |
| --- | --- |
| Alt+i / Alt+Enter | Toggle metadata sidebar / open metadata popup |
| F2 | Open the preferred metadata view |
| `c` (in metadata) | Count a directory's contents and size |
| F12 | Toggle colour diagnostics |
| Alt+PageUp / Alt+PageDown | Scroll colour diagnostics |
| `?` / F1 | Open Help |
| Alt+h | Session notification history |
| Alt+k / Ctrl+Alt+k | Dismiss oldest toast / dismiss all toasts |
| `q` / F10 / Ctrl+c | Quit without saving a session |
| Shift+q | Save the navigation session and quit |

### Local operations

| Keys | Action |
| --- | --- |
| F5 / Alt+c | Copy the visible selection |
| F6 / Alt+x | Move the visible selection |
| Shift+F5 / Shift+c | Copy without multi-entry confirmation |
| Shift+F6 / Shift+m | Move without multi-entry confirmation |
| F7 | Create a directory |
| Shift+F7 / Alt+n | Create an empty file |
| Alt+r / Alt+a | Rename one entry / edit supported attributes |
| F8 / Alt+t | Preview and move the selection to native Trash |
| Shift+F8 / Alt+d | Preview permanent deletion; Enter cancels |
| F9 / Alt+j | Jobs, progress and results |
| `y` | Copy names or paths to the clipboard |

## Working with files

Operations use visible marks, or the highlighted entry when no visible marks
remain. Marks hidden by filters or visibility settings stay marked but are
excluded from operations. The `..` row is navigation only; it cannot be marked
or used in file operations. Hidden entries start hidden, and Git-ignored entries
start dimmed.

Alt+b selects by wildcard or regex, for example `*.rs`. Wildcards match whole
filenames; regex matches substrings. **Replace can clear visible marks even when
nothing matches.** Review the preview before applying a pattern.

With two panes, copy and move target the other pane: one entry starts at once,
while multiple entries ask for confirmation. With one pane, choose a destination
in the dialog. F2 in a transfer dialog toggles SHA-256 verification. Conflicts
default to Skip; directories merge.

When creating files or directories, enter a single name. Existing entries are
not overwritten.

Review Trash and permanent-deletion previews before confirming. **Enter cancels
permanent deletion.** TFM does not use permanent deletion as a Trash fallback.
If the OS reports irreversible deletion during a Trash request, review the
critical incident in Jobs; acknowledgement cannot restore the data.

In Jobs, `c` requests cancellation, `r` retries incomplete work and `x` reconciles
an unknown outcome. **Unknown outcomes block retries and normal safe exit until
reconciled.** When quitting during an operation, choose whether to wait or cancel
safely; TFM waits for active work and cleanup before exiting.

## Copy names and paths

Press `y`, choose `m` Marked, `h` Highlighted or `c` Current folder, then choose
`n` Name, `r` Relative or `f` Full path.
For example, `y h f` copies the highlighted entry's full path.
Wait for the directory listing to load before starting.
Enter accepts defaults, Backspace goes back and Esc cancels.
Copied text is plain and unquoted; marked entries use one line each.

Relative paths use a fixed session base, initially the first directory argument
or the directory you launched TFM from.
At **From**, use `s` for this pane's folder or `Shift+s` for the other pane's
folder in two-pane mode. Both change the base without copying.
Wait for confirmation before using the new base.
Relative copying requires an accessible base directory, including after restoring
a session. If it becomes unavailable, TFM keeps the base and clipboard unchanged;
choose another base or use Name or Full path.

On Linux, clipboard copying needs `wl-copy` (Wayland) or `xclip`/`xsel` (X11).
Clipboard access over SSH uses OSC 52 and requires terminal support;
**sent does not guarantee accepted**.
In-app Help covers clipboard setup, compatibility and limits.

## Views and metadata

Use Shift+v to choose columns, sorting and size, date, permission or ownership
formats. Alt+v offers **Compact**, **Detailed** and **Minimal** profiles, or you
can save a named view. Profiles apply to the active pane unless you choose
**Apply to both**.

Metadata views show the highlighted entry's details. Press `c` within one to
count directory contents and size; press it again to refresh the count.

On Windows, **Symbolic** shows `RHSA` (read-only, hidden, system, archive),
with `-` for unset attributes. **Native hex** shows the full Windows bitmask.
Click an attribute value in a metadata view to toggle that view's format.

## Notifications

Results and errors appear in toasts or the status area. Notification history
includes expired toasts and lasts until you quit. Critical incidents and unknown
outcomes remain available in Jobs after their toasts expire.

## Saving and restoring sessions

Shift+q saves both pane locations, the layout and relative-path base. Restore a
saved session, or save locations without opening the interface, with:

```sh
tfm --restore-session
tfm --save-session -2 /path/to/left /path/to/right
```

Ordinary quit does not overwrite the saved session. Set `restore_session = true`
for automatic restoration; explicit CLI paths take precedence. Marks, filters,
directory history, notifications and Jobs last only for the current run.

## Configuration

TFM works without a configuration file and saves preferences when they change.
It uses `tfm/config.toml` under your platform's configuration directory:

| Platform | Configuration directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME` or `~/.config` |
| macOS | `$XDG_CONFIG_HOME` or `~/Library/Application Support` |
| Windows | `%APPDATA%` |

Example:

```toml
two_panes = true
restore_session = false
split_percent = 50 # Range: 20–80
human_sizes = true
fuzzy_timestamps = true
owner_display = "names" # or "numeric"
ignore = "dim" # "dim", "hide" or "off"
mouse = true
# path_hover = "#b4f5ff"
# inactive_pane_border = "#767676"
toast_placement = "bottom-right" # top-left, top-right, bottom-left, bottom-right
toast_expiry_seconds = 5 # Range: 1–60

[columns]
size = true
modified = true
attributes = true
owner = false

[clipboard]
osc52_fallback = false # Opt in to terminal clipboard fallback

[bindings]
"Ctrl+f" = "filter"
```

Custom bindings override defaults. For punctuation, use the produced character
rather than a `Shift+` symbol combination; terminals cannot distinguish every
modified-key combination. Set `NO_COLOR` to disable colours.

## Contributing and licence

See [Contributing](CONTRIBUTING.md) for development instructions,
[security reporting](SECURITY.md) to report a vulnerability, and the
[architecture](docs/architecture.md) and [planned work](TODO.md) for project details.

TFM is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
See [third-party notices](THIRD_PARTY_NOTICES.md) and
[source attribution](docs/provenance.md) for dependency and adapted-code details.
