# Acceptance gates

## Current status and evidence

M0's engineering foundation is complete at the accepted checkpoints below. The
application includes the M1 local browser and the M2 implementation in progress below.
The maintainer merged M1 in [PR #2](https://github.com/seapagan/terminal-file-manager/pull/2)
on 18 October 2026. M2 is in progress; M2–M8 cover the remaining
application and final release. Genuine direct SSH-to-SSH transfer
remains post-v1 work.

| Checkpoint | Completed proof |
| --- | --- |
| M0-S: feasibility | [Selected dependency graph](m0-dependency-feasibility.md) built and ran on all eight native targets at `1cf5408`, [run 38163834667](https://github.com/seapagan/terminal-file-manager/actions/runs/38163834667) |
| M0-B: CLI and quality foundation | Native Linux/macOS/Windows Rust, exact MSRV, package and policy checks; [Rust CI at `67a25ac`](https://github.com/seapagan/terminal-file-manager/actions/runs/38783314282) and [manual Python support CI at `b3c31aa`](https://github.com/seapagan/terminal-file-manager/actions/runs/38625636018) |
| M0-C: distribution and installers | Eight minimal-binary builds, ABI/object checks, real-archive installer acceptance and the sixteen-file aggregate at `b3c31aa`, [run 38625643530](https://github.com/seapagan/terminal-file-manager/actions/runs/38625643530) |
| M0-D: delivery and hosted proof | Signed foundation delivery and hosted acceptance; [PR #1](https://github.com/seapagan/terminal-file-manager/pull/1) retains the review history. Merged into main at `c84557a`; publication remains separately authorized |

The eight-target rehearsal passed on 2026-10-14. It covered both Linux
architectures with GNU and MUSL, both macOS architectures, and Windows x64/ARM64
with PowerShell 5.1 and 7. The later Unix execution diagnostic and documentation
updates have no independent eight-target rerun. They do not replace the recorded
checkpoint or certify final release artifacts.

## M1 implementation and current proof

The browser supports cwd/explicit-path startup, two independent session panes,
width- and cached-content-based columns, single/dual/sidebar restoration,
all local sort/grouping modes, hidden/ignore/filter policies, visible-only marks
and counts, session history,
metadata popup/sidebar and cancellable on-demand totals. File mutations, SSH,
viewer/editor and automatic launching are unavailable by design. Preferences
are lazy/atomic. `-2`, two explicit paths or `two_panes = true` request dual
startup. Navigation-session restoration is opt-in; `Q` saves both locations/layout
and quits. Ordinary quit preserves the saved session. Shell cwd handoff remains
future work.

### M1 closure checkpoint, 18 October 2026

PR #2 merged at `0ea80f185dc9adad2385304c0dfc404a22236b02`, with final branch
head `6a755883ac3308e87eee6d6ee8f4dfedff0b7888`. Its five final workflows
succeeded: [Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39130775083),
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39130775084),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39130775085),
[Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39130775088)
and [Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39130775109).
Native suites passed **Linux 284 / macOS 280 / Windows 272**. Each host reported
one intentional skipped `terminal_child` entry point, exercised by its parent
restoration tests through PTY/ConPTY.

The maintainer's final local record reports unchanged clean-tree
`RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` success, including exact MSRV,
package/security/static/maintainability/coverage gates and mandatory Linux xwin
check/Clippy/build. Xwin supplies cross-build evidence, not native Windows
execution. Final Codacy static analysis and diff coverage succeeded; the
maintainer accepted coverage variation of −3.0% against the −1.0% target.
Lizard 1.23.0's `permissions::file_type` 55/50 result remains an accepted parser
false positive. No threshold, scanner or dependency-policy exception changes
follow from this record; see [provenance](provenance.md).

These results belong to the final M1 head. On 19 October, upstream `main` was
checked at `10a2682c72de5c6428ae911ea07194d994b6822a`, the README rewrite dated
18 October in Git. Neither that documentation commit nor this reconciliation
constitutes a fresh foreign-native or hosted run.

### Early execution checkpoint, 16 October 2026

Local Linux `cargo make verify` at `d59281b` passed exact Rust 1.95.0,
package/source/security gates, Ruff/mypy/static support checks, strict Rust
maintainability, Lizard 1.23.0 and compulsory xwin check/Clippy/build. Its
44 passed native cases and one owned child-entry skip describe that early
checkpoint only; the final M1 counts above supersede them for closure.

| Evidence | Scope and limits |
| --- | --- |
| State/input/TestBackend | Independent panes/history, visible/excluded marks, empty filter results, stale list/metadata/totals rejection, configurable keys/mouse, 1–200-column Unicode/control-name/monochrome rendering |
| Linux filesystem | Natural and other sorts, nested/global/linked-worktree ignore refresh/precedence, invalid-byte identity, links/broken links/modes, no-follow cancelled/incomplete/overflow totals |
| Native Linux PTY | Real cwd/explicit-path executable, file non-launch, simultaneous resize/input, quit, returned-error and unwind restoration, paired alternate/mouse/paste/keyboard modes |
| Windows source/cross-build | Hidden attributes, junction/broken target/trailing separator, symlink/native-ID replacement, failed queries and lossless native UTF-16/drive/UNC normalization regressions compile under xwin; this is not native execution |
| Hosted native acceptance | Existing Linux/macOS/Windows jobs run the real application tests and native PTY/ConPTY; final results at `6a75588` are recorded above; configuration and cross-compilation alone do not establish execution |

The native Linux PTY check exercised the real executable with an owned fixture:
listing files, toggling dual panes, opening help and quitting.

### Early local coverage and complexity checkpoint, 16 October 2026

The early Linux production coverage record after the reviewed fixes was 1,720/2,052
lines (83.82%), 198/231 functions (85.71%) and 2,679/3,264 regions (82.08%).
LLVM supplied no branch data. This is native Linux evidence, not combined
coverage or proof of Windows-only paths. No new coverage floor was introduced.

At that early checkpoint, exact Lizard 1.23.0 reported no advisory findings under
10/50/8/500 across the maintained Rust/Python sources and tests. Rust maxima
were CCN 10, function NLOC
49, parameters 6 and file NCSS 363. Strict Clippy maintainability passed.
These historical maxima do not describe the final M1 source; its accepted
Lizard anomaly and observed Codacy dispositions are recorded above.

### Measured M1 baseline — 16 October 2026

Measurements used a Linux x86_64 host (kernel 7.0.0-38, glibc 2.43), local warm
filesystem and unoptimized builds; fixture creation is excluded. The listing /
filter measurements use Rust 1.95.0 with the reviewed refresh/sort fixes. Startup
uses the unchanged terminal path measured with Rust 1.99.0 at `d59281b`. These are descriptive measurements, not acceptance thresholds or
macOS/Windows/remote-heavy workload proof.

| Operation | Five-sample median | Observed range |
| --- | --- | --- |
| Snapshot/lstat/ignore, 10,000 owned regular files | 19.440 ms | 19.057–20.871 ms |
| Install + natural sort + visible working set, same files | 7.755 ms | 7.596–7.962 ms |
| Native fuzzy filter to one matching entry, same files | 2.315 ms | 2.303–2.352 ms |
| Spawn through first populated listing, native PTY / five entries | 6.039 ms | 5.078–6.505 ms |

The final-review cached-snapshot reproduction on Rust 1.95.0 measured marked
refresh at 2,875.378 ms before the fix and 18.305 ms after it (10,000 marked
entries; snapshot clone excluded). Reconciliation now makes one entries pass
with native-path hash lookups. Per-snapshot enumeration ordinals restore Unsorted
transitions independently of directory grouping, cursor or marks. Focused tests
also cover order replacement on refresh and missing/replaced/unavailable identity.

Listing/filter measurements call the actual public fs/app APIs, assert 10,000
entries before filtering and one afterward, over five fresh App instances.
Startup starts its timer before PTY fork/exec and stops when the listing marker
is observed. It includes terminal query responses; the owned config is absent.
No sleeps or external filesystem/network data are used. Numeric performance,
memory and subsystem coverage budgets remain a later reviewed decision.

Remaining acceptance distinctions: final native macOS/Windows suites include
PTY/ConPTY and applicable platform tests. UNC normalization/CLI identity tests
do not establish access to a live SMB server.
Representative real terminals/multiplexers, minimum OS versions, all eight
release artifacts and final shipping graph certification retain M7/M8 ownership.
M1 closure rests on the merged PR and its observed native/hosted evidence;
these later release and compatibility limits remain open.

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

## M2: implementation in progress

The browser now exposes frozen copy/move plans, a two-worker queue, recursive
merge and conflicts, verified staging, create/rename/attributes, native Trash,
counted permanent-delete confirmation, retained job results, reconciliation and
safe exit.
Local regressions include real executable/PTY creation, copy, conflict replacement,
and permanent-delete cancellation/confirmation. Linux Trash tests use an owned
XDG data root, sticky/private per-volume layouts and an owned file on a separate
real filesystem; no cross-device copy/delete fallback is allowed. Windows/macOS native Trash
and Read-only behavior still require hosted execution for this implementation.

M2 is not accepted yet. Completed-tree full gates and native hosted outcomes
remain outstanding. Reconciliation tests cover retained sources, committed
destinations, owned staging and unresolved evidence without repeating removals.
Preserve the three-layer mutation contract in [architecture](architecture.md).

The existing Theme/Role boundary supplies all operation styles. Selectable
palettes, OSC 11/background detection and theme configuration remain later work.

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
| M1 (merged) | Local panes, raw identity, natural sorting, selection/filters, key protocol and terminal lifecycle; Unix PTY and Windows ConPTY, resize and restored state |
| M2 (in progress) | Shared operation planner/conflicts, two-job queue, staging/commit/source-removal and restricted Trash; race/fault/cancel/uncertainty/source-preservation tests |
| M3 | Embedded byte-preserving SFTP; trust/auth/config, opaque handles, short writes/acknowledged close, bounded pooling/teardown and native clients with real isolated server fixtures |
| M4 | Local/SSH transfer and explicit labelled client relay; validation, honest sent-request uncertainty, safe retry/removal; no genuine direct route |
| M5 | Navigation/history/bookmarks, optional zoxide/fzf/clipboard/shell integration; missing capabilities, exact payloads, isolation, no implicit shell configuration changes |
| M6 | Bounded incremental viewer, recursive filename search and directory comparison; huge files/lines, decoder/range boundaries, cancellation and incomplete/error results |
| M7 | Full UX/native compatibility, settings/key conflicts, colour/monochrome policy and representative terminals/multiplexers |
| M8 | All v1 acceptance, eight artifacts/installers/native ABI, measured budgets, immutable publishing rehearsal and separately authorized production publication |

No future seam constitutes a passing feature test. Restricted Trash remains
required; true direct transfer is a post-v1 TODO. Resume, synchronization,
editing and other deferrals are listed in [TODO](../TODO.md).
