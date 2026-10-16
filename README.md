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
exit. Pane locations, marks, filters, history and startup layout are not saved.

The config is `tfm/config.toml` under `$XDG_CONFIG_HOME` (or `~/.config`) on
Linux, `$XDG_CONFIG_HOME` (or `~/Library/Application Support`) on macOS, and
`%APPDATA%` on Windows. Invalid settings or key conflicts report errors;
linked/nonregular config files are rejected.

```toml
sidebar = true
sidebar_left = false
mouse = true
sort = "natural"
grouping = "first"
ignore = "dim"

[bindings]
"F12" = "toggle-panes"
"Ctrl+f" = "filter"
```

Other preference fields are `reverse` and `hidden` (booleans). Grouping is
`first`, `last` or `mixed`; ignore is `dim`, `hide` or `off`. Sort values are
`natural`, `name`, `extension`, `size`, `modified`, `created`, `kind`, `version`
and `unsorted`. Binding action names also include `up`, `down`, `enter`,
`parent`, `back`, `forward`, `focus-pane`, `mark`, `mark-next`, `select-all`,
`clear-visible`, `toggle-sidebar`, `metadata`, `count`, `cancel`, `refresh`,
`hidden`, `ignore`, `sort`, `reverse`, `group`, `help` and `quit`.
Chords accept Ctrl/Alt/Shift, a printable character, named navigation keys,
Space, Insert, Esc, or F1–F24. Overrides replace the chosen chord; the help
screen and footer describe the default bindings. Ctrl+C remains an emergency
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

TFM uses the MIT OR Apache-2.0 licence.

## Dependency notices

These notices accompany the locked M1 dependencies, including build/test-only
crates for source redistributors. Dual-licensed dependencies offering Apache-2.0
may be used under the accompanying [Apache licence](LICENSE-APACHE). The MIT
notices below share the permission/warranty text that follows. Adapted lsplus
and Labeldeck source is Copyright (c) Grant Ramsay, under MIT; see provenance.

<details>
<summary>MIT dependency copyright and permission notices</summary>

| Dependency | Copyright notice |
| --- | --- |
| aho-corasick 1.1.5 | Copyright (c) 2015 Andrew Gallant |
| castaway 0.2.4 | Copyright (c) 2021 Stephen M. Coakley |
| cfg_aliases 0.1.1 | Copyright (c) 2020 Katharos Technology |
| colored_text 0.6.0 | Copyright (c) 2025 Grant Ramsay |
| compact_str 0.9.1 | Copyright (c) 2021 Parker Timmerman |
| convert_case 0.10.0 | Copyright (c) 2025 rutrum |
| crossterm 0.29.0 | Copyright (c) 2019 Timon |
| crossterm_winapi 0.9.1 | Copyright (c) 2019 Timon |
| darling 0.24.1 | Copyright (c) 2017 Ted Driggs |
| darling_core 0.24.1 | Copyright (c) 2017 Ted Driggs |
| darling_macro 0.24.1 | Copyright (c) 2017 Ted Driggs |
| derive_more 2.1.1 | Copyright (c) 2016 Jelte Fennema |
| derive_more-impl 2.1.1 | Copyright (c) 2016 Jelte Fennema |
| filedescriptor 0.8.3 | Copyright (c) 2018 Wez Furlong |
| globset 0.4.20 | Copyright (c) 2015 Andrew Gallant |
| ignore 0.4.33 | Copyright (c) 2015 Andrew Gallant |
| instability 0.3.14 | Copyright (c) 2020 Stephen M. Coakley; Copyright (c) The Ratatui Developers |
| lru 0.18.5 | Copyright (c) 2016 Jerome Froelich |
| memchr 2.8.3 | Copyright (c) 2015 Andrew Gallant |
| mio 1.2.4 | Copyright (c) 2014 Carl Lerche and other MIO contributors |
| nix 0.28.0 | Copyright (c) 2015 Carl Lerche + nix-rust Authors |
| portable-pty 0.9.0 | Copyright (c) 2018 Wez Furlong |
| ratatui 0.30.2 | Copyright (c) 2016-2022 Florian Dehau; Copyright (c) 2023-2025 The Ratatui Developers |
| ratatui-core 0.1.2 | Copyright (c) 2016-2022 Florian Dehau; Copyright (c) 2023-2025 The Ratatui Developers |
| ratatui-crossterm 0.1.2 | Copyright (c) 2016-2022 Florian Dehau; Copyright (c) 2023-2025 The Ratatui Developers |
| ratatui-widgets 0.3.2 | Copyright (c) 2016-2022 Florian Dehau; Copyright (c) 2023-2025 The Ratatui Developers |
| redox_syscall 0.5.18 | Copyright (c) 2017 Redox OS Developers |
| same-file 1.0.6 | Copyright (c) 2017 Andrew Gallant |
| strsim 0.11.1 | Copyright (c) 2015 Danny Guo; Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com>; Copyright (c) 2018 Akash Kurdekar |
| strum 0.28.0 | Copyright (c) 2019 Peter Glotfelty |
| strum_macros 0.28.0 | Copyright (c) 2019 Peter Glotfelty |
| tokio 1.53.3 | Copyright (c) Tokio Contributors |
| vsort 0.2.0 | Copyright (c) @juansc |
| walkdir 2.5.0 | Copyright (c) 2015 Andrew Gallant |
| winapi-util 0.1.11 | Copyright (c) 2017 Andrew Gallant |
| winnow 1.0.4 | Upstream MIT text names no copyright holder |
| winreg 0.10.1 | Copyright (c) 2015 Igor Shaula |

```text
Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

The cfg_aliases macro also incorporates MIT-licensed tectonic_cfg_support code.

Additional libm 0.2.16 attribution, covered by the MIT permission above:

```text
This Rust library contains the following copyrights:

    Copyright (c) 2018 Jorge Aparicio

Portions of this software are derived from third-party works licensed under
terms compatible with the above MIT license:

* musl libc https://www.musl-libc.org/. This library contains the following
  copyright:

      Copyright © 2005-2020 Rich Felker, et al.

* The CORE-MATH project https://core-math.gitlabpages.inria.fr/. CORE-MATH
  routines are available under the MIT license on a per-file basis.

The musl libc COPYRIGHT file also includes the following notice relevant to
math portions of the library:

------------------------------------------------------------------------------
Much of the math library code (src/math/* and src/complex/*) is
Copyright © 1993,2004 Sun Microsystems or
Copyright © 2003-2011 David Schultz or
Copyright © 2003-2009 Steven G. Kargl or
Copyright © 2003-2009 Bruce D. Evans or
Copyright © 2008 Stephen L. Moshier or
Copyright © 2017-2018 Arm Limited
and labelled as such in comments in the individual source files. All
have been licensed under extremely permissive terms.
------------------------------------------------------------------------------

Copyright notices are retained in src/* files where relevant.
```

</details>

<details>
<summary>foldhash 0.2.0 — Zlib</summary>

```text
Copyright (c) 2024 Orson Peters

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

2. Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.
```

</details>

<details>
<summary>unicode-ident 1.0.27 — Unicode-3.0</summary>

```text
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2023 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.
```

</details>
