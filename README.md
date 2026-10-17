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

The package is `terminal-file-manager`; the command is `tfm`. Each invocation
starts in a single pane at the shell's current directory, or the supplied
native directory path. Use `tfm -- -directory` for names starting with a hyphen.
An explicit invalid/unreadable directory reports an error without falling back
to another location. Browsing requires a terminal on stdin and stdout;
help/version also work in scripts. Errors return exit code 2.

## Browsing

| Keys | Action |
| --- | --- |
| Up/Down or k/j; Home/End/PageUp/PageDown | Move the cursor |
| Enter; Backspace or Alt+Up | Enter a directory; go to its parent |
| Alt+Left / Alt+Right | Back/forward in this pane's session history |
| Ctrl+P; Tab | Toggle one/two panes; focus the other visible pane |
| Space; Insert | Mark/unmark; mark/unmark and advance |
| Ctrl+A / Ctrl+U | Select all / clear marks in the visible working set |
| /; Enter; Esc | Edit fuzzy filter; accept; cancel/clear |
| h; i | Toggle hidden entries; cycle ignore dim/hide/off |
| s; v; g; r | Cycle sort; reverse; cycle directory grouping; refresh |
| Alt+I; Alt+Enter | Toggle metadata sidebar; open full metadata |
| F2 | Toggle preferred metadata presentation |
| V | View options: arrows, Space/Enter to change, Esc to close; mouse supported |
| c; Esc | Count an ordinary directory's descendants; cancel count/close overlay |
| ?; q/F10/Ctrl+C | Help; quit without changing the shell directory |

Mouse clicks focus a pane and position its cursor. Double-click enters a
directory; files stay unopened. The wheel moves the pane under the pointer
without changing focus. Help and metadata support keyboard scrolling. The
footer's enabled labels use the same actions as the keyboard.

The two panes retain independent locations, cursor, marks, filter and scroll
when hidden. Narrow terminals show the active pane and restore requested dual
view when width permits. A single-pane sidebar similarly returns when there is
space. Columns shrink with terminal width; names take priority over optional
metadata. Plain Unicode, explicit marks, active-pane labels and reverse-video
cursor keep the browser usable without a special font or colour.

Natural name order with directories first is the default. Other sort modes are
name, extension, size (largest first), modified/created time (newest first),
type, version and filesystem order. Reverse and directory grouping are separate.
Hidden entries start off; Git-ignore awareness starts in dim mode. Refresh
reloads nested/global/linked-worktree rules. Rule errors remain visible as
unknown status. Hidden entries carry `[h]`; ignored entries use dim styling.

Marked entries outside the current hidden/ignore/filter rules remain marked but
are excluded from the visible scope. Counts distinguish those exclusions;
select-all and clear affect only visible entries. Zero matches have no cursor
fallback. Refresh removes marks for missing/replaced objects conservatively.

Metadata is loaded in bounded background work; late results cannot replace the
current selection. Size/item totals run only on request, stop cooperatively and
report incomplete scans. They never recurse into links, junctions or other
reparse points. Interactive navigation into supported directory links remains
available; unsupported reparse objects stay visible with an explanation.

## Preferences

No config file is created just by browsing. Deliberate changes to sidebar,
sorting, grouping, hidden or ignore preferences are saved atomically on normal
exit. Each preference keeps its last explicit change from either pane; switching
panes never replaces it. Pane policies remain independent during the session.
Pane locations, marks, filters, history and startup layout are not saved.

The config is `tfm/config.toml` under `$XDG_CONFIG_HOME` (or `~/.config`) on
Linux, `$XDG_CONFIG_HOME` (or `~/Library/Application Support`) on macOS, and
`%APPDATA%` on Windows. Invalid settings or key conflicts report errors;
linked/nonregular config files are rejected.

```toml
sidebar = true
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
