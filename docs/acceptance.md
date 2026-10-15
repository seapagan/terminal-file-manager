# Acceptance gates

## Current status and evidence

M0's engineering foundation is complete at the accepted checkpoints below. The
application still implements only help, version and argument errors. M1–M8
cover the file manager and its final release; genuine direct SSH-to-SSH transfer
remains post-v1 work.

| Checkpoint | Completed proof |
| --- | --- |
| M0-S: feasibility | [Selected dependency graph](m0-dependency-feasibility.md) built and ran on all eight native targets at `1cf5408`, [run 38163834667](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667) |
| M0-B: CLI and quality foundation | Native Linux/macOS/Windows Rust, exact MSRV, package and policy checks; [Rust CI at `67a25ac`](https://github.com/seapagan/terminal-file-manager/actions/runs/38783314282) and [manual Python support CI at `b3c31aa`](https://github.com/seapagan/terminal-file-manager/actions/runs/38625636018) |
| M0-C: distribution and installers | Eight minimal-binary builds, ABI/object checks, real-archive installer acceptance and the sixteen-file aggregate at `b3c31aa`, [run 38625643530](https://github.com/seapagan/terminal-file-manager/actions/runs/38625643530) |
| M0-D: delivery and hosted proof | Signed foundation delivery and hosted acceptance; [PR #1](https://github.com/seapagan/terminal-file-manager/pull/1) retains the review history. Merge and publication require separate authorization |

The eight-target rehearsal passed on 2026-10-14. It covered both Linux
architectures with GNU and MUSL, both macOS architectures, and Windows x64/ARM64
with PowerShell 5.1 and 7. The later Unix execution diagnostic and documentation
updates have no independent eight-target rerun. They do not replace the recorded
checkpoint or certify final release artifacts.

### M0-S: feasibility decisions and limits

The selected-dependency probe established an embedded russh/ring and
byte-preserving Yazi SFTP direction. Production adoption still needs a maintained
fork, the opaque HANDLE correction, licence review and the M3 lifecycle gates.
The handover preserves accepted Windows SDK-inventory and compiler-minimum
limitations. The product's declared Rust floor remains 1.95.0.

Early Linux experiments established raw-path requirements, sent-request
uncertainty and the restriction to same-filesystem Trash that preserves link
identity. Linux PTY parser/restoration fixtures passed ten cases; they did not
establish ConPTY or representative-terminal behaviour. Those contracts remain
in [architecture](architecture.md) and the M1/M2/M3/M7 acceptance below. Retain
Lizard 1.23.0; a parser upgrade needs its separate compatibility gate.

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
- Native Rust and exact MSRV CI, plus manual Python support CI; all eight
  targets retained in a manual nonpublishing release rehearsal.

Canonical packaging rejects uncommitted packaged files. Changes limited to
excluded support files can pass without a clean tracked tree; that is local
package evidence, not signed-delivery evidence. `--allow-dirty` is only interim
evidence. Hosted/native results are not implied by configuration files.

## M0-C: completed distribution contracts

### M0-C1: binary verifiers and D1 regressions

- Preserve the inherited Labeldeck ELF/PE contracts and original 20 ELF /
  seven PE cases. The ELF verifier additionally requires ELF64 and little-endian
  data. Retain fail-closed input/type/inspection/argument, MUSL-symbol and
  PE-signature/offset cases.
- Characterize both actual inline Mach-O workflow blocks, including x86_64 and
  arm64 positives, architecture/type mismatches and failed `file` inspection.
- `cargo make verify-infra` runs `release-verification-test`: ELF and Mach-O on
  Linux, Mach-O on macOS, PE on Windows. It also runs Unix `release-shellcheck`.
  Explicit host guards reject wrong-host tasks; missing required tools fail.
  `verify-fast` remains unchanged; full `verify` retains compulsory Linux xwin
  and static script checks. Native Rust CI tests application code; infrastructure
  regressions run locally or in the manual release rehearsal.
- Supplemental Linux PowerShell execution proves synthetic PE parsing only.
  The completed native rehearsal above supplies the separate macOS/Windows,
  ARM object and eight-target build proof.

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
- `cargo make verify-infra` includes host-selected installer tests and Unix
  ShellCheck. The manual rehearsal uses independent Windows invocations.
  `verify-fast`, full `verify` and compulsory local Linux xwin are unchanged.
  Unix installer `--help` returns before platform detection or downloads;
  unsupported arguments fail before installation. Configuration stays in the
  environment. Candidate launch failures include conditional executable-`TMPDIR`
  guidance; the diagnostic does not identify `noexec` as the sole cause. No hosted
  xwin is added.
- C2's delivered native Windows x64 PowerShell 5.1/7+ suites passed at `df3919a`,
  including running/locked-file preflight, old bytes, cleanup-failure rollback
  and retained-backup/manual recovery. macOS Apple Silicon also has C2 native
  fixture proof. The later eight-target rehearsal completed Windows ARM64,
  native architecture-discovery, macOS Intel and real release-archive cells.

### M0-C3: eight-target nonpublishing distribution rehearsal (D9/D10)

- Source-side archive validation is mandatory before each upload, with the
  verified built executable supplied through `--binary`. Foreign aggregate
  validation omits that argument and executes no archived code.
- Validate exact package/version/target naming, a single lowercase SHA256 record
  with two spaces and exact basename, corruption, exact root regular members,
  duplicates/aliases/links/special types, Unix executable mode, machine headers,
  checkout licence/README bytes and packaged build bytes. Source-side validation
  caps archives at 256 MiB before hashing, executables at 128 MiB before reading
  payloads, and trailing tar zero padding at 64 KiB. The recorded M0 archives
  were below 220 KiB. These bounds also apply to aggregate validation; documents
  retain exact checkout-size limits. Reads are chunked; no untrusted member is
  extracted by the source-side helper.
- Native macOS hashing uses `shasum -a 256`, retaining the Linux aggregate's
  existing wire format and checksum block. Existing secure Rustup bootstrap,
  sixteen-file inventory, all verifiers/strip/smoke and two-file one-day uploads
  remain unchanged. All new tests join existing Ruff ALL/strict mypy/Lizard
  policy and `verify-infra`; `verify-fast`, full `verify` and local-only xwin
  are preserved. Python support CI remains available through manual dispatch.
- Each target has offline fresh/reinstall/corruption-preservation acceptance
  using its actual archive and sidecar, unmodified candidate checks and real
  prompt version execution. Each Linux libc is explicitly selected. Controlled
  Linux images execute both unstripped and stripped builds with a ten-second
  test deadline; GNU execution must occur inside GLIBC 2.28.
- Run 38625643530 completed all target jobs, per-target native regressions,
  real-archive installations, uploads and the aggregate. Its real-archive
  acceptance covers fresh install, reinstall and corruption preservation.
  Earlier C2 fixtures remain separate parser/control-flow evidence.
- The rehearsal publishes no release, tag or attestation. Future dispatch,
  publication and settings changes require their own authorization.

### Final-release verification still required

M0's minimal application and selected-dependency probe have separate accepted
native evidence. Neither certifies the future shipping graph or completed
file-manager behaviour. M8 must repeat eight-target locked builds, ABI/object
checks and installer acceptance for the release artifacts, review adopted
dependency licences/notices, measure the agreed budgets, and verify immutable
publication before authorized production release. Minimum-OS fields in objects
do not establish execution on those older systems. No public release exists yet.

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
