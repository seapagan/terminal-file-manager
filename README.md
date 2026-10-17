# Terminal File Manager

TFM is a keyboard-first local file browser for Linux, macOS and Windows.
Browse in one or two panes, sort and filter listings, mark visible entries,
and inspect metadata without changing your files. M1 is read-only: file
operations, Trash, SSH/SFTP, viewers and editors remain later milestones.
Selected files are never launched.

TFM is in development and has no public release yet. Build from source:

```console
cargo run --locked
cargo run --locked -- /path/to/directory
cargo run --locked -- --help
cargo run --locked -- --version
```

To install from this checkout using your default Rust toolchain:

```console
cargo install --path . --force --locked
```

Rust 1.95.0 or newer is required. The checkout does not override your default
Rust toolchain; CI selects its compiler explicitly and checks the exact MSRV.

The package is `terminal-file-manager`; the command is `tfm`. Each invocation
starts at the shell's current directory, or the supplied native directory paths.
`tfm -2` / `--two-panes` requests two panes; `two_panes = true` configures that
startup default. Two path arguments also start dual-pane mode. One explicit
path initializes both panes. Use `tfm -- -left -right` for leading-hyphen paths.
Alt+P changes the current layout without changing the configured startup default.
An explicit invalid/unreadable directory reports an error without falling back
to another location. Browsing requires a terminal on stdin and stdout;
help/version also work in scripts. Errors return exit code 2.

## Browsing

| Keys | Action |
| --- | --- |
| Up/Down or k/j; Home/End/PageUp/PageDown | Move the cursor |
| Enter; Backspace or Alt+Up | Enter a directory; go to its parent |
| Alt+Left / Alt+Right | Back/forward in this pane's session history |
| Alt+P; Tab/Shift+Tab | Toggle one/two panes; focus the other visible pane |
| Ctrl+Left / Ctrl+Right | Move divider left/right; drag the central border with the mouse |
| Alt+M | Temporarily maximise active pane / restore its previous split |
| Space; Insert | Mark/unmark; mark/unmark and advance |
| Ctrl+A / Ctrl+U | Select all / clear marks in the visible working set |
| /; Enter; Esc | Edit fuzzy filter; accept; cancel/clear |
| h; i | Toggle hidden entries; cycle ignore dim/hide/off |
| s; v; g; r | Cycle sort; reverse; cycle directory grouping; refresh |
| Alt+I; Alt+Enter | Toggle metadata sidebar; open full metadata |
| F2 | Toggle preferred metadata presentation |
| V | View options: arrows, Space/Enter to change, Esc to close; mouse supported |
| Alt+V | View profiles: Enter applies to active; Space selects explicit apply-to-both |
| c; Esc | Count an ordinary directory's descendants; cancel count/close overlay |
| ?; q/F10/Ctrl+C | Help; quit without replacing the saved navigation session |
| Q | Save current navigation session and quit |

Mouse clicks focus a pane and position its cursor. Double-click enters a
directory; files stay unopened. The wheel moves the pane under the pointer
without changing focus. Help and metadata support keyboard scrolling. The
footer's enabled labels use the same actions as the keyboard.

The two panes retain independent locations, cursor, marks, filter and scroll
when hidden. Narrow terminals show the active pane and restore requested dual
view when width permits. A single-pane sidebar similarly returns when there is
space. Columns shrink with terminal width; names take priority over optional
metadata. Plain Unicode, explicit marks, active-pane labels and reverse-video
cursor keep the browser usable without a special font or colour. The active
cursor uses `>` and reverse video; the other pane's retained cursor uses `:`
and bold text. Both keep the terminal's default colours and show `*` when marked.

Natural name order with directories first is the default. Other sort modes are
name, extension, size (largest first), modified/created time (newest first),
type, version and filesystem order. Reverse and directory grouping are separate.
Hidden entries start off; Git-ignore awareness starts in dim mode. Refresh
reloads nested/global/linked-worktree rules. Rule errors remain visible as
unknown status. Hidden entries carry `[h]`; ignored filenames use dim styling
while type indicators, marks and metadata retain their normal styles.

Marked entries outside the current hidden/ignore/filter rules remain marked but
are excluded from the visible scope. Counts distinguish those exclusions;
select-all and clear affect only visible entries. Zero matches have no cursor
fallback. Refresh removes marks for missing/replaced objects conservatively.

Metadata is loaded in bounded background work; late results cannot replace the
current selection. Size/item totals run only on request, stop cooperatively and
report incomplete scans. They never recurse into links, junctions or other
reparse points. Interactive navigation into supported directory links remains
available; unsupported reparse objects stay visible with an explanation.

Each pane reports its visible item count separately from selection. Excluded
selection counts refer only to marked entries hidden by visibility or filtering,
with hidden, ignored and filtered reasons counted once in that order. Narrow
panes shorten the summary or use a second row to retain selection counts.

The `V` View popup includes **Owner/group display: Names / Numeric**. On Unix,
names use cached account lookups with numeric fallback; clicking an entry's
owner/group data cell also toggles that pane. Windows ownership remains
unavailable. `owner_display = "names"` (default) or `"numeric"` also works in
TOML settings and named view profiles. Explicit changes update restart defaults;
switching panes does not change those defaults.

## Navigation sessions

`q`, F10 and Ctrl+C never replace a saved navigation session. `Q` saves the two
current local pane locations, active pane, desired one/two-pane layout, split
ratio and temporary maximise state, then quits. History, marks, selection,
filters and view profiles are excluded. Saving happens after terminal restoration.

Use `tfm --restore-session` to restore explicitly, or `restore_session = true`
in preferences to restore only when no directory paths are supplied. Explicit
paths always override configured restoration; combining paths with the explicit
restore flag is rejected. `tfm --save-session [-2] [LEFT [RIGHT]]` validates and
saves those startup locations/layout, then exits without opening a terminal.
With no paths, it uses current startup locations (including opted-in restoration).
`-2` overrides a restored single/maximised layout for that invocation.

Sessions use `tfm/session.toml` beside config, with a strict version-1 schema.
Native Unix bytes or Windows UTF-16 units retain non-Unicode paths; sessions
are platform-specific. Writes use the same synced, atomic regular-file staging
as preferences. No session file is created by ordinary browsing. Missing,
deleted, unreadable or invalid saved locations fail visibly; there is no silent
fallback to another directory. Correct the location or supply explicit paths.

## Preferences

No config file is created just by browsing. Deliberate changes to sidebar,
sorting, grouping, hidden or ignore preferences are saved atomically on normal
exit. Each preference keeps its last explicit change from either pane; switching
panes never replaces it. Pane policies remain independent during the session.
Marks, filters and history are not saved. Navigation sessions are separate and opt-in.

The config is `tfm/config.toml` under `$XDG_CONFIG_HOME` (or `~/.config`) on
Linux, `$XDG_CONFIG_HOME` (or `~/Library/Application Support`) on macOS, and
`%APPDATA%` on Windows. Invalid settings or key conflicts report errors;
linked/nonregular config files are rejected.

```toml
sidebar = true
restore_session = false # opt-in navigation restore when no paths are supplied
two_panes = false # -2/--two-panes overrides this for one invocation
split_percent = 50 # 20–80; each visible pane retains at least 24 columns
sidebar_left = false
human_sizes = true # false shows exact bytes; human values use B/K/M/G/T/P
fuzzy_timestamps = false # true uses relative phrases, including future times
permissions = "symbolic" # or "octal" on Unix; Windows keeps native attributes
metadata_presentation = "popup" # F2 preference; or "sidebar"
mouse = true
sort = "natural"
grouping = "first"
ignore = "dim"

[columns]
size = true
modified = true
attributes = true
owner = true

[bindings]
"F12" = "toggle-panes"
"Ctrl+f" = "filter"
```

View profiles contain formatting, columns, sorting, visibility and an optional
sidebar preference. Compact, Detailed and Minimal are built in. Alt+V also
offers **Save active as…**: enter a name and press Enter to save explicitly.
Changes show **Modified: name** until saved; they never change a saved profile
implicitly. Profiles contain no directories, marks, cursor, filter or history.
User profiles use `[profiles.NAME]` and optional `[profiles.NAME.columns]` TOML
tables with the view fields above. Sidebar preference follows the active pane;
ordinary pane focus changes never choose restart defaults.

Other preference fields are `reverse` and `hidden` (booleans). Grouping is
`first`, `last` or `mixed`; ignore is `dim`, `hide` or `off`. Sort values are
`natural`, `name`, `extension`, `size`, `modified`, `created`, `kind`, `version`
and `unsorted`. Binding action names also include `up`, `down`, `enter`,
`parent`, `back`, `forward`, `focus-pane`, `mark`, `mark-next`, `select-all`,
`clear-visible`, `toggle-sidebar`, `metadata`, `info`, `count`, `cancel`, `refresh`,
`hidden`, `ignore`, `sort`, `reverse`, `group`, `help` and `quit`.
Chords accept Ctrl/Alt/Shift, a printable character, named navigation keys,
Space, Insert, Esc, or F1–F24. Overrides replace the chosen chord; the centred,
scrollable help lists the effective bindings. The footer shows defaults. Ctrl+C remains an emergency
quit while editing a filter.

Colour detection and capability downgrades use `colored_text`, including its
`NO_COLOR`, `FORCE_COLOR`, `CLICOLOR_FORCE`, `CLICOLOR`, `TERM` and `COLORTERM`
policy. TFM draws Ratatui styles rather than embedding ANSI strings in cells.

## Project status

The M0 engineering foundation, native CI and nonpublishing eight-target build /
installer rehearsals are complete. The M1 implementation is under review;
code and configured tests alone do not establish native acceptance. Safe file
operations/Trash, SSH/SFTP, transfers, persistent navigation integrations and
viewing/search/comparison remain later work. True server-to-server transfer
remains post-v1.

- [Contributing and verification](CONTRIBUTING.md)
- [Architecture and safety contracts](docs/architecture.md)
- [Acceptance evidence and remaining gates](docs/acceptance.md)
- [Reference provenance](docs/provenance.md)
- [Post-v1 work](TODO.md)
- [Security reporting](SECURITY.md)

TFM is licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
See the [exceptional third-party notices](THIRD_PARTY_NOTICES.md) and
[source attribution](https://github.com/seapagan/terminal-file-manager/blob/feat/m1-local-browsing/docs/provenance.md).
