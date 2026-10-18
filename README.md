# Terminal File Manager

TFM is a keyboard-first local file browser for Linux, macOS and Windows.
Browse in one or two panes, sort and filter listings, mark entries and inspect
metadata. The current version is read-only: file operations, SSH/SFTP, viewers
and editors are planned. Enter opens directories; files stay unopened.

## Installation

Build from a checkout with Rust 1.95.0 or newer. There is no public release yet.

```sh
git clone https://github.com/seapagan/terminal-file-manager.git
cd terminal-file-manager
cargo install --path . --locked
```

The executable is `tfm`. To try it without installing, use `cargo run --locked`;
pass arguments after `--`, for example `cargo run --locked -- -2`.

## Usage

```sh
tfm                         # Browse the current directory
tfm /path/to/directory
tfm -2                      # Start with two panes
tfm /path/to/left /path/to/right
tfm --help
```

Browsing requires an interactive terminal. A single path starts both panes at
that location. On narrow terminals, you see the active pane until space permits
both. Pane headers shorten long paths from the middle to keep the current
directory visible. Click a pane to focus it, double-click a directory to enter
it, and use the wheel to scroll. Press `?` for your configured shortcuts.

## Keybindings

| Keys | Action |
| --- | --- |
| Up/Down or k/j; Home/End/PageUp/PageDown | Move the cursor |
| Enter; Backspace or Alt+Up | Enter a directory; go to its parent |
| Alt+Left / Alt+Right | Back/forward |
| Alt+P; Tab/Shift+Tab | Toggle one/two panes; focus the other pane |
| Ctrl+Left / Ctrl+Right; Alt+M | Adjust the split; maximise/restore active pane |
| Space; Insert | Mark/unmark; mark/unmark and advance |
| Shift+Up / Shift+Down; Shift+click | Extend or shrink a marked range |
| Ctrl+A / Ctrl+U | Select all / clear marks in the visible listing |
| /; Enter; Esc | Edit fuzzy filter; accept; cancel/clear |
| h; i | Toggle hidden entries; cycle ignore dim/hide/off |
| s; v; g; r | Cycle sort; reverse; cycle directory grouping; refresh |
| Alt+I; Alt+Enter; F2 | Toggle metadata sidebar; open metadata popup; use preferred presentation |
| V; Alt+V | View options; View profiles |
| c; Esc | Count a directory's contents; cancel count/close overlay |
| F12; Alt+PageUp / Alt+PageDown | Toggle colour diagnostics; scroll diagnostics |
| ?; q/F10/Ctrl+C | Help; quit |
| Q | Save navigation session and quit |

Hidden entries start excluded (`Hidden: Hide`); toggle to `Hidden: Show` in the
View menu or with `h`. The `h` beside the file-type indicator marks hidden entries
(for example, `dh` or `-h`). The blank hidden cell keeps filenames aligned.
The type indicator uses `d` for directories and `D` for devices.
Ignored filenames start dimmed. Marks outside the current filter or visibility
settings remain marked. Each pane keeps its own location, filter and marks while
you switch focus or layouts.

## View profiles

Press `V` to choose columns, size/time formats, owner/group display, sorting and
visibility. Unix permissions support symbolic and octal formats. Press `Alt+V`
to choose **Compact**, **Detailed** or **Minimal**. Detailed shows hidden entries
and requests the sidebar; Compact and Minimal turn both off. Dual-pane and
narrow layouts hide the sidebar while retaining its requested state.

In View profiles, press Enter to apply a profile to the active pane. Press Space
on a profile to select **Apply to both**, then Enter to confirm. Choose **Save
active as…** to save your view under a name. You must save again to update a
named profile after changing its options. Profiles store display and listing
preferences; use sessions to save locations.

Metadata values have temporary display controls: click attributes for symbolic/octal,
timestamps for exact/fuzzy, or owner/group for names/numbers. These choices affect
only that open metadata view. Reopening it uses the active pane's View defaults.

## Sessions

Press `Q` to save both pane locations and the layout, then quit. Use
`tfm --restore-session` to return to them. Ordinary quit (`q`, F10 or Ctrl+C)
leaves the saved session unchanged. Marks, filters and navigation history last
for the current run.

Set `restore_session = true` to restore on startup without explicit paths.
Paths supplied on the command line take precedence over that preference.
Use `tfm --save-session -2 LEFT RIGHT` to save locations without opening the TUI.
Keep saved sessions on the same operating system.

## Configuration

TFM saves changed preferences on normal exit. Browsing without changing settings
does not create a config file. Edit `tfm/config.toml` in your config directory:

- Linux: `$XDG_CONFIG_HOME`, or `~/.config`.
- macOS: `$XDG_CONFIG_HOME`, or `~/Library/Application Support`.
- Windows: `%APPDATA%`.

On Unix, TFM ignores empty or relative `XDG_CONFIG_HOME` values for both
configuration and saved sessions.

TFM can read a config symlink pointing to a regular file, up to 1 MiB.
Saving preferences refuses linked destinations; edit the target file yourself.

Example settings:

```toml
two_panes = true
restore_session = false
split_percent = 50 # 20–80
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

Use `[profiles.NAME]` and `[profiles.NAME.columns]` for named views in TOML.
Custom keybindings override defaults; `?` shows your effective bindings.
Exact Ctrl+C remains an emergency exit in menus and filters, even when rebound.
Ctrl+Alt+C and other modified variants follow their configured bindings.
For symbols, bind the character your keyboard produces without `Shift+`.
TFM rejects shifted symbol, digit and Space bindings because legacy terminals
cannot distinguish their physical Shift state. Shifted letters remain supported.
F12 opens colour diagnostics by default. Set `NO_COLOR` to disable colours.

## Links

- [Contributing and development checks](CONTRIBUTING.md)
- [Architecture](docs/architecture.md) and [acceptance status](docs/acceptance.md)
- [Planned work](TODO.md) and [security reporting](SECURITY.md)
- [Source attribution](docs/provenance.md) and [third-party notices](THIRD_PARTY_NOTICES.md)

TFM is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
