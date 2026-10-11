# Acceptance gates

## M0-B: local foundation

- Exact package/bin/licence metadata and lockfile; help/version/error CLI smoke
  tests plus a documented library seam. No filesystem-manager or SSH features.
- Reference Rust format/check/Clippy/nextest/doctest/doc/build/package tasks;
  manifest-derived exact 1.95.0 full compiler gate; strict maintainability.
  Full Linux verification also requires reference x86_64 MSVC xwin check,
  warning-denying Clippy and build; fast verification excludes xwin.
- Required Python 3.10+ Ruff ALL/strict mypy/unittest and actual Lizard 1.23.0
  checker. All 18 canonical assertions and distinct legacy regressions remain.
  Raw-name/XML/CSV/source-reconciliation corrections require adversarial tests.
- Complexity limits: CCN 10, function NLOC 50, parameters 8, file NLOC 500.
  Equality passes; findings are advisory and need checkpoint disposition;
  machine/schema/configuration/source failures block. No absent-tool success.
- Valid nonempty normalized production LCOV records and measured baseline.
  Set meaningful subsystem/platform floors after M1 measurements and review.
- Audit, strict deny, weekly policy, actionlint/ShellCheck, pedantic Zizmor;
  SHA-pinned actions and verified executable downloads. Renovate is the sole
  dependency updater. CI never depends on ignored private planning sources.
- Native Rust CI, separate Python support CI and exact MSRV CI; all eight
  targets retained in a manual nonpublishing release framework.

Before authorized Git delivery the canonical clean-tree package gate is blocked
by intentional uncommitted files. `cargo package --locked --allow-dirty` is only
interim evidence. Hosted/native results are not implied by configuration files.

## M0-C / M0-D: remaining foundation proof

### M0-C1: binary verifiers and D1 regressions

- Preserve the complete pinned Labeldeck ELF/PE helpers and original 20 ELF /
  seven PE cases. ELF production bytes remain unchanged; PE adapts the usage
  comment only. Add fail-closed input/type/inspection/argument, MUSL-symbol and
  PE-signature/offset cases without changing production policy.
- Characterize both actual inline Mach-O workflow blocks, including x86_64 and
  arm64 positives, architecture/type mismatches and failed `file` inspection.
- Full local verification adds `release-verification-test`: ELF and Mach-O on
  Linux, Mach-O on macOS, PE on Windows. Unix full verification also runs
  `release-shellcheck`. Explicit host guards reject wrong-host tasks; missing
  required tools fail. `verify-fast` remains unchanged; Linux full verification
  retains compulsory xwin. Native Rust CI runs the same host-selected test task
  independently; its existing Linux quality job lints the shell scripts.
- Supplemental Linux PowerShell execution proves synthetic PE parsing only.
  Native macOS/Windows CI, real ARM objects and all eight rehearsal builds remain
  unproven until separately authorized hosted execution. Stop for C1 review
  before installer implementation.

### Remaining distribution proof

M0-C adds actual executable binary verifiers/installers with full reference
regressions; all eight locked builds, pre/post-strip ELF/MUSL/PE/Mach-O checks,
GNU glibc <=2.28, suitable native smoke and real-asset installer negative/atomic
replacement tests. Scaffold-only success does not prove the future crypto graph.
M0-D follows separately authorized Git delivery: actual hosted native/QC/rehearsal
results and agreed stable check names. Settings/publication require separate
permission. No publish/tag/release action belongs to M0-B.

## V1 milestones

| Milestone | Required behaviour and proof |
| --- | --- |
| M1 | Local panes, raw identity, natural sorting, selection/filters, key protocol and terminal lifecycle; Unix PTY and Windows ConPTY, resize and restored state |
| M2 | Shared operation planner/conflicts, two-job queue, staging/commit/source-removal and restricted Trash; race/fault/cancel/uncertainty/source-preservation tests |
| M3 | Embedded byte-preserving SFTP; trust/auth/config, opaque handles, short writes/acknowledged close, bounded pooling/teardown and native clients with real isolated server fixtures |
| M4 | Local/SSH transfer and explicit labelled client relay; validation, honest sent-request uncertainty, safe retry/removal; no genuine direct route |
| M5 | Navigation/history/bookmarks, optional zoxide/fzf/clipboard/shell integration; missing capabilities, exact payloads, isolation, no implicit shell configuration changes |
| M6 | Bounded incremental viewer, recursive filename search and directory comparison; huge files/lines, decoder/range boundaries, cancellation and incomplete/error results |
| M7 | Full UX/native compatibility, settings/key conflicts, colour/monochrome policy and representative terminals/multiplexers |
| M8 | All v1 acceptance, eight artifacts/installers/native ABI, measured budgets, immutable publishing rehearsal and separately authorized production publication |

No future seam constitutes a passing feature test. Restricted Trash remains
required; true direct transfer is a post-v1 TODO. Resume, synchronization,
editing and other deferrals are listed in [TODO](../TODO.md).
