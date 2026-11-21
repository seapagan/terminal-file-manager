# Terminal File Manager

`tfm` is a keyboard-first terminal file manager for Linux, macOS and Windows.
Browse in one or two panes, copy and move files, rename entries, filter and mark
selections, and inspect file information without leaving the terminal.

SSH/SFTP access, a file viewer and an editor are planned. Enter opens
directories; it does not open or execute files.

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

Press `?` or F1 for Help. In the file list, you can search Help; in a dialog,
you see instructions for that dialog. Use F1 while editing a text field, where
`?` stays part of the text.

## Keyboard shortcuts

Customise these default shortcuts in `config.toml`. The tables show uppercase
letter shortcuts as `Shift+` plus the lowercase letter.

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
| Alt+e / Alt+Shift+e | Replace all marks in the active pane by extension / final extension |
| `/` | Filter the listing; Enter accepts, Esc cancels or clears |
| `h` | Show or hide hidden entries |
| `i` | Cycle Git-ignore display: dim, hide, off |
| `s` / `v` / `g` | Cycle sort order / reverse order / directory grouping |
| `r` | Refresh the directory |
| Shift+v / Alt+v | View options / view profiles |

On Linux and macOS, TFM refreshes the listing as files change, including their
sizes and modification times. **On Windows, press `r` to refresh after external
changes.** Reopen an unavailable or replaced directory after resolving the error.

### Information and exit

| Keys | Action |
| --- | --- |
| Alt+i | Toggle metadata sidebar |
| Alt+Enter / Alt+o | Open metadata popup |
| F2 | Open the preferred metadata view |
| `c` (in metadata) | Count a directory's contents and size |
| F12 | Toggle colour diagnostics |
| Alt+PageUp / Alt+PageDown | Scroll colour diagnostics |
| `?` / F1 | Open Help |
| Alt+h / Shift+F9 | Open session notification history |
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
| F8 / Delete / Alt+t | Preview and move the selection to native Trash |
| Shift+F8 / Shift+Delete / Alt+d | Preview permanent deletion; Enter cancels |
| F9 / Alt+j | Jobs, progress and results |
| `y` | Copy names or paths to the clipboard |

### Filters and pane options

An active filter appears as **FILTER:** at the bottom of the pane. A filtered
listing with no matches shows **No matches**.

TFM clears the pane's filter after you enter another folder. To keep filters as
you browse, set `keep_filter_on_navigation = true` in `config.toml` or select
**Keep filter on navigation** in View Options (Shift+v). Refreshes and failed
navigation keep the filter.

Click **Sort**, **Group**, **Hidden** or **Ignore** at the bottom of a pane to
choose a setting. The menu opens above the control. Use Up/Down and Enter, or
click a choice; Esc closes the menu without applying it. Press F1 or click
`[?]` to read about each option. Esc closes Help and returns to the same
highlighted choice. The keyboard shortcuts for these settings cycle through
their values.

View Options (Shift+v) also lets you switch between one and two panes for the
current session. The `two_panes` setting controls the startup layout.

### Terminal shortcut conflicts

Terminal emulators, multiplexers and desktop terminal hosts can capture shortcuts
before TFM receives them. Windows Terminal can use `Alt+Enter` for fullscreen;
use `Alt+o` to open TFM's metadata popup. Zellij and the Herdr desktop app can
use `Alt+h` for pane focus; use `Shift+F9` for TFM's notification history. The
Herdr desktop app may also reserve `Alt+j`, `Alt+k`, `Alt+l` and F2.

In Zellij, press **Ctrl+G** to toggle locked mode. While locked, Zellij passes
its usual shortcuts through to TFM; press Ctrl+G again to unlock.

For a shortcut your terminal still captures, choose a key combination it passes
through and bind it in TFM's `config.toml`. You can keep your terminal settings
unchanged. For example, add these entries to `[bindings]`:

```toml
"Ctrl+F9" = "notifications"
"Ctrl+F2" = "metadata"
```

Help shows the shortcuts in use after your changes.

## Working with files

File operations use the visible marked entries. If no marked entries are
visible, TFM uses the highlighted entry. Filtering or hiding an entry keeps
its mark but excludes it from operations. The `..` row is for navigation; you
cannot mark it or use it in file operations. TFM hides hidden entries and dims
Git-ignored entries by default.

On Windows, red filenames identify files and directories with the System
attribute. Windows permissions still determine access.

Use `Alt+e` to select visible files with the same extension as the highlighted
file. Configured compound extensions such as `.tar.gz` stay distinct from `.gz`.
Use `Alt+Shift+e` to match the final extension, including both `.gz` and `.tar.gz`
files. Set `compound_extensions = [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst"]`
in `config.toml` to choose which compound extensions TFM recognises.

Both shortcuts clear all marks in the active pane, including marks on hidden
and filtered-out entries, then mark visible matches. The other pane keeps its
marks. If the highlighted entry has no usable extension, both shortcuts leave
all marks unchanged.

Alt+b opens pattern selection. Use a wildcard such as `*.rs` or a regular
expression (regex). Wildcards match whole filenames; regex matches within
filenames. Mark and Unmark affect matching visible entries. **Replace clears
all marks in the active pane, including marks on hidden and filtered-out
entries, then marks visible matches. Zero matches leaves the pane with no
marks.** The other pane keeps its marks. Review the preview before applying
a pattern.

TFM remembers the matching mode and case sensitivity until you quit. Each time
you open pattern selection, the pattern starts empty. Set
`pattern_mode = "wildcard"` (or `"regex"`) and `pattern_case_sensitive = true`
in `config.toml` to choose startup defaults.

With two panes, copy and move target the other pane: one entry starts at once,
while multiple entries ask for confirmation. With one pane, choose a destination
in the dialog. F2 in a transfer dialog toggles SHA-256 verification. Conflicts
default to Skip; directories merge.

Use Left/Right or Home/End to move through a text field. Backspace and Delete
edit at the cursor; Insert switches between insert and overwrite modes. On
terminals that support cursor shapes, a bar indicates insertion and a block
indicates overwrite. Pasted text starts at the cursor, and long fields scroll
horizontally.

Enter a single name to create a file or directory. TFM does not overwrite
existing entries.

Review Trash and permanent-deletion previews before confirming. **Enter cancels
permanent deletion.** TFM requires explicit approval for permanent deletion.
On Windows, TFM skips the ordinary Recycle Bin confirmation after you approve
its preview. Windows still warns if it cannot recycle an item: **No keeps the
item; Yes permits permanent deletion.** Jobs labels completed permanent
deletions on Windows as irreversible. Unverified or conflicting deletion
results remain critical or unknown.

In Jobs, `c` requests cancellation, `r` retries incomplete work and `x`
reconciles an unknown outcome by checking what happened. **Unknown outcomes
block retries and normal exit until resolved.** To quit during an operation,
choose whether to wait or request cancellation. TFM waits for active work and
cleanup to finish before exiting.

## Copy names and paths

Wait for the directory listing to load, then press `y`. Choose `m` Marked,
`h` Highlighted or `c` Current folder, followed by `n` Name, `r` Relative or
`f` Full path. For example, `y h f` copies the highlighted entry's full path.
Enter accepts defaults, Backspace returns to the previous step and Esc cancels.
TFM copies plain, unquoted text, with one line per marked entry.

The base folder for relative paths stays the same as you browse. It starts at
the first directory argument or the folder you launched TFM from. At **From**,
use `s` to set the base to this pane's folder, or `Shift+s` for the other pane's
folder in two-pane mode. These choices change the base without copying. Wait
for confirmation before using the new base.

TFM must be able to access the base folder, including after restoring a
session. If it cannot, it leaves the base and clipboard unchanged. Choose
another base or use Name or Full path.

On Linux, clipboard copying needs `wl-copy` (Wayland) or `xclip`/`xsel` (X11).
TFM running over SSH copies through OSC 52, which requires terminal support.
**A `Sent` message means TFM sent the request; your terminal may still reject it.**
Press `?` during Copy Path for clipboard setup, compatibility and limits.
Press `Esc` to return to the same choice, then `Esc` again to cancel.

## Views and metadata

Use Shift+v to choose columns, sorting and size, date, permission or ownership
formats. Alt+v offers **Compact**, **Detailed** and **Minimal** profiles, or you
can save a named view. Profiles apply to the active pane unless you choose
**Apply to both**.

Metadata views show the highlighted entry's details. Press `c` within one to
count directory contents and size; press it again to refresh the count.

On Unix, choose **Symbolic** for the file type and permissions (`drwxrwxr-x`),
or **Octal** for four permission digits (`0775`).

On Windows, **Symbolic** shows `RHSA` (read-only, hidden, system, archive),
with `-` for unset attributes. **Native hex** shows the full Windows bitmask.
Click metadata attributes, owner or time values to change their format.
The popup and sidebar share these choices for each pane during the session.

Windows owners appear as `DOMAIN\user`; **Numeric** shows the SID. TFM looks up
account names in the background and keeps the SID if it cannot find a name.
You can see ownership lookup errors in metadata.

## Notifications

Use `Alt+h` or `Shift+F9` to open notification history. TFM shows results and
errors in toasts or the status area. History includes expired toasts and lasts
until you quit. Critical incidents and unknown outcomes remain available in
Jobs after their toasts expire.

## Saving and restoring sessions

Shift+q saves both pane locations, the layout and relative-path base. Restore a
saved session, or save locations without opening the interface, with:

```sh
tfm --restore-session
tfm --save-session -2 /path/to/left /path/to/right
```

Quit without saving to keep the previous saved session. Set
`restore_session = true` to restore it on startup; paths you provide on the
command line take precedence. Marks, filters, directory history, notifications
and Jobs last for the current run.

## Configuration

You can use TFM without a configuration file. TFM saves preference changes in
`tfm/config.toml` under your platform's configuration directory:

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

Custom bindings override defaults. To bind punctuation, use the character you
type rather than a `Shift+` symbol combination. Terminals cannot distinguish
every modified-key combination. Set `NO_COLOR` to disable colours.

## Contributing and licence

See [Contributing](CONTRIBUTING.md) for development instructions,
[security reporting](SECURITY.md) to report a vulnerability, and the
[architecture](docs/architecture.md) and [planned work](TODO.md) for project details.

TFM is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
See [third-party notices](THIRD_PARTY_NOTICES.md) and
[source attribution](docs/provenance.md) for dependency and adapted-code details.
