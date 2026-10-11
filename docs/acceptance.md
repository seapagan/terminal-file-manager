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

Canonical packaging rejects uncommitted packaged files. Changes limited to
excluded support files can pass without a clean tracked tree; that is local
package evidence, not signed-delivery evidence. `--allow-dirty` is only interim
evidence. Hosted/native results are not implied by configuration files.

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

### M0-C2: secure consumer installers (D2–D8)

- Retain all 58 Labeldeck Unix control-flow cases and 13 portable PowerShell
  cases, plus its native success group. Separate synthetic header/control-flow
  fixtures from real native binary successes. Use platform-native fixture hashes;
  reject checksum errors before extraction, header inspection or execution.
- Require stable tags, HTTPS remote transport, exact one-record/two-field/name
  sidecars and exactly four root-level regular archive members. Reject unsafe
  names, duplicates, links and special types before extracting into owned staging.
- Validate native executable architecture with bounded reads, then require exact
  successful `tfm --version`. Per the revised C2 decision, this is synchronous
  without a deadline; no extra supervisor/runtime prerequisite. Native acceptance
  must check prompt termination. No descendant-containment claim is made.
- Reject linked/reparse/non-directory ancestors and unsafe final objects;
  recheck destination identity before commit. Preserve old bytes on precommit
  failure. Pathname checks narrow races but cannot eliminate every path race.
- Windows retains PowerShell 5.1/7+, native x64/ARM64 discovery under emulation,
  exclusive destination-local staging, no-overwrite fresh move and `File.Replace`
  with an owned backup. Failed recovery retains the last good copy and reports
  its exact path. Native locking, read-only, denied paths and reparse tests must
  run on Windows before transaction semantics are accepted.
- Full host verification includes installer tests and Unix ShellCheck alongside
  every M0-B/C1 gate; `verify-fast` and local compulsory Linux xwin are unchanged.
  Native CI uses independent Windows invocations. No hosted xwin is added.
- C2's delivered native Windows x64 PowerShell 5.1/7+ suites passed at `df3919a`,
  including running/locked-file preflight, old bytes, cleanup-failure rollback
  and retained-backup/manual recovery. macOS Apple Silicon also has C2 native
  fixture proof. ARM64 Windows/emulation, macOS Intel and real release archive
  acceptance remain separate cells.

### M0-C3: eight-target nonpublishing distribution rehearsal (D9/D10)

- Source-side archive validation is mandatory before each upload, with the
  verified built executable supplied through `--binary`. Foreign aggregate
  validation omits that argument and executes no archived code.
- Validate exact package/version/target naming, a single lowercase SHA256 record
  with two spaces and exact basename, corruption, exact root regular members,
  duplicates/aliases/links/special types, Unix executable mode, machine headers,
  checkout licence/README bytes and packaged build bytes. Reads are chunked;
  no untrusted member is extracted by the source-side helper.
- Native macOS hashing uses `shasum -a 256`, retaining the Linux aggregate's
  existing wire format and checksum block. Existing secure Rustup bootstrap,
  sixteen-file inventory, all verifiers/strip/smoke and two-file one-day uploads
  remain unchanged. All new tests join existing Ruff ALL/strict mypy/Lizard
  policy and full verification; `verify-fast` and local-only xwin are preserved.
- Each target has offline fresh/reinstall/corruption-preservation acceptance
  using its actual archive and sidecar, unmodified candidate checks and real
  prompt version execution. Each Linux libc is explicitly selected. Controlled
  Linux images execute both unstripped and stripped builds with a ten-second
  test deadline; GNU execution must occur inside GLIBC 2.28.
- Required hosted cells remain pending until separately authorized delivery and
  dispatch: all four Linux targets (including both ARM64 libcs), macOS Intel
  and ARM64, Windows x64 and ARM64 with both PowerShell 5.1/7+, per-target native
  regressions/real-archive installs/uploads and complete aggregate. Prior C2
  fixture success does not establish these C3 release-artifact cells.
- This checkpoint authorizes local implementation evidence only. No dispatch,
  publishing, tags, attestations or settings change. Stop for implementation
  review and separate signed-delivery/manual-dispatch approval.

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
