# Terminal File Manager

`tfm` is a keyboard-first terminal file manager for Linux, macOS and Windows.
Browse local directories in one or two panes, sort and filter files, mark entries,
and inspect detailed file information — all without leaving the terminal.

Copy, move, rename and organise local files from the browser. SSH/SFTP, file
viewing and editing remain planned. Enter navigates into directories; it does
not open or execute files.

## Installation

There is no public release yet. Build and install from source with Rust 1.95.0
or newer:

```sh
git clone https://github.com/seapagan/terminal-file-manager.git
cd terminal-file-manager
cargo install --path . --locked
```

The installed command is `tfm`. To try it from the checkout without installing,
run `cargo run --locked` (add CLI arguments after `--`).

## Getting started

```sh
tfm                              # Browse the current directory
tfm /path/to/directory           # Browse a chosen directory
tfm -2                           # Start with two panes
tfm /path/to/left /path/to/right # Start with two different locations
tfm --help                       # See command-line options
```

A single directory argument is used for both panes. You can switch between one
and two visible panes at any time; each pane keeps its own directory and view
settings. On a narrow terminal, `tfm` shows the active pane until there is
room for both.

Use the arrow keys (or `j`/`k`) to navigate, Enter to enter a directory and
Backspace to go up. With a mouse, click to focus, double-click a directory to
enter it, and use the wheel to scroll. Press `?` to see the shortcuts currently
configured in your installation.

## Keyboard shortcuts

These are the default bindings. You can customise them in `config.toml`.

### Navigation and layout

| Keys | Action |
| --- | --- |
| Up/Down or `k`/`j` | Move through the listing |
| Home / End / PageUp / PageDown | Jump or scroll through the listing |
| Enter / Backspace (or Alt+Up) | Enter directory / go to parent |
| Alt+Left / Alt+Right | Go back / forward in directory history |
| Alt+P | Toggle one or two panes |
| Tab / Shift+Tab | Switch active pane |
| Ctrl+Left / Ctrl+Right | Adjust the pane split |
| Alt+M | Maximise or restore the active pane |

### Selection and display

| Keys | Action |
| --- | --- |
| Space / Insert | Toggle a mark / toggle a mark and advance |
| Shift+Up / Shift+Down or Shift+click | Extend or shrink a marked range |
| Ctrl+A / Ctrl+U | Mark all / clear marks in the visible listing |
| `/` | Filter the current listing; Enter accepts, Esc cancels or clears |
| `h` | Show or hide hidden entries |
| `i` | Cycle Git-ignore display: dim, hide, off |
| `s` / `v` / `g` | Cycle sort order / reverse order / directory grouping |
| `r` | Refresh the directory |
| `V` / Alt+V | View options / view profiles |

### Information and exit

| Keys | Action |
| --- | --- |
| Alt+I / Alt+Enter | Toggle metadata sidebar / open metadata popup |
| F2 | Open the preferred metadata presentation |
| `c` (in metadata view) | Count a directory's contents and size |
| F12 | Toggle colour diagnostics |
| Alt+PageUp / Alt+PageDown | Scroll colour diagnostics |
| `?` | Show keyboard help |
| `q` / F10 / Ctrl+C | Quit without saving a session |
| `Q` | Save the navigation session and quit |

### Local operations

| Keys | Action |
| --- | --- |
| F5 / Alt+C | Copy the visible selection |
| F6 / Alt+X | Move the visible selection |
| F7 | Create a directory |
| Alt+R / Alt+A | Rename one entry / edit supported attributes |
| F8 / Alt+T | Preview and move the selection to native Trash |
| Shift+F8 / Alt+D | Preview permanent deletion; Enter cancels |
| F9 / Alt+J | Jobs, progress and retained results |

Operations use visible marks, or the cursor when no visible marks remain. With
two panes, copy and move target the other pane: one entry starts immediately,
while multiple entries ask for confirmation. Shift+F5/F6 or `C`/`M` bypass that
routine confirmation. With one pane, choose the destination in the dialog.
F2 in a transfer dialog enables SHA-256 verification.

Conflicts default to Skip. Choose Replace, Keep both or Newer, and use All to
repeat a choice within that job and type pair. Directories merge. In Jobs,
`C` requests cancellation, `R` retries incomplete work from scratch, and `X`
rechecks unknown outcomes without repeating source removal.
Completed entries stay excluded from retries.

Trash can fail where the native facility cannot preserve the selected object.
Linux Trash requires the source and home Trash to share a filesystem. Permanent
deletion requires its own confirmation and never serves as a fallback. On quit, choose Return, Wait, or
Cancel safely; `tfm` waits for workers and cleanup. An unknown outcome blocks
retry and safe exit until reconciliation.

Hidden entries are excluded by default. Use `h` or the View menu to change
between **Hidden: Hide** and **Hidden: Show**. A small `h` beside an entry's type
marks it as hidden. Git-ignored entries are dimmed by default; choose `hide`
or `off` if you prefer. Marks on entries hidden by a filter or visibility option
are retained, but those entries remain outside the visible selection.

## Views and metadata

Press `V` to change the active pane's columns, sorting, file sizes, timestamps,
permissions or ownership display. Press Alt+V for the built-in **Compact**,
**Detailed** and **Minimal** profiles, or save your own named view. Apply a
profile to the active pane with Enter; use Space to choose **Apply to both**
before confirming. Changes to a saved profile must be saved again to update it.

The metadata sidebar and popup display information about the selected entry.
Within an open metadata view, click attributes, timestamps or owner/group to
switch their presentation. Those temporary choices do not change your saved
view preferences.

Press `c` in a metadata view for an on-demand directory count and size. In the
**sidebar**, Esc cancels an active count but does not close the sidebar. In the
**popup**, Esc cancels the count or closes the popup.

## Saving and restoring sessions

Press `Q` to save both pane locations and the layout when quitting. Restore them
later with:

```sh
tfm --restore-session
```

Ordinary quit (`q`, F10 or Ctrl+C) does not overwrite your saved session. You
can also enable automatic restoration on startup with `restore_session = true`.
Explicit command-line paths take precedence over automatic restoration.

To save a session without opening the interface:

```sh
tfm --save-session -2 /path/to/left /path/to/right
```

Marks, filters and back/forward navigation history are kept only for the current
run.

## Configuration

`tfm` works without a configuration file and saves preferences when they change.
The file is `tfm/config.toml` under your platform's configuration directory:

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

[columns]
size = true
modified = true
attributes = true
owner = false

[bindings]
"Ctrl+f" = "filter"
```

Custom bindings override the defaults, and `?` always displays the effective
shortcuts. For punctuation keys, use the character produced rather than a
`Shift+` symbol combination; some modified-key combinations cannot be
reliably distinguished by terminals.

Set `NO_COLOR` to disable colours. Press F12 to inspect the terminal colour
diagnostics when needed.

## Project documentation

- [Contributing and development checks](CONTRIBUTING.md)
- [Architecture](docs/architecture.md) and [acceptance status](docs/acceptance.md)
- [Planned work](TODO.md) and [security reporting](SECURITY.md)
- [Source attribution](docs/provenance.md) and [third-party notices](THIRD_PARTY_NOTICES.md)

`tfm` is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
