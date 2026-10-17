# Source attribution

TFM incorporates source adapted from Grant Ramsay's MIT-licensed projects.
The source areas below identify direct ports and adaptations. Their copyright
and permission notices remain applicable; see [LICENSE-MIT](../LICENSE-MIT).

## Local browsing

[lsplus](https://github.com/seapagan/lsplus/tree/d30ea4a4d8f382bce1fd761f62b470926906f847),
Copyright (c) Grant Ramsay, supplied the filesystem, metadata, sorting and
Git-ignore reference code and associated native regression cases:
`src/platform/{unix,windows}.rs`, `src/utils/{sort,gitignore}.rs` and
`tests/crate/`. TFM adapts these areas for independent panes, asynchronous
requests, native path identity and explicit errors when rule status is unknown.

Size formatting is a direct port of `lsplus/src/utils/format.rs` at that
revision (`SizeScale`, `human_readable_format`, `show_size` and rounding).
`tests/sizes.rs` retains its seven size regressions from `tests/format.rs`.
The numeric/unit columns adapt `src/utils/render.rs`'s separate right/left
alignment; TFM caches escaped display widths when the working set changes.
TFM uses binary units by default and supports exact bytes in configuration.

Relative timestamps port the complete `src/utils/fuzzy_time.rs` algorithm and
`src/utils/time.rs` constants from the same lsplus revision. All 16 original
`tests/fuzzy_time.rs` cases are retained with an explicit fixed clock instead
of wall-clock sampling. TFM supplies one frame clock and a once-per-second
refresh; thresholds, subsecond truncation, past/future phrases and duration
semantics are unchanged. Exact timestamps retain TFM's UTC precision/fallback.
The original past-bucket function measured CCN 12 against TFM's limit 10;
its unchanged day-and-older branches are extracted into one helper. Boundary
regressions cover both sides of that split and all bucket transitions.

Permission rendering ports `src/utils/format.rs`'s complete `mode_to_rwx`,
`mode_to_octal` and special-bit helpers, plus the type mapping from
`src/platform/unix.rs`/`src/platform/mod.rs` at the same lsplus revision.
The five original permission tests from `tests/format.rs` are retained alongside
native link/special-bit and missing-metadata tests. TFM uses cached full native
mode bits, never follows link targets for permissions, and leaves Windows
attributes intact for either preference. No Unix permissions are fabricated.

Entry/column colours adapt `src/utils/file.rs::colorize_name` and
`src/utils/render.rs`'s permission, size and timestamp style classifiers at that
revision. `tests/colours.rs` adapts special-type/name, ignored executable,
permission-character, raw-size boundary and age-segment regressions from
`tests/crate/{file_unix,file_windows,render,render_unix}.rs`. TFM composes dimming
with type accents and explicit cursor/mark styles. Capability-resolved neutral
accents and indexed age buckets replace RGB gradients; all resolution uses `colored_text`, all output Ratatui.

[Labeldeck](https://github.com/seapagan/labeldeck/tree/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d),
Copyright (c) Grant Ramsay, supplied terminal guards, colour mapping, semantic
styling and the PTY/ConPTY session driver. The relevant sources include
`src/edit/color.rs`, `src/edit/ui/{theme,render}.rs` and
`tests/common/terminal*.rs`. TFM adapts them to its browser state and terminal
restoration contract.

Pane focus roles and buffer regressions originate in Labeldeck's
`src/edit/ui/theme.rs` and `tests/suite/edit/rendering/selection.rs` at that
revision. TFM now keeps both pane backgrounds at terminal defaults and uses
reverse cursors, with bold for active focus. Its neutral RGB accents resolve
through `colored_text` for richer terminals; ANSI-16 retains named colours.

TFM calls `colored_text`'s public detection and resolution APIs as a dependency.
Its internal colour implementation was not copied into TFM.

## Installers and binary verification

The [Labeldeck installer and verifier sources](https://github.com/seapagan/labeldeck/tree/8c0fdea354bf0b0bd8abafb293af78c34ac9fe1d)
are direct MIT-licensed ports, Copyright (c) Grant Ramsay: `install.sh`,
`install.ps1`, `scripts/verify-release-binary.{sh,ps1}` and their installer and
release regression suites. TFM adapts package names, platform handling,
archive validation and installation preservation for its distribution contract.

[Keyhold's installer tests](https://github.com/seapagan/keyhold/blob/0cbd7108cd46aae929b5d39b77ecf3290fa913e9/tests/installers/unix.sh),
Copyright (c) Grant Ramsay, MIT, also supplied checksum-first, existing-file
preservation and libc-override regression cases.

## Engineering framework

TFM's CI/QC, security, testing and distribution framework evolved from
[colored_text](https://github.com/seapagan/colored_text),
[keyhold](https://github.com/seapagan/keyhold),
[git-diff-out](https://github.com/seapagan/git-diff-out) and
[labeldeck](https://github.com/seapagan/labeldeck). Direct ports include the
complexity checker and tests, task/workflow sources and release framework;
TFM has since adapted source reconciliation, coverage merging, package checks
and platform safeguards to its own requirements.

[Acceptance records](acceptance.md), [contributor guidance](../CONTRIBUTING.md)
and Git history hold verification details. Exceptional dependency licence texts
are in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
