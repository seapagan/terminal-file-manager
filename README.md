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
| Enter / Backspace (or Alt+Up) | Enter directory / go to parent, highlighting the directory just exited |
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
| Alt+B | Mark, unmark or replace visible marks by wildcard or regex |
| `/` | Filter the current listing; Enter accepts, Esc cancels or clears |
| `h` | Show or hide hidden entries |
| `i` | Cycle Git-ignore display: dim, hide, off |
| `s` / `v` / `g` | Cycle sort order / reverse order / directory grouping |
| `r` | Refresh the directory |
| `V` / Alt+V | View options / view profiles |

Linux and macOS refresh local directories automatically, including the hidden
pane. External changes update the listing while preserving the cursor and
surviving marks. Press `r` to refresh manually on every platform.
If native notifications fail, automatic refresh slows to a maximum 30-second
interval while TFM retries the watcher. Press `r` for an immediate refresh.

**Windows requires `r` to refresh.** Native directory watches blocked ancestor
renames, and two-second polling proved expensive in large directories. Manual
refresh avoids persistent watcher handles and idle enumeration. Windows panes,
including hidden panes, retain their last listing until deliberate refresh or
navigation.

Linux and macOS also reconcile each pane every 30 seconds to catch missed
notifications. If native watching fails, TFM warns once and refreshes periodically
while retrying. These automatic recovery checks apply to Linux and macOS.
Large or slow directories, concurrent operations and unavailable mounts can
extend refresh latency.
If a directory disappears or another directory replaces it, TFM retains the
last good view and reports the error; navigate out and reopen the directory to select the replacement.

### Pattern selection

Press **Alt+B** or click **Alt+B Mark** to mark filenames by pattern in the
active pane. Choose Wildcard or Regex, edit the expression, and use Tab and
Enter or the mouse to select **Mark matches**, **Unmark matches**, or
**Replace visible marks with matches**. Enter from the expression focuses
Cancel; Esc cancels without changing marks. Paste edits only the expression.
You can bind another key to `pattern-selection` in `[keybindings]` if your
terminal or multiplexer reserves Alt+B.

Wildcards match the whole filename: `*.rs`, `file?.txt`, `[abc]*` and `{a,b}*`.
Globset treats `?` as one byte and case-insensitive wildcards fold ASCII.
Regex uses Rust's `regex` syntax and matches substrings; use `^` and `$` to
anchor a filename, for example `^report.*\.txt$`. Regex supports Unicode case
folding; `(?-u:...)` enables explicit byte matching for native non-UTF names.
Both modes start case-sensitive. Matching uses native filename bytes, including
Windows native encoding, without replacing invalid characters for selection.

The live counts include directories and only visible selectable names, across
the full listing. Hidden, ignored and filtered-out marks stay unchanged.
Replace shows how many visible marks it replaces and can clear them when
there are zero matches. Empty or invalid patterns cannot apply. If the visible
names or identities change while the modal is open, cancel and reopen it.
Enlarge the terminal when the expression, counts or action cannot fit.

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
F2 in a transfer dialog toggles SHA-256 verification; its button describes the
next action. PgUp/PgDown or the wheel over the source list scroll selected roots.
An ASCII activity spinner runs while Trash/Delete counts recursively.
Approval stays disabled until counting finishes and the dialog can
show the selected objects and required safety context. Resize or cancel if needed.

Conflicts compare Incoming with Existing and default to Skip. Newer replaces
Existing only when Incoming has a later modified time. `A Apply to all` toggles
whether the next chosen action also applies to matching conflicts in this job;
`Esc` cancels the job. Unavailable choices stay disabled. Enlarge the terminal
if the comparison cannot fit before replacing. Directories merge. In Jobs,
select a job with Up/Down or the mouse. Jobs opens with a compact summary;
`E` toggles full evidence, including recorded errors and individual Trash receipts.
Failed operations show a reason in the status line. Jobs Summary groups repeated
failure reasons with affected counts and keeps mixed outcomes visible.
The viewport stays fixed while you select jobs or toggle evidence.
At small terminal sizes, the Jobs footer lists eligible shortcuts when their
buttons cannot fit; safety warnings take priority.
Available controls depend on the job state:
`C` requests cancellation, `R` retries incomplete work from scratch, and `X`
rechecks an unknown outcome with retained evidence. Completed entries stay
excluded from retries. PgUp/PgDown or the mouse wheel scroll details; results
and error evidence remain in Jobs after success, warning and error notices expire.

Trash can fail where the native facility cannot preserve the selected object.
Linux uses home Trash or a validated Trash on the source volume; it never copies
and deletes across filesystems to emulate Trash. Permanent
deletion requires its own confirmation and never serves as a fallback. On quit, choose Return, Wait, or
Cancel safely; `tfm` waits for workers and cleanup. An unknown outcome blocks
retry and safe exit until reconciliation.

If the OS reports permanent deletion during a Trash request, TFM retains a
critical incident in Jobs. This is irreversible; acknowledgement cannot restore
the data. After all workers stop, quitting offers a separate acknowledgement:
type `ACKNOWLEDGE`, then press F10. Enter or Esc keeps TFM open. Ordinary unknown
outcomes cannot use this exit path. Job diagnostics exist only for the current
session; `Q` saves navigation, not job history.

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
Counts are on-demand snapshots; press `c` again to update them.
The floating popup keeps keyboard and mouse focus until you close it; pane
selection and file-operation shortcuts stay inactive behind it. Its fixed
**Esc Close** control remains available while the fields scroll.

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

The Linux `inotify` and `inotify-sys` dependencies use ISC. See
[third-party notices](THIRD_PARTY_NOTICES.md) for their terms and other non-base
licences. Binary archives include this notices file alongside both TFM licences.
