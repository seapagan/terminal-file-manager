# Terminal File Manager

`tfm` is a keyboard-first terminal file manager for Linux, macOS and Windows.
Browse local directories in one or two panes, sort and filter files, mark entries,
and inspect file information without leaving the terminal.

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

A single directory argument sets both pane locations. You can switch between one
and two visible panes at any time; each pane keeps its own directory and view
settings. On a narrow terminal, `tfm` shows the active pane until there is
room for both.

Use the arrow keys (or `j`/`k`) to navigate, Enter to enter a directory and
Backspace to go up. With a mouse, click to focus, double-click a directory to
enter it, and use the wheel to scroll. Press `?` to see the shortcuts currently
configured in your installation.

## Keyboard shortcuts

These are the default bindings. You can customise them in `config.toml`.
Uppercase letter bindings use `Shift+` plus the lowercase letter below; for
example, `Shift+v` opens view options while `v` reverses the sort order.

### Navigation and layout

| Keys | Action |
| --- | --- |
| Up/Down or `k`/`j` | Move through the listing |
| Home / End / PageUp / PageDown | Jump or scroll through the listing |
| Enter / Backspace (or Alt+Up) | Enter directory / go to parent, highlighting the directory just exited |
| Alt+Left / Alt+Right | Go back / forward in directory history |
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
| Alt+b | Mark, unmark or replace visible marks by wildcard or regex |
| `/` | Filter the current listing; Enter accepts, Esc cancels or clears |
| `h` | Show or hide hidden entries |
| `i` | Cycle Git-ignore display: dim, hide, off |
| `s` / `v` / `g` | Cycle sort order / reverse order / directory grouping |
| `r` | Refresh the directory |
| `Shift+v` / Alt+v | View options / view profiles |

Linux and macOS refresh local directories, including the hidden pane, while
preserving the cursor and surviving marks. **On Windows, press `r` to see
external changes.** You can use `r` on any platform to refresh the listing.

If automatic refresh fails on Linux or macOS, `tfm` reports the problem and
retries. Slow directories and unavailable mounts can delay updates. If a
directory disappears or another directory replaces it, reopen the directory;
`tfm` keeps the last good listing and reports the error.

### Pattern selection

Press **Alt+b** or click **Alt+b Mark** to mark filenames by wildcard or regex.
Type a pattern, choose **Wildcard**, **Regex** and **Case sensitive** options,
then use **Mark**, **Unmark** or **Replace**. With a valid pattern, Enter from
the Pattern field moves to Mark; a second Enter applies it. Esc cancels without
changing marks.

Tab or click to move between controls. Use **Alt+w / Alt+x / Alt+s** to change
matching options while typing, and **Alt+m / Alt+u / Alt+r** to mark, unmark or
replace. Outside the Pattern field, the control letters also work without Alt.
You can assign another shortcut to `pattern-selection` in `[bindings]`.

Wildcards match the whole filename, for example `*.rs`, `file?.txt` or `[abc]*`.
Regex matches substrings; use anchors such as `^report.*\.txt$` to match a whole
name. Matching starts case-sensitive in both modes.

The preview counts matches across the visible listing, including directories.
Hidden, ignored and filtered-out marks stay unchanged. **Replace can clear
visible marks even when the pattern matches nothing.** Empty or invalid patterns
cannot apply. If the listing changes while the dialog is open, reopen it to
review the new matches. Enlarge the terminal if the controls cannot fit.

### Information and exit

| Keys | Action |
| --- | --- |
| Alt+i / Alt+Enter | Toggle metadata sidebar / open metadata popup |
| F2 | Open the preferred metadata presentation |
| `c` (in metadata view) | Count a directory's contents and size |
| F12 | Toggle colour diagnostics |
| Alt+PageUp / Alt+PageDown | Scroll colour diagnostics |
| `?` | Show keyboard help |
| `q` / F10 / Ctrl+c | Quit without saving a session |
| `Shift+q` | Save the navigation session and quit |

### Local operations

| Keys | Action |
| --- | --- |
| F5 / Alt+c | Copy the visible selection |
| F6 / Alt+x | Move the visible selection |
| F7 | Create a directory |
| Shift+F7 / Alt+n | Create an empty file |
| Alt+r / Alt+a | Rename one entry / edit supported attributes |
| F8 / Alt+t | Preview and move the selection to native Trash |
| Shift+F8 / Alt+d | Preview permanent deletion; Enter cancels |
| F9 / Alt+j | Jobs, progress and retained results |

Create a file or directory in the active pane by entering one name. Existing
entries stay intact. Press **Shift+F7** or **Alt+n** to create an empty file,
or click **Alt+n File** in the footer. `tfm` selects the new file if it is visible
and you have not moved the cursor or changed the selection or directory in the
meantime.

Other operations use visible marks, or the cursor when no visible marks remain.
With two panes, copy and move target the other pane: one entry starts at once,
while multiple entries ask for confirmation. Shift+F5 / Shift+F6 or
`Shift+c` / `Shift+m` bypass that confirmation. With one pane, choose the
destination in the dialog. F2 in a transfer dialog toggles SHA-256 verification.

Trash and permanent deletion show a preview before confirmation. Wait for
counting to finish and review the selection. Enlarge the terminal if the preview
cannot fit. **Enter cancels permanent deletion.**

Conflicts compare Incoming with Existing and default to Skip. Newer replaces
Existing only when Incoming has a later modification time. `a Apply to all`
applies your next choice to matching conflicts in the job; Esc cancels the job.
Directories merge. Enlarge the terminal if the comparison cannot fit.

Press F9 to inspect jobs and errors. Select a job with Up/Down or the mouse;
PgUp/PgDown or the wheel scrolls its details. Jobs opens in Summary; `e` toggles
Evidence.
Available actions depend on the job's state:

- `c` requests cancellation.
- `r` retries incomplete work, leaving completed entries untouched.
- `x Reconcile` rechecks an eligible unknown outcome.

**An unknown outcome blocks retries and normal safe exit until reconciled.**
Critical irreversible Trash incidents require the separate acknowledgement below.

Results stay in Jobs for the current session. `Shift+q` saves navigation, not job
history. On quit, choose Return, Wait, or Cancel safely; `tfm` waits for active
operations and cleanup.

Trash can fail where the native facility cannot preserve the selected object.
Permanent deletion requires separate confirmation; `tfm` does not use it as a
fallback. If the OS reports permanent deletion during a Trash request, Jobs shows
a critical incident. Acknowledgement cannot restore the data. After all workers
stop, type `ACKNOWLEDGE` and press F10 in the exit dialog. Enter or Esc keeps
`tfm` open.

`tfm` hides hidden entries by default. Use `h` or the View menu to change
between **Hidden: Hide** and **Hidden: Show**. A small `h` beside an entry's type
marks it as hidden. `tfm` dims Git-ignored entries by default; choose `hide`
or `off` if you prefer. It retains marks on entries hidden by a filter or
visibility option; operations use the visible selection.

## Views and metadata

Press `Shift+v` to change the active pane's columns, sorting, file sizes,
timestamps, permissions or ownership display. Press Alt+v for the built-in **Compact**,
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
Close the floating popup with Esc before navigating or starting a file operation.

## Saving and restoring sessions

Press `Shift+q` to save both pane locations and the layout when quitting.
Restore them later with:

```sh
tfm --restore-session
```

Ordinary quit (`q`, F10 or Ctrl+c) does not overwrite your saved session. You
can also enable automatic restoration on startup with `restore_session = true`.
Explicit command-line paths take precedence over automatic restoration.

To save a session without opening the interface:

```sh
tfm --save-session -2 /path/to/left /path/to/right
```

`tfm` keeps marks, filters and back/forward navigation history for the current
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

Press `?` for one scrollable Help document, grouped by task. Press `/` in Help
or click **Jump** to find a section or search commands and their effective
shortcuts. Use the arrows and Enter, or click a result, to jump to its entry.
Esc closes the picker first, then Help.

Custom bindings override the defaults, and `?` displays the effective
shortcuts. For punctuation keys, use the character produced rather than a
`Shift+` symbol combination; some modified-key combinations cannot be
reliably distinguished by terminals.

Set `NO_COLOR` to disable colours. Press F12 to inspect the terminal colour
diagnostics when needed.

## Contributing and licence

See [Contributing](CONTRIBUTING.md) for development instructions and
[security reporting](SECURITY.md) to report a vulnerability. Explore the
[architecture](docs/architecture.md), [acceptance record](docs/acceptance.md)
and [planned work](TODO.md) for more project detail.

`tfm` is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
See [third-party notices](THIRD_PARTY_NOTICES.md) for dependency licences and
[source attribution](docs/provenance.md) for adapted code.
