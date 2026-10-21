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
real filesystem; no cross-device copy/delete fallback is allowed. Windows/macOS Trash
uses direct Foundation/Shell adapters with hosted-only owned recovery fixtures.
The callback outcome matrix covers missing, contradictory and interrupted
acknowledgements, optional metadata and critical permanent-deletion incidents.

### Native Trash checkpoint, 20 October 2026

At `e0b6184`, [Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39483201039)
passed **Linux 362 / macOS 350 / Windows 338**, plus combined coverage and exact
MSRV. Each host reported two child-entry skips exercised by parent terminal tests.
Native tests recovered owned Linux per-volume payloads, macOS collision-renamed
files and symlinks, and Windows files, recursive trees, symlinks and junctions.
Windows tests used supported Shell moves from opaque recovery references and
verified target preservation. The selected-path parsing regression passed.
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39483200952),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39483201136),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39483201032)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39483201041)
also passed. The unchanged clean-tree local full gate passed, including Linux
xwin check/Clippy/build; xwin remains cross-build evidence.

Codacy cleared the native unsafe-audit findings after approved line-scoped
dispositions. Its existing Lizard false positive in `tests/operation_controls.rs`
remains reported; no threshold or project policy changed.

M2 is not accepted yet. This checkpoint covers the native Trash correction;
it does not establish live UNC/SMB recycling, minimum-OS compatibility or every
mount topology. Reconciliation tests cover retained sources, committed
destinations, owned staging and unresolved evidence without repeating removals.
Preserve the three-layer mutation contract in [architecture](architecture.md).

The existing Theme/Role boundary supplies all operation styles. Selectable
palettes, OSC 11/background detection and theme configuration remain later work.

### M2 remediation source checkpoint, 21 October 2026

Signed checkpoint `0807ae2` separates skipped, failed and critical jobs and guards
contextual retry/reconciliation/cancellation. Its unchanged clean-tree
`RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed, including mandatory Linux xwin.
Signed checkpoints `4983449` and `5417253` add incident exit, operation UX and
behavioural assurance. Each passed an immediate unchanged clean-tree full gate
after its signed commit, including mandatory Linux xwin check/Clippy/build.

Controller tests cover deliberate incident acknowledgement after worker
drain, accidental keys/paste, ordinary unknown outcomes, retained diagnostics,
Q versus q, and replacement of an earlier exit intent. The existing terminal
child fixture exercises acknowledgement and cleanup without provoking OS data
loss. TestBackend/input cases cover content sizing, narrow/tiny terminals,
resize, four colour capabilities, sanitized Unicode paths, mouse hit regions,
contextual controls, error evidence and notice expiry. Jobs opens the selected
job's conflict without submitting either pending decision; its regression first
reproduced the wrong-job routing. The source checkpoint's clean local suite
passed 401 tests, with two owned child-entry skips.

Assurance additions exercise the real browser/Queue retry and reconciliation
bridge, frozen-source replacement refusal, destructive retry previews,
newer-only replacement, recursive move cleanup, cancellation after commit and
owned-stage failure/quarantine. Input regressions also cover empty/NUL/traversal
rename refusal, invalid attributes, bounded paste, outcome counts/severity and
critical records that cannot become expiring completion notices.

At `5417253`, [Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39650670119)
passed **Linux 401 / macOS 389 / Windows 377**, exact MSRV, source quality and
combined coverage. Each host reported two intentional child-entry skips: the
terminal lifecycle child runs through its PTY/ConPTY parent, and the native
Trash isolation child runs through its native parent. Those parent tests passed,
including terminal cleanup and platform-specific owned Trash recovery.
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39650670086),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39650669962),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39650669873)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39650669959)
also passed. Linux xwin supplies separate cross-build evidence. Infrastructure
files did not change, so this assignment did not run `verify-infra`.

### Coverage investigation baseline, 21 October 2026

Downloaded native LLVM JSON/LCOV and combined artifacts from
[`976cd2b` Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39484254984)
and comparable
[`main` at `10a2682` Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39133765767).
The published source records give:

| Measurement | `main` | M2 baseline |
| --- | ---: | ---: |
| Combined lines | 5,045/5,202 (96.981930%) | 8,803/9,655 (91.175557%) |
| Combined functions | 529/552 (95.833333%) | 915/1,033 (88.576960%) |
| Production-only line projection | 4,538/4,692 (96.717818%) | 8,010/8,853 (90.477804%) |
| Existing production modules | 4,538/4,692 (96.717818%) | 4,588/4,741 (96.772833%) |
| New M2 production modules | Not applicable | 3,422/4,112 (83.219844%) |

The production projection omits explicit test-only modules for analysis only,
following inline and external Rust module declarations transitively. It retains
any module with a production route and test hooks outside those modules.
Collection, uploads and exclusions remain unchanged. The published line delta
is **−5.806373 percentage points**, matching Codacy's −5.806 result against its
−1.0 target. Codacy's **84.72% diff coverage** uses 3,864/4,561 changed coverable
lines. The denominators explain the different percentages; the regression comes
from added M2 runtime code, while coverage of existing modules remains stable.
Native LLVM summaries include overlapping function views; the combined LCOV
uses unique source lines and source-function locations. Neither supplies branch
coverage.

Baseline production-path triage uses each platform's native report. Values below
are covered/found lines; cfg-only modules are absent on other hosts.

| Module/path group | Linux | macOS | Windows | Assurance priority |
| --- | ---: | ---: | ---: | --- |
| Browser operation bridge | 28/100 | 28/100 | 28/100 | Retry linkage, frozen identities and retained-claim reconciliation |
| Operation input/dialog/jobs | 525/707 | 510/707 | 520/707 | Confirmation, disabled conflicts, cancellation and exit routing |
| Execute/transfer | 454/486 | 454/486 | 449/486 | Newer replacement, completed-directory removal and post-commit cancellation |
| Queue/recovery | 437/515 | 438/515 | 440/515 | Same-job reconciliation, ownership and source retention |
| Staging | 136/153 | 136/153 | 148/183 | No-clobber failure, quarantine and native link branches |
| Linux Trash | 191/228 | cfg | cfg | Safe refusal and owned metadata cleanup |
| macOS Trash | cfg | 58/65 | cfg | Native failure and optional receipt evidence |
| Windows Trash/metadata | cfg | cfg | 121/149 | Native error and retained-source paths |
| Windows COM sink | cfg | cfg | 27/125 | Identity refusal; most unhit methods are required no-op callbacks |
| Operations UI | 275/303 | 275/303 | 275/303 | New modal/action/expiry/error branches require fresh evidence |

Required COM interface callbacks, cfg-gated platform code and inline-test
artifacts are separate from missing safety tests. Calling no-op callbacks merely
to raise coverage would add little assurance. Meaningful remediation tests target
the integration and mutation boundaries above. Final native reports and Codacy
variation must be compared with this baseline; no waiver or threshold change
follows from the investigation. Preserve the established Lizard 1.23 parser
anomalies rather than changing valid source to fit the scanner.

### Remediation coverage and remaining disposition, 21 October 2026

The three native JSON/LCOV pairs at `5417253` regenerate the uploaded combined
LCOV byte for byte through the existing merger. No collection, exclusion,
threshold or reporting changes accompany these results.

| Measurement | M2 baseline `976cd2b` | Remediated source `5417253` |
| --- | ---: | ---: |
| Combined lines | 8,803/9,655 (91.175557%) | 9,618/10,339 (93.026405%) |
| Combined functions | 915/1,033 (88.576960%) | 993/1,098 (90.437158%) |
| Production-only lines | 8,010/8,853 (90.477804%) | 8,797/9,509 (92.512357%) |
| Production-only functions | 851/967 (88.004137%) | 927/1,030 (90.000000%) |
| Codacy diff lines | 3,864/4,561 (84.72%) | 4,679/5,245 (89.21%) |
| Codacy variation versus main | −5.806373 pp | −3.955525 pp |

Published line coverage improves 1.850848 percentage points; the like-for-like
production projection improves 2.034553 points. Production uncovered lines fall
from 843 to 712 despite 656 additional coverable lines. The four final input and
result tests add 46 covered production lines without changing the denominator.
Codacy's rounded −3.956-point variation agrees with the published comparison to
main. Its **−1.0-point variation target remains unmet**. Diff coverage has no
defined gate. Green Actions workflows do not constitute Codacy or M2 acceptance.

Final production-path triage distinguishes native availability from coverage:

| Module | Combined | Linux | macOS | Windows |
| --- | ---: | ---: | ---: | ---: |
| Browser operation bridge | 99/111 | 99/111 | 99/111 | 99/111 |
| Operation dialog input | 161/197 | 156/197 | 146/197 | 149/197 |
| Exit input | 91/95 | 91/95 | 91/95 | 91/95 |
| Execute | 387/408 | 387/408 | 387/408 | 382/408 |
| Queue | 349/381 | 347/381 | 349/381 | 348/381 |
| Recovery | 119/134 | 119/134 | 119/134 | 119/134 |
| Staging | 170/186 | 145/153 | 145/153 | 157/183 |
| Operation UI, including submodules | 707/788 | 705/788 | 705/788 | 705/788 |

Native Trash counts remain as in the baseline table. Of the Windows COM sink's
98 unhit lines, 96 implement required no-op callbacks; two cover pre-delete
identity refusal. Remaining safety gaps include exceptional native Trash
submission/cleanup failures, lost stage ownership, transfer mismatch refusals,
and successful Queue reconciliation with genuinely retained quarantined claims.
The real bridge tests cover same-job submission and fail-closed missing claims;
existing direct recovery tests cover restored owned cleanup. Destructive retry
tests prove fresh preview and default cancellation, without exercising an
affirmative retry submission. These are assurance limits, not demonstrated
defects or permission to weaken guards. UI detail/error branches remain partly
uncovered. Neither the native nor merged reports provide branch coverage.

Strict Rust maintainability passes. Lizard 1.23 still merges adjacent valid Rust
tests: existing `operation_controls.rs:145` reports 141 NLOC, and new
`operation_incidents.rs:115` reports 113 although that function ends at line 140.
The prior `permissions::file_type` 55/50 parser anomaly remains unchanged.
No suppression, scanner change or authenticated issue dismissal occurred.
The residual coverage variation and new parser finding require separate narrow
maintainer dispositions. M2 acceptance remains pending.

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
