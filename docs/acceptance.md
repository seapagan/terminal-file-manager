# Acceptance gates

## Windows manual-refresh decision — 25 October 2026

Windows now requires explicit `r` refresh or navigation. It creates no directory
watch owner, periodic listing, reconciliation timer or watcher-unavailable retry.
Hidden panes retain their last listing. Linux/macOS retain native automatic
notifications, debounce, recovery and 30-second reconciliation; `r` works on
every platform. M2 still validates selected objects and destinations independently
of displayed snapshots.

The maintainer rejected buffered candidate `f85fd78` after
[native Windows run 40178904166](https://github.com/seapagan/terminal-file-manager/actions/runs/40178904166):
directory records returned 3-byte sizes where fresh file inspection returned
35 bytes for both changed hard links and flushed open writers. The hard-link
timestamps were stale despite matching IDs and creation stamps. A third ACL
fixture failed to establish denial even for the original backend; its failure
does not justify an access-bypass claim or weaker assertions.

Signed revert `18fffdf` restores the tracked tree exactly to accepted polling
checkpoint `19e1951`; its unchanged clean Rust 1.95.0 full gate passed in 60.11 s,
including mandatory xwin. The signed
[polling performance report](https://github.com/seapagan/terminal-file-manager/blob/a8b205564ba6c2af272a271f719bdf6e4b2b8caa/research/windows-polling-report.md)
measured substantial release CPU at 2,000/10,000 entries. Native watchers blocked
ancestor renames; polling avoided persistent handles but repeatedly enumerated
large directories. Those findings explain the manual policy.

The ignored local archive
`specs/windows-refresh-investigations-2026-10/FULL_REPORT.md` preserves raw
experiments, failed approaches, exact sources, hosted artifacts and limitations.
Future agents must consult it before revisiting Windows auto-refresh. The
historical checkpoints below retain their original scope. Final manual-policy
native and release measurements will be recorded after hosted verification.

## Windows polling: native acceptance, 24 October 2026

Signed implementation `944c83f` uses intentional two-second reconciliation on
Windows. Linux/macOS retain native `notify`, 200 ms debounce, one-second sustained
scan spacing and 30-second reconciliation. Windows creates no native watch owner
or failure/recovery loop. Both panes use the existing bounded listing lanes,
generation checks and directory identity validation. Overdue polls defer behind
in-flight listings and navigation. Unchanged, identity-verified selected directory
counts survive no-op polls; manual refresh, replacement and errors invalidate old
results. Counts remain on-demand snapshots, not live recursive totals.

The unchanged clean `RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed in 49.06 s,
including exact MSRV, packaging, security, coverage and mandatory Linux xwin.
`cargo make verify-infra` passed in 16.25 s. The completed-tree gate first reached
Cargo's strict uncommitted-source rejection; the approved interim 188-file package,
signed commit and immediate clean full gate completed that sequence. Four
historical complexity advisories remain; no new exception or suppression was added.

[Native CI at 944c83f](https://github.com/seapagan/terminal-file-manager/actions/runs/40128806190)
passed Linux 519, macOS 498 and Windows 479 tests, with the two existing terminal
child-entry skips on each platform. The original parent-rename regression passed
unchanged. Current-directory and grandparent renames, deletion/path reuse,
explicit reopening, mark/approval safety, navigation during polling, manual
refresh, read/sibling cadence and ConPTY refresh without input all passed.
Deterministic tests additionally cover concealed-pane updates, no warning/recovery
loop, held-listing deferral, bounded catch-up and stale generations. Frozen M2
operands and operation-time validation remain covered by the native suites.
The previous Windows failure is closed by these native results, not reclassified.

All five ordinary workflows passed at that implementation head. Combined native
LLVM/LCOV coverage merged successfully and was reported to Codacy; static
analysis, diff coverage and coverage variation checks passed. The draft PR remains
unmerged. Historical checkpoints below describe their original evidence only.

### Owned Windows polling measurements

The actual `windows-2025` host was Windows Server 2025, image
`windows-2025-vs2026` version `20261021.278.1`. The existing workflow executed
`cargo llvm-cov nextest --all-targets --all-features --locked --no-fail-fast
--remap-path-prefix --lcov --output-path=coverage-windows.lcov`, followed by the
JSON report. Successful Nextest logs retain the measurement output. These are
instrumented debug observations, not release-build benchmarks or visibility SLAs.

| Observation | 2,000 entries | 10,000 entries |
| --- | --- | --- |
| Per-pane scan starts | 2.001–2.014 s apart | 2.000–2.013 s apart |
| Listing completion, including collection delay | 131–169 ms | 645–776 ms |
| Three isolated changes, both-pane model visibility | 1.041 / 1.996 / 2.017 s | 1.683 / 2.034 / 1.978 s |
| Sustained writer, final model convergence | 196 writes; 1.985 s | 193 writes; 1.937 s |
| Peak active / pending listing lanes | 2 / 0 | 2 / 0 |
| Fresh manual listing, exact model agreement | 86.97 ms | 436.78 ms |
| Idle model CPU / sampled wall interval | 875 ms / 6.557 s | 4,234 ms / 6.833 s |
| Idle TUI CPU / sampled wall interval | 641 ms / 4.530 s | 2,938 ms / 4.584 s |
| Idle TUI working set, before → after | 14,196,736 → 14,893,056 bytes | 24,711,168 → 26,066,944 bytes |
| Idle TUI handles, before → after | 110 → 110 | 110 → 109 |
| External create, TUI render **and exit** | 1.415 s | 1.985 s |

Each size recorded three idle listings per pane in approximately 6.2–6.5 seconds,
then isolated changes and continuous writes. Both installed snapshots matched a
fresh manual listing, including final size and modification time. No overlap,
pending backlog or repeated cancellation occurred. The 10,000-entry TUI sample
used approximately 64% of one core; polling cost is material, even though scans
finished within the approved interval. Release-build and longer load/idle
benchmarks remain useful future performance evidence; no slower cadence was
silently substituted.

After model shutdown, handle counts returned to their respective pre-runtime
baselines (99 and 102). The native cases renamed current/parent/grandparent while
polling remained enabled and idle listing lanes had drained; teardown allowed
another rename, fixture cleanup and terminal re-entry. This proves absence of a
persistent polling watch-handle obstruction in the owned topology, not immunity
from brief ordinary enumeration handles racing another operation. Allocator
working-set retention is not claimed to be a leak or a fixed memory bound.
Windows 11 desktop, UNC/network mounts and unusually slow/unavailable mounts were
not performance-tested. The deferred notification investigation in TODO retains
its broader native path, mount, lifecycle and platform requirements.

## Live local refresh: implementation checkpoint, 24 October 2026

The first post-M2 feature adds native nonrecursive subscriptions for both local
panes, ordinary refresh intents, 200 ms debounce, one-second sustained scan
spacing, post-registration catch-up and unconditional 30-second reconciliation.
Errors retain the last good view; native registration retries back off from one
to 30 seconds while two-second fallback reconciliation remains available.

Local Linux `RUSTUP_TOOLCHAIN=1.95.0 cargo make test` passed 509 tests, with two
intentional child-entry skips exercised by their parent terminal tests.
Regressions cover scheduler deadlines, late dirty state, silent watches,
failure/recovery, stale subscriptions, navigation, physical directory replacement,
marks, deep cursor recovery, metadata scroll and stale metadata/count results.
Native cases exercise create/write/rename/delete, permissions, parent rename,
symlink spelling, registration failure/retry, read-event and sibling filtering,
bounded bursts and clean owner shutdown. PTY tests show automatic rendering
without input and successful terminal re-entry. An active Copy conflict supplies
a deterministic boundary for external changes while the frozen job remains live.

The pinned `notify` 8.2.0 graph passes `cargo make audit` and `cargo make deny`.
The maintainer approved CC0-1.0 and ISC in the normal licence allowlist and the
exact `windows-sys =0.60.2` duplicate exception. Other policy remains unchanged.
ISC notices accompany the Cargo source package and the README in binary archives.
The exact stable crate lacks `EventKindMask`; access-event exclusion occurs in
TFM's adapter rather than an unavailable backend configuration API.

Linux xwin check/Clippy/build and `cargo make verify-infra` passed at development
checkpoints. The unmodified full gate passed native Rust/static/build checks and
stopped at Cargo's clean-source package requirement on the uncommitted tree.
The exact-MSRV aggregate likewise reached that packaging boundary. These results
are interim evidence; clean-tree full verification and hosted Linux/macOS/Windows,
Codacy and combined coverage remain delivery gates. Four historical complexity
advisories remain; changed functions introduce no new threshold exceptions.

Single-workstation Linux measurements: an isolated create appeared in the
2,000-row release TUI in 233 ms; a 1,000-file burst converged in 1.011 s with one
scan per pane, peak two active listing lanes and zero pending lanes. A fresh
1,000-row manual listing took 1.34 ms and matched both installed snapshots.
Latest-only registration/rebinding took 0.259 ms and owner shutdown after recovery
took 0.129 ms in separate native fixtures. These observations do not guarantee
kernel notification or completed-scan deadlines.

After warmup, a two-second idle release-TUI sample used 0.15 CPU seconds versus
0.13 on the exported merged-main baseline; neither emitted terminal output.
Watch-owner threads accumulated no measurable CPU ticks. Feature RSS stayed at
9.35 MiB during the sample, versus 7.49 MiB for the baseline. These short samples
include the existing UI polling cost and do not establish a general benchmark.

No network mount, UNC server, or interactive GNOME Terminal/Ptyxis/Ghostty/Zellij
session was tested locally. Native notifications remain best effort; periodic
enumeration provides eventual reconciliation where enumeration succeeds. A
stalled filesystem syscall can delay its background owner's resource release;
UI shutdown does not wait for it. SFTP refresh remains separate M3 scope.

The first draft-PR run at `9dd9d5c` passed policy, security, packaging, advisory
quality and Codacy static analysis with zero new issues. Native CI stopped before
tests because Rust 1.99 deprecates `AtomicU8::fetch_update`; the exact Rust 1.95
job passed. A compatible compare-and-exchange loop preserves the same bounded
signal state. Local Rust 1.99 Clippy and all 511 native tests pass after that
correction, including concurrent publication, backend overflow/error and recovery regressions. Native
macOS/Windows execution and combined coverage still require the new-head run.

Review also reproduced immediate directory inode reuse in an owned Linux fixture.
The listing adapter now compares available creation stamps as well as canonical
path and native object ID before accepting the result of enumeration. A focused
regression rejects a recycled ID with a changed creation stamp; ordinary
directory length/mtime changes remain valid. Existing M2 operation-time guards
remain unchanged; no destructive-operation bypass was demonstrated.

At `05e53ef`, hosted Linux passed 511 tests and macOS passed 490; each ran the
watch and PTY regressions with two intentional child-entry skips. Windows ran
467 tests: 466 passed and the parent-directory rename case failed with access
denied while a descendant watch handle remained open. The native Windows policy
requires a maintainer decision before acceptance; the failing regression remains
intact. Combined coverage could not run after that failure.

The nonpublishing [eight-target rehearsal](https://github.com/seapagan/terminal-file-manager/actions/runs/40112763161)
passed at that same code/dependency checkpoint: GNU glibc-cap and static MUSL
checks, Intel/ARM Mach-O and Windows x64/ARM64 PE checks, native installer
regressions, actual-archive installation and complete inventory validation.
This proves that checkpoint's builds and distribution compatibility; it does not
replace the pending Windows watch-policy acceptance or certify a public release.

### Independent verification checkpoint at `2738df5`

The creation-stamp correction has two deterministic regressions: recycled native
object IDs with different creation stamps invalidate directory continuity, while
ordinary directory length/mtime activity preserves it. All three feature commits
have valid GPG signatures and Signed-off-by trailers.

The unchanged clean Linux `RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed in
49.81 seconds, then passed again in 48.49 seconds during the Windows-policy
investigation. The latter run followed an identical elevated retry after three
restricted-environment test failures. Both successful runs include 513 native
tests, exact MSRV, clean source packaging, audit/deny, coverage and mandatory
Windows MSVC xwin check/Clippy/build. Two intentional child-entry skips remain.
The earlier infrastructure verification still applies to the unchanged harness.
Four historical Lizard advisories remain; this feature adds no threshold or
suppression exception.

At this runtime checkpoint, [hosted Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/40114194104)
passed 513 Linux and 492 macOS tests. Windows ran 469: 468 passed and the existing
ancestor-rename regression failed with `AccessDenied` while the native watch was
active. Each platform reports two intentional child-entry skips. The other four
ordinary workflows and Codacy static analysis passed; combined coverage was
skipped after the Windows failure. New combined coverage acceptance remains
outstanding. The draft PR remains unmerged, and Windows policy remains undecided.

A corrected release-TUI collector aggregates threads with duplicate names.
Its owned 2,000-file fixture measured 233.87 ms isolated-change visibility;
2.008 seconds of idle browsing used 0.12 CPU seconds, emitted zero terminal
bytes and retained 9,244,672 bytes of RSS. Watch owners and the notification
backend accumulated zero measurable CPU ticks; the two watch owners recorded
40 context switches combined. These values supersede the earlier feature idle
sample above. The separately measured baseline remains 0.13 CPU seconds and
7.49 MiB RSS; short samples do not establish a general performance comparison.

A separate native Linux coordinator/worker probe used a debug build and 2,000
rows. An external writer made 666 updates over 3.401 seconds. Both panes started
four scans at 1.009, 2.017, 3.024 and 4.027 seconds, with 1.003–1.008 seconds
between starts. Peak listing occupancy was two active lanes and zero pending;
both snapshots matched a fresh manual listing after the final write. Final
catch-up took 642 ms; that manual listing took 4.31 ms. This measures sustained
native scheduling separately from the earlier finite 1,000-file burst and the
release-TUI latency sample. It does not establish Windows Shell-notification
performance or network-filesystem behaviour.

The [eight-target rehearsal at `2738df5`](https://github.com/seapagan/terminal-file-manager/actions/runs/40116517330)
passed preparation, all eight native build jobs and complete inventory checks.
Logs confirm GNU glibc 2.28 limits, static MUSL checks, Mach-O/PE validation,
native installer regressions and actual-archive installation. Windows x64 and
ARM64 each ran PowerShell 5.1 and 7 security/installer cases without native skips.
This closes build/distribution compatibility for the current runtime sources;
the failing Windows watcher gate and combined coverage remain outstanding.

## Current status and evidence

M0's engineering foundation is complete at the accepted checkpoints below. The
application includes the M1 local browser and completed M2 local operations.
The maintainer merged M1 in [PR #2](https://github.com/seapagan/terminal-file-manager/pull/2)
on 18 October 2026 and M2 in [PR #3](https://github.com/seapagan/terminal-file-manager/pull/3)
on 24 October 2026. The post-M2 live-refresh feature is in development; M3–M8 cover the remaining
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

### M2 final safety-assurance source checkpoint, 22 October 2026

This round adds five tests without changing production behavior, dependencies,
QC gates, suppressions or coverage collection. The previous checkpoints and
coverage figures above retain their original attribution.

| Test location | Exercised safety guarantee | Platforms |
| --- | --- | --- |
| `src/operations/queue/tests.rs`, `queue_owned_reconciliation_releases_quarantine_only_after_owned_cleanup` | A Queue-issued move acquires real coordinator stage claims, retains unknown cleanup evidence after payload replacement, refuses conflicting work through an unsuccessful same-job reconciliation, then cleans the restored owned payload and releases the blocked job. Source bytes and foreign content survive; reconciliation never copies or removes the source again. | Portable filesystem fixture |
| `src/browser/operations/tests.rs`, three destructive retry cases | F9/Retry opens a fresh recursive preview; early D submits nothing. A later D submits a distinct child attempt that removes only the unfinished file/tree, preserves a new occupant at the completed path, and reports exact completed counts and a success notice. Replaced roots fail preview; replaced descendants fail final execution. Enter cancels the failed-preview dialog. | Portable filesystem fixtures |
| `tests/operation_assurance.rs`, `transfer_verification_failures_preserve_move_source_and_previous_destination` | Staged size/checksum mismatch and a changed opened source prevent verification, commit and move-source removal. The previous destination survives and owned staging disappears. | Portable filesystem fixtures |

**A: successful queue-owned reconciliation is covered under controlled scheduler
driving.** The test constructs Queue's channels and uses its public submission,
update and reconciliation methods, production `pump`/`receive`, actual workers
and the same Coordinator throughout. It pauses the acknowledged stage claim
before replying to the worker. No unrelated recovery record or ownership claim
is fabricated. This test does not run the background `schedule` loop, progress
forwarding or shutdown; existing Queue integration tests exercise those paths
without this fault. It therefore does not establish the entire background-loop
recovery scenario end to end.

**B: affirmative destructive retry submission is covered through the controller
and real background Queue.** The initial partial deletion comes from direct
execution with a synchronous cancellation callback and enters history as its
actual record. Retry, fresh count, separate D approval, worker acknowledgement,
job linkage and notices use the production controller/Queue path. The original
partial attempt does not pass through the background Queue. Existing Delete and
Trash retry tests retain safe default cancellation. This round deliberately
submits permanent deletion only against disposable fixtures; it does not add an
affirmative native Trash retry or infer native recycling proof from deletion.

**C: bounded transfer failures are covered.** No new exceptional macOS Foundation
or Windows Shell failure is induced: those native APIs lack a deterministic
failure seam for a controlled post-submission/cleanup error, and provoking
ambiguous recycling or OS permanent loss would exceed this round's safety
boundary. Existing native recovery/refusal tests remain required. The test does
not cover every source read/close error or crash durability; process-local
identity checks retain the documented external-writer race limits.

The completed working-tree native Linux suite passes **406 tests**, with the
two existing child-entry skips. The sandbox run failed existing Trash fixtures;
the identical elevated retry passed. Strict Rust maintainability passes. The
precommit full gate and exact-MSRV task reach only Cargo's expected dirty-package
boundary; interim `cargo package --locked --allow-dirty` and its list check pass.
Audit, deny, workflow security, native paired coverage, ShellCheck and compulsory
Linux xwin check/Clippy/build pass separately. Clean signed-tree full verification,
foreign-native results and regenerated combined/production/diff coverage remain
pending delivery below. No infrastructure changed; `verify-infra` is not run.

Lizard 1.23.0 reports a new parser merge at
`src/browser/operations/tests.rs:341`: `open_delete_retry` reports **128 NLOC**
although its body ends at line 360; the scanner includes the three following
separate tests. Strict Clippy accepts each function. The established
`permissions::file_type` 55/50, `operation_controls.rs:145` 141/50 and
`operation_incidents.rs:115` 113/50 findings remain. No cosmetic restructuring,
suppression, detector change or issue dismissal follows. Maintainer disposition
of each scanner finding, residual Codacy variation and final M2 acceptance
remains pending; behavioral assurance does not waive coverage policy.

#### Native fixture correction, 22 October 2026

The first hosted assurance run at `d575636` passed Linux but failed two new
macOS cases before producing combined coverage. APFS moved the stage's creation
time backward while the worker preserved the source mtime. These faults occur
before `OwnedStage::seal` can refresh evidence through its held handle; cleanup
and later reconciliation retained OutcomeUnknown under the original identity.
The worker retained uncertainty before authorizing source removal. The corrected
fixtures set a fixed future source mtime so they isolate payload replacement and transfer
mismatch without an additional APFS birth-time transition. Ownership checks and
expected failure/cleanup assertions remain intact. Pre-seal failure combined
with APFS birth-time adjustment remains a recovery limitation; this round does
not broaden identity adoption or change production recovery semantics.

#### Corrected native and coverage proof, 22 October 2026

Signed source checkpoint `3280286c5f830ff8c51dc8916b43559c9659687c` passed
[Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39826650209),
including native **Linux 406 / macOS 394 / Windows 382**, exact declared
**Rust 1.95.0**, source quality and combined coverage. All five added tests ran
on each host, including the corrected APFS cases. Each host retained two
intentional ignored entry points: `terminal_child`, exercised by the PTY/ConPTY
lifecycle parent, and `trash_child`, exercised by the native Trash isolation
parent. Those parents passed; the native logs contain no NOT RUN fallback.
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39826650186),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39826650227),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39826650239)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39826650333)
passed. The earlier `d575636` run failed its two macOS cases and skipped combined
coverage; these corrected results supersede that incomplete assurance run.

Immediate unchanged clean-tree
`RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed at both signed checkpoints:
`d575636` in 35.16 seconds and `3280286` in 37.95 seconds. The latter includes
406 native Linux tests, exact MSRV, clean package checks, audits, source/workflow
security, paired coverage, ShellCheck and mandatory xwin check/Clippy/build.
Xwin remains compilation evidence; the Windows runtime proof comes from its
native job. Infrastructure, collection and reporting policy did not change;
`verify-infra` and distribution rehearsals were not run.

The three native JSON/LCOV pairs regenerate the uploaded combined LCOV and
summary exactly (LCOV SHA-256
`7c67c817ac7d502616e4759e5abf9e4558c57feef60f6c81e6cedfdad73ee90e`).
The same module-graph projection used above removes only explicit test-only
modules for analysis. Raw collection and Codacy uploads retain their existing
source set. Values below count unique source lines/functions in normalized
LCOV, rather than overlapping native LLVM summary views; no branch data exists.

| Measurement | Comparable `main` `10a2682` | Previous delivery `3d13c13` | Assurance source `3280286` |
| --- | ---: | ---: | ---: |
| Combined lines | 5,045/5,202 (96.981930%) | 9,617/10,339 (93.016733%) | 9,657/10,339 (93.403617%) |
| Combined functions | 529/552 (95.833333%) | 993/1,098 (90.437158%) | 998/1,098 (90.892532%) |
| Production-only lines | 4,538/4,692 (96.717818%) | 8,796/9,509 (92.501840%) | 8,836/9,509 (92.922494%) |
| Production-only functions | 489/512 (95.507813%) | 927/1,030 (90.000000%) | 932/1,030 (90.485437%) |
| Codacy diff lines | Not applicable | 4,678/5,245 (89.19%) | 4,718/5,245 (89.95%) |
| Exact published variation versus `main` | Not applicable | −3.965197 pp | −3.578313 pp |

| Native unique LCOV lines | Previous delivery | Assurance source |
| --- | ---: | ---: |
| Linux | 9,151/9,736 (93.991372%) | 9,189/9,736 (94.381676%) |
| macOS | 8,832/9,378 (94.177863%) | 8,871/9,378 (94.593730%) |
| Windows | 8,908/9,624 (92.560266%) | 8,961/9,624 (93.110973%) |

The combined report observes **40 additional covered production lines and five
functions**, with unchanged denominators. Production uncovered lines fall from
713 to 673. This measurement remains distinct from the specific guarantees and
limits above; scheduler/progress timing can vary individual native hits.
[Codacy PR #3](https://app.codacy.com/gh/seapagan/terminal-file-manager/pull-requests/3)
reports **−3.578 pp variation**, **89.95% diff coverage**, and a failed
**−1.0 pp variation target** at this exact source head. Green Actions do not
resolve that failure.

Codacy also reports three open Lizard PR findings: existing
`c8915940aa1c966dc9ec9281d5741f71` (controls 141/50),
`ab49192d7f4876ca903bcff9061fe1cc` (incidents 113/50), and new
`70e4c4e09ad13a4bc3a72275d2ea7324` (retry helper/parser merge 128/50).
The local `permissions::file_type` 55/50 anomaly remains the fourth local
advisory. These issue-specific parser explanations do not dismiss findings or
change thresholds. Strict Rust maintainability passes. The maintainer still
must decide scanner dispositions, residual coverage variation and final M2
acceptance. The controlled scheduler scope, original-attempt retry scope,
pre-seal APFS recovery limitation and deferred exceptional native failures
remain as stated above.

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

## M2 review-fix checkpoint — 23 October 2026

Starting local, remote and draft PR #3 heads agreed at
`358324c429c778ffcfa01cbb74243bc0353497fd`. Review-fix source is
`fc084dad21b44a6601b0ac9f7a08cd248f3179fb`, followed by the final source-identity
guard in the commit containing this checkpoint. A and C pass local
regressions. B passes on Linux but remains blocked on native macOS; this
checkpoint does not grant M2 acceptance. Older checkpoints above remain dated
records, including their original coverage measurements and scanner status.

### A: private transfer directories and existing merges

The original deterministic reproducer copied a `0700` directory with a `0644`
child. At `Stage::Committed`, both new ancestors had mode `0775`; cancellation
and a failed child retained that exposure. An existing `0711` merge target also
changed to source mode `0750`. Four new tests failed before the correction.

Unix transfer creation now supplies mode `0700` to `DirBuilder` at mkdir,
before copying children. The worker checks cancellation before directory
finalisation and applies source permissions only to newly created, completed
directories. Existing merge targets keep their modes. Windows retains native
directory creation and final attributes for new targets. The recursive executor
shares this policy across Copy and Move, including Linux cross-filesystem Move.

`tests/directory_permissions.rs` checks real destination modes and bytes:

- `cancelled_recursive_copy_and_move_keep_new_ancestors_private` inspects nested
  ancestors at the committed-child callback and after cancellation.
- `cancelled_cross_filesystem_directory_move_keeps_new_ancestors_private` uses
  owned `/dev/shm` storage and proves different native device IDs on Linux.
- `failed_child_keeps_new_directories_private_and_copied_bytes_intact` uses an
  unsupported Unix socket child and verifies retained private directories.
- `completed_directories_copy_source_modes_but_merges_preserve_existing_modes`
  checks complete Copy/Move modes, bytes, source retention/removal and merges.
- `retry_treats_retained_private_directories_as_existing_merge_targets` proves
  the safe retry limitation: a retained `0700` target stays `0700` even when the
  source mode is `0750`. Current cancellation records do not retain destination
  ownership sufficient to authorize changing an existing target's permissions.
- `source_directory_replacement_cannot_broaden_copied_private_permissions`
  replaces the source at the last committed child. Its RED run exposed a
  `0777` destination and a false completed-directory result. The final guard
  validates source parent/object identity before chmod and uses frozen Unix
  mode bits, leaving copied bytes under `0700` and reporting the source change.

These Unix tests use owned fixtures, callback boundaries and native metadata;
none changes process umask or relies on sleep. Pre-existing permissive merge
parents remain permissive. Completed descendants retain their successful result;
cancelled/incomplete ancestors remain private with existing partial/unknown
classifications. The filesystem race and external-writer limits above remain.

### B: Linux owner chmod and the macOS approval gate

The native RED test exercised owned file `0000 → 0600` and directory
`0000 → 0700`; both returned `Failed` with mode `0000`. Linux now opens an
`O_PATH | O_NOFOLLOW` descriptor, validates its native identity and the parent
namespace, then chmods the held descriptor through `/proc/self/fd`. This pins
the inode without requesting content access. Missing/inaccessible procfs fails
visibly; there is no original-path fallback. Windows retains its attribute
access mask, reparse flags and descriptor-based Read-only editing. No dependency,
unsafe boundary or suppression was added.

Regression names:

- `owned_unreadable_file_and_directory_modes_can_be_restored_without_read_access`
  checks resulting modes, device/inode identity and retained file/child bytes
  without root privileges.
- `chmod_refuses_a_replaced_regular_file_and_preserves_both_objects` and existing
  `mode_changes_use_frozen_identity_and_reject_replacement_links` check frozen
  selection and unrelated bytes/modes.
- `chmod_denied_parent_search_preserves_the_selected_object` checks denied
  namespace access using an owned parent and restores fixture access.
- `attribute_descriptor_never_retargets_a_replacement_name_or_link` replaces the
  selected name after opening; the held descriptor affects only its original
  inode, and selection revalidation rejects the replacement.
- Windows `native_readonly_survives_new_destination_commit_and_can_be_edited`
  remains unchanged and passed on the actual Windows runner.

**macOS B remains unresolved.** `O_EVTONLY | O_NOFOLLOW` passes ordinary readable
attribute cases but cannot open the owned `0000` fixtures on macOS 15. The two
new unreadable/descriptor tests remain failing; their assertions were not
weakened or skipped. Apple XNU's `vn_authorize_open_existing` retains content
read authorization, and its alternative process policy requires a private
entitlement. A public `fchmodat(..., AT_SYMLINK_NOFOLLOW)` fallback could retain
parent and no-follow checks but cannot atomically reject a regular-file
replacement. The assignment requires explicit maintainer approval before that
identity-assurance deviation. That decision is pending; no fallback or private
entitlement was introduced.

### C: conflict controls during safe exit

The original RED sequence selected Wait then exit and installed a pending
conflict. Keyboard, mouse and Esc replies left the channel empty; a real queued
worker could not finish. The corrected wait branch routes visible conflict and
Jobs controls through the existing handler while blocking ordinary actions and
new retry/reconciliation attempts. Worker acknowledgement, q/Q intent, frozen
session snapshots and unknown/incident exit protections retain their guards.

Esc now cancels the visible conflict while retaining the pending exit; in Jobs
it closes Jobs first. With neither surface active, Esc or its rendered Return
button abandons the exit and discards the snapshot. Under diagnostics, ordinary
actions stay blocked and hidden conflicts receive no input; Esc abandons exit.
A separate pair of failing-first regressions demonstrated the previous
diagnostics bypass and inert Return mouse button before their correction.

`tests/exit_conflicts.rs` covers these observable paths:

- `waiting_exit_routes_conflict_choices_once_and_keeps_q_or_q_save_intent_until_ack`
  checks Skip/Replace/Keep both/default Enter/cancel, q/Q, Wait/Cancel safely,
  Press versus Repeat/Release, immediate single channel delivery and real bytes.
- `waiting_exit_mouse_conflict_hit_regions_reply_without_allowing_pane_actions`
  checks exact hit/non-hit regions and blocked pane/mutation/exit keys.
- `conflict_escape_cancels_that_job_then_a_second_escape_abandons_pending_exit`
  distinguishes conflict cancellation from discarding exit intent.
- `multiple_conflicts_keep_exit_pending_until_both_jobs_acknowledge` checks
  intended reply channels, Jobs access, blocked retry and both acknowledgements.
- `queued_worker_conflict_after_waiting_save_exit_requires_finished_acknowledgement`
  uses the actual background Queue, controller and completed destination bytes.
- `safely_cancelled_worker_ignores_a_stale_conflict_choice_and_exits_only_after_ack`
  proves real queue cancellation preserves source and old destination bytes.
- `diagnostics_cannot_bypass_pending_exit_or_steal_hidden_conflict_input` and
  `pending_exit_return_button_abandons_snapshot_without_cancelling_the_job`
  cover the two additional demonstrated boundary paths.

The direct channel assertions detect swallowed replies immediately. Ten-second
Queue receives serve only as failure guards around actual lifecycle updates;
no sleep controls the test sequence. Existing unknown/critical, destructive
confirmation, Trash, recovery/quarantine and terminal restoration tests remain.

### Local and hosted evidence

All scoped commits are GPG-signed with Signed-off-by trailers. Local unchanged
`RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed on clean commits `b818316`
(37.90 s), `382ea10` (37.66 s), `394dc8e` (37.23 s) and `fc084da` (36.83 s).
The final source ran **423 Linux tests, 423 passed, two intentional child-entry
skips**, plus doctests, exact MSRV, package verification, coverage, security and
mandatory xwin check/Clippy/build. The initial A commit `622a0fe` exposed a
Windows-only unused-mut error in its immediate full gate; signed follow-up
`b818316` corrected it and passed. Dirty-package checks were interim only,
followed by unchanged clean gates. Sandbox SSH/test restrictions received
elevated retries; the product failures above were not attributed to sandboxing.

Four existing local Lizard parser advisories remain (permissions 55/50, controls
141/50, incidents 113/50, retry helper 128/50); no new finding or suppression
remains. Infrastructure is unchanged, so verify-infra was not run. No release
rehearsal, minimum-OS, live UNC/SMB or additional filesystem certification is
claimed.

At source `382ea10`, [Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39991564469)
ran actual Linux **414 passed**, Windows **382 passed**, and macOS **399 passed,
two B tests failed** out of 401. Each host reports two intentional ignored entry
points: `terminal_child` and `trash_child`, exercised through their parent
terminal-restoration and native-Trash tests. The Windows Read-only regression
and Linux A/B regressions executed successfully. macOS A tests passed; its two
B failures block foreign-native completion and the combined coverage job.
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39991564408),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39991564360),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39991564280)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39991564298)
passed at that source. Final review-source native observation follows below.

[Codacy PR #3](https://app.codacy.com/gh/seapagan/terminal-file-manager/pull-requests/3)
Static Code Analysis succeeded at `382ea10` with zero check annotations. No new
combined coverage variation exists for that source because macOS failed and
combined coverage skipped. The maintainer has accepted the documented earlier
M2 coverage decrease without changing the configured −1.0 pp target, and marked
the three existing Codacy Lizard parser findings false positive. Those decisions
supersede the outstanding-disposition wording in older dated checkpoints, not
their historical measurements. No threshold, metrics collection, scanner
disposition or security/dependency policy was changed here.

At hosted review source `fc084da`, [Rust CI](https://github.com/seapagan/terminal-file-manager/actions/runs/39993060079)
ran Linux **423 passed**, Windows **390 passed**, and macOS **408 passed, two B
tests failed** out of 410. All eight C regressions and all applicable A
regressions executed successfully on each native host; the Linux-only
cross-filesystem case did not run on macOS/Windows. The two child-entry skips
per host retain the parent-test rationale above. Exact MSRV and source quality
passed; combined coverage skipped. [Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39993060091),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39993060086),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39993060078)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39993060146)
passed. Codacy static analysis succeeded with zero check annotations; no new
combined variation was published. The final source-identity guard has an
additional local RED/GREEN regression (**424 Linux tests passed**); native and
clean-gate evidence for that follow-up must be recorded separately.

### 23 October follow-up: approved macOS capability limit

The maintainer chose **“Keep macOS safely blocked; do not weaken identity.”**
This decision supersedes the pending-approval language in the checkpoint above.
Unreadable macOS files/directories remain unsupported for permission edits;
they fail visibly without changing mode or native identity. TFM does not use a
pathname fallback, private entitlement or weaker selected-object assurance.
The ineffective `O_EVTONLY` addition was removed; readable macOS objects keep
the existing no-follow read handle and descriptor-based chmod.

Linux's
`owned_unreadable_file_and_directory_modes_can_be_restored_without_read_access`
retains its successful `0000 → 0600` / `0000 → 0700` assertions. The separate
macOS regression
`unreadable_unix_attribute_edits_fail_closed_when_identity_cannot_be_pinned`
requires `Failed` results, unchanged native identity and mode `0000`, and
retained file/child bytes before fixture cleanup. This is the approved macOS
capability contract, not a skipped restoration claim.
`attribute_descriptor_never_retargets_a_replacement_name_or_link` keeps the
Linux unreadable-handle case and uses a readable `0400` macOS fixture to prove
that normal supported attribute edits cannot retarget a replacement. No new
ignore, lint suppression or dependency exception was added. Native results for
this approved policy follow below.

### Final approved-source native evidence — 23 October 2026

Immutable source `b7d6cecaf753b5b2d30dba9ff0e674876ebbcdb3` includes the final
source-directory identity guard and the maintainer-approved macOS capability
limit. Its unchanged clean local
`RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed in **36.90 s**, including
424 Linux tests, exact MSRV, package/security/coverage and mandatory Windows
xwin check, Clippy and build. The preceding source guard `85d54f1` passed its
clean full gate in **36.12 s**. These are Linux full gates, separate from the
actual native runs below.

[Rust CI 39995625473](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625473)
completed successfully, including exact MSRV, source quality and
[combined coverage](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625473/job/120561240808).

| Actual native job | Tests passed | Intentional child-entry skips | Exercised contracts |
| --- | ---: | ---: | --- |
| [Linux / ubuntu-24.04](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625473/job/120558490282) | 424 | 2 | A modes/bytes/source replacement/retry and distinct-device Move; B unreadable restoration and pinned identity; all eight C regressions |
| [macOS / macos-15](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625473/job/120558490550) | 411 | 2 | A Unix modes/source identity/retry; B approved unchanged-mode/identity rejection and readable descriptor safety; all eight C regressions |
| [Windows / windows-2025](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625473/job/120558490512) | 390 | 2 | Native Read-only editing, shared transfer/Trash safeguards and all eight C regressions |

The two skipped child entry points retain the parent-execution rationale above;
actual native Trash and terminal-restoration parents passed on all three hosts.
No new skip or unsupported-host substitution supplied these results.
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625561),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625551),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625514)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/39995625456)
also passed. Cache restoration emitted nonfatal `ENOENT` annotations for
generated `target/package/.../tests/target` and `tests/trybuild` paths; the
subsequent native and coverage steps passed. No cache/workflow change was made.

Codacy at this exact source reports **90.52% diff coverage (4,823/5,328)** and
**−3.305 pp variation** against the unchanged **−1.0 pp** target. The coverage
variation remains `action_required`, under the maintainer's accepted M2
decrease; this work did not adjust collection or targets. Static analysis
reported one genuinely new issue, `markdownlint_MD012`
`fea36b203143b34ffe63cf808d40b6ad`: an extra blank line added before this new
checkpoint. The final evidence change removes only that added blank line.
Earlier checkpoint text and measurements remain unchanged. Post-push analysis
must confirm resolution; no manual issue dismissal or suppression was used.
The three previously accepted Codacy Lizard false positives remain untouched.

Only documentation changes follow this tested source. The ignored append-only
ledger records signed commits, each immediate clean-gate outcome and final-head
checks. PR #3 remains draft and unmerged; M2 acceptance and merge remain the
maintainer's decision.

## M2 DeepSeek correction round — 23 October 2026

The maintainer authorized remediation of the 16-reviewer summary on draft PR #3,
starting from verified clean head `96a2f53fa16dd8a80a35acc2271f49b6a5eca5a3`.
This checkpoint appends correction evidence; earlier measurements remain intact.
The approved macOS unreadable-object chmod limitation and accepted M2 coverage
variation remain unchanged. No dependency version, QC gate or coverage target
changed.

### Correctness and native filesystem contracts

| Review item | Correction and regression evidence |
| --- | --- |
| A: modal key repeats | Real controller routing covers Jobs and conflict-only surfaces, `j`/`k`/Up/Down Repeat and Release, with and without pending exit. The RED case moved the pane cursor. Nonmodal repeats still navigate; existing conflict keyboard/mouse and Esc tests pass. Paging repeats also stay consumed. |
| B: directory finalisation | Deterministic permission-finalisation, removal, cancellation and identity-replacement cases assert child bytes/outcomes, parent state, recovery and retry/reconciliation. Proven retained sources report partial results; replaced identities remain unknown and refuse retry. Worker-side directory evidence validation precedes retry mutations. Copy cancellation retains both sources and destinations. |
| C: Linux mount aliases | Resolution exercises `reject_aliases` with same-device unrelated aliases, a textual but noncomponent prefix, physical symlinked parents, source ancestors, subtree binds and ambiguous stacks. Unrelated aliases work; relevant ambiguity still fails closed. |
| D/E: exclusive rename | Linux/macOS use native exclusive rename; Windows uses `MoveFileExW(0)`. File/directory destination races preserve independent destination bytes and identity. Normal file/directory/selected-link moves and distinct-device Rename refusal have regressions. No hard-link/unlink fallback remains. Windows also has an owned counterfactual fixture demonstrating the old overwrite. |
| Stage construction | A malformed link target reproduced an empty-stage leak. Ordinary construction failure now removes only its revalidated empty owned root; foreign content and replacement fixtures remain intact. |
| Transfer fidelity | An 8 MiB fixture reproduced dense expansion. Coalesced zero-region writes preserve large holes, interleaved pages and trailing length; tests compare complete bytes, logical size and allocated blocks. Optional SHA-256 and the existing injected-truncation regression remain effective. A 511-byte write fixture covers unaligned writes. |
| Destination atime | Copy/Move, with checksum on/off, restore pre-read source atime after verification. Whole-second native comparisons accommodate timestamp resolution; mtime and payload assertions remain exact. |
| Missing device identity | An exact-diagnostic regression distinguishes unavailable identity (`Failed`, mutation refused) from an actual device mismatch. |
| Windows Trash callback | Typed callback failures retain their original outcome. Native-only tests preserve validation/native `Failed` classification while giving ambiguous results and critical permanent-deletion acknowledgements precedence. |
| Rename presentation | Unix native-byte names display escaped text and require a deliberate replacement; the selected native operand stays separate. Native-byte rename and valid Unicode replacements have regressions. Stale M1 read-only action messages and input documentation were removed. |

The maintainer approved the narrow native rename unsafe boundaries and two
Windows sparse handle boundaries, plus the existing `windows-sys`
`Win32_System_IO` feature. No new dependency or pathname-only chmod fallback was
introduced. Native syscall/filesystem errors remain visible and fail closed;
no synthetic unsupported-primitive fallback can report success.

Sparse handling preserves the tested allocation behaviour, not identical extent
maps, compression or arbitrary filesystem capabilities. A substantially sparse
or compressed source whose destination expands to at least half its logical
size fails before commit/source removal. Windows requires the native sparse
control for a sparse source; unsupported controls fail. Source atime may change
because TFM reads content. Only destination atime restoration is promised when
the filesystem supports the metadata operations. Local sparse fixtures ran on
Linux tmpfs; actual APFS/NTFS execution remains pending below.

### Jobs accessibility and measured cost

`tests/operation_ui_stress.rs` constructs 2,500 results with 40 long path
components at 120×40 cells. It exercises PageUp/PageDown, consumes Repeat and
Release, checks a detail limit above 65,535 rows and renders the final marker.
Existing scrolling already supplies navigation; additional pagination was not
needed. The old `u16` offset did prevent access to the last result.

Optimized native fixture measurements, using the same release dependency
fingerprints and `/usr/bin/time -v`, were:

| Measurement | Before | After |
| --- | ---: | ---: |
| Geometry | 1.299 s | 26.60 ms |
| Bottom redraw | 3.235 s | 40.29 ms |
| Fixture total | 7.770 s | 108.66 ms |
| Peak resident memory | 32,356 KiB | 20,408 KiB |
| Final result accessible | No | Yes |

These are single-fixture observations, not percentile or unlimited-history
claims. Shared wrapped layouts, capped width scanning and visible-row rendering
resolve the reproduced cost without a shared-modal rewrite or history
virtualisation. Unicode/wide/combining/control/bidi cell assertions still pass.

### Test rigor and remaining recovery work

Conflict cancellation now proves exactly one operand record and no later
mutation, with both source payloads and the previous destination retained.
Linux cross-volume tests discover and verify a writable different device;
hosted Linux refuses to silently skip that assurance. Linux Trash fixtures
restore only their frozen owned receipts, unlink only their metadata and remove
only newly created, identity-checked empty store directories. Regression cases
preserve pre-existing stores, foreign contents, replaced/dangling directories
and racing restore targets. macOS native tests accept an absent optional
recovery URL and verify identity/content/link targets whenever a URL exists.
Job outcome tables compare `JobState` values directly.

Crash/panic orphan discovery remains deferred under
[Pre-v1 reliability](../TODO.md#pre-v1-reliability). Payload sync and in-memory
ownership/recovery evidence do not establish safe ownership after restart.
There is no startup sweep. The TODO requires durable ownership and quarantine,
termination-boundary/replaced-identity tests and proof that cleanup preserves
the only payload and unrelated user files. No speculative fidelity or UI TODO
substitutes for the implemented fixes above.

### Local verification checkpoint

Signed source commits `5db135d`, `ab0213b`, `0e0940d`, `6ccdd4a`, `e11a710` and
`28b2d34` each passed an immediate unchanged clean
`RUSTUP_TOOLCHAIN=1.95.0 cargo make verify`. The last gate passed in **39.20 s**,
with **452 Linux tests passed**, two intentional child-entry skips, exact MSRV,
package/security/coverage checks and mandatory Windows MSVC xwin check, Clippy
and build. Xwin is compilation/lint proof, not Windows runtime proof.
The approved dirty-package/interim-package/sign/clean-gate sequence supplied
each source commit; no partial gate is labelled a clean full pass.

Four existing Lizard advisories remain: `open_delete_retry`, `file_type`,
`cancel_then_exit_waits_for_acknowledgement_and_return_abandons_exit_intent` and
`active_workers_must_acknowledge_stop_before_incident_exit_is_available`.
No suppression or threshold changed. Infrastructure and distribution tooling
are unchanged; `verify-infra` and release rehearsal were not required or run.
Hosted native results, all five workflows and Codacy for this correction source
must be recorded after push. PR #3 remains draft and unmerged.

### Correction-source native evidence and remaining static gate

Immutable source `65d048472e2fd73c8152d4c2c25eb21374bb76f5` passed its unchanged
clean local `RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` in **46.71 s**:
452 Linux tests, two existing child-entry skips, exact MSRV, package/security/
coverage checks and mandatory xwin check, Clippy and build. The intermediate
fixture-layout commit `32fc7e8` passed its clean gate in **40.35 s**.

[Rust CI 40021352935](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352935)
passed, including exact MSRV, source quality and
[combined native coverage](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352935/job/120648939910).

| Native host | Passed | Exercised correction contracts |
| --- | ---: | --- |
| [Linux / ubuntu-24.04](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352935/job/120646421375) | 452 | Actual second-device Move/Rename refusal, directory finalisation/recovery, native exclusive rename and directory Trash, owned cleanup, raw-byte Rename, sparse allocation/bytes and atime |
| [macOS / macos-15](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352935/job/120646421486) | 432 | Directory finalisation/recovery, native exclusive file/directory/link rename, optional Trash receipt, APFS sparse Copy/Move and atime; approved unreadable-object chmod rejection remains intact |
| [Windows / windows-2025](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352935/job/120646421249) | 411 | Native no-clobber races and old-overwrite control, NTFS sparse Copy/Move/cancellation/allocation, directory recovery, typed Trash failure classification and native recycling, Unicode Rename and atime |

Each host retained the two established child-entry skips; passing parent tests
ran native Trash and terminal restoration. No new skip supplied this proof.
[Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352908),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352934),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/40021352960)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/40021353007)
also passed. Source quality retains the same four Lizard advisories. Cache
restoration still emits nonfatal `ENOENT` annotations for generated packaged
`tests/target` and `tests/trybuild` paths; subsequent jobs succeed.

The first correction push exposed a package-content failure: the new nested
`tests/common` helpers built but violated the existing root-test-only inventory.
The correction moves them to `tests/volume_fixture.rs` and
`tests/trash_fixture.rs`, following the existing fixture layout, and removes the
added manifest include. The unchanged package analyser/enforcer passes locally
and in the linked workflow. No package policy or workflow changed.

Initial APFS tests also exposed a fixture assumption: the operating system
allocated all **8,388,608 bytes** of the 8 MiB source before TFM ran, and all
**1,048,576 bytes** of the small short-write output. Mount diagnostics confirmed
APFS. The final Copy/Move fixture uses native seek/write to create **64 MiB** and
requires source allocation below one quarter of logical size before execution.
It then requires destination allocation below the same bound, SHA-256, complete
bytes and correct source retention/removal. The 511-byte short-write test
compares allocation against an independent native write/seek/length fixture on
the same filesystem and still checks every byte and trailing length. Both
tests ran and passed on macOS. These results establish preservation of proven
sparse sources; they do not promise sparse storage for small files that APFS
itself allocates densely. No broad APFS fidelity deferral or platform skip was
introduced.

[Codacy PR #3](https://app.codacy.com/gh/seapagan/terminal-file-manager/pull-requests/3)
at this exact source reports **91.18% diff coverage (5,409/5,932)** and
**−3.156 pp variation** against the unchanged **−1.0 pp** target. The variation
remains under the maintainer's accepted M2 decrease. Combined collection and
configured thresholds remain unchanged.

**The static-analysis gate remains blocked by three new generic unsafe-use
alerts** (`rust.lang.security.unsafe-usage.unsafe-usage`): Linux rename,
Windows rename and Windows sparse control. They point to the native boundaries
whose function-scoped unsafe allowances the maintainer approved. The audit
checks live borrowed handles, initialized/sized buffers, lossless native paths,
synchronous lifetimes and exclusive/no-fallback flags. Separate approval for
five call-site scanner comments remains pending; no scanner suppression or
remote finding disposition has been applied. This checkpoint does not declare
Codacy green or waive the static gate. Crash recovery remains the explicit
[pre-v1 reliability TODO](../TODO.md#pre-v1-reliability). PR #3 stays draft and
unmerged pending the maintainer's decision.

### Final sparse-allocation refusal correction

Source `51fe5f6ceac86188b4e172291950c9386bbea1a1` also refuses full destination
materialisation when the source uses more than half its logical size. The
earlier substantial-expansion check alone accepted a 75%-allocated source
becoming fully dense. A native dense-target regression reproduced that `Ok`
result before the correction, then required `Unsupported`, unchanged target
bytes and continued acceptance of an ordinary dense source. The guard runs
before stage commit and source removal; it adds no native API or dependency.

Its immediate unchanged clean `RUSTUP_TOOLCHAIN=1.95.0 cargo make verify` passed
in **46.74 s**, including **453 Linux tests**, two established child-entry skips
and mandatory xwin check, Clippy and build. The evidence-only `19bed4f` preceding
it passed its clean gate in **45.28 s**. Earlier checkpoint bytes remain intact.

[Rust CI 40023012170](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012170)
passed with actual native
[Linux 453](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012170/job/120652082331),
[macOS 433](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012170/job/120652081889)
and [Windows 412](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012170/job/120652082357)
tests passed, two established child-entry skips each. The new refusal regression
ran on all three hosts. Exact MSRV, source quality and
[combined coverage](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012170/job/120653747493)
passed. [Policy](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012103),
[Source Package](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012173),
[GitHub Actions Security](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012136)
and [Advisory Quality](https://github.com/seapagan/terminal-file-manager/actions/runs/40023012327)
also passed.

Codacy at this exact source reports **91.28% diff coverage (5,432/5,951)** and
**−3.110 pp variation** under the existing accepted M2 decrease. The same three
generic unsafe-use alerts still block static analysis. Approval for the five
audited call-site scanner comments remains pending; no suppression or remote
disposition has changed. Only this append-only evidence follows the tested
runtime source. Final pushed-head checks belong in the ignored ledger and
maintainer handoff; PR #3 remains draft and unmerged.

## M2 Claude review corrections — 24 October 2026

The maintainer authorised the nine reviewed items on the existing M2 branch.
All five narrow native-call scanner comments and their function-scoped unsafe
allowances are approved; the tenth review finding is not a defect. Earlier
acceptance evidence remains historical, including its pending-approval wording.

Preview replacement and safe-exit entry cancel the superseded preview. Late
results cannot authorise its replacement. Retry eligibility uses the same
validation as action submission and caches the result when the job finishes.
Execution refreshes retry identities for both direct and Queue callers; preview
preparation refreshes before counting. Jobs rendering reuses the layout used
for hitboxes and retains only visible rows. A release-mode 2,500-result fixture
with 40-component paths measured bottom redraw at 40.61 ms before and 13.77 ms
after; its final row remains reachable beyond 65,535 wrapped rows. With a
2,000-entry preview, 100 former clone-based retry queries took 37.76 ms;
10,000 cached queries took 3.32 microseconds. These are local measurements.

Allocation checks permit up to 64 KiB of incidental growth for block rounding
and inline-data differences. Larger full materialisation, and expansion from
below half the logical size to at least half, still fail before commit. The
75%-allocated materialisation regression uses a 1 MiB logical file so its
256 KiB expansion exceeds that explicit allowance. Ordinary 1-byte through
1 MiB-plus-one Copy/Move fixtures verify length, every byte and checksum;
existing native sparse-hole, trailing-zero, cancellation and retention tests
remain. These checks do not promise identical extent maps or universal
compression preservation. NTFS-compressed sources now receive an explicit
preflight refusal; the [pre-v1 compatibility task](../TODO.md#ntfs-compressed-transfers)
requires native compression/allocation preservation before enabling them.

The final allocation correction replaces the half-size and full-size thresholds
with a direct growth limit for sources allocated below their logical length:
destination allocation may exceed source allocation by at most 64 KiB.
Dense sources retain their existing bypass for native preallocation differences.
This also refuses intermediate expansion, including 600 KiB
to 900 KiB for a 1 MiB file. Deterministic regressions cover unchanged and reduced
allocation, the exact 64 KiB boundary, one byte beyond it, full expansion and
counts near `u64::MAX`. The NTFS-compression preflight refusal remains unchanged.

The first signed state/UI commit passed unchanged clean Rust 1.95.0 full local
verification, including mandatory xwin check, Clippy and build: 458 Linux tests
passed, with the two established owned-child entry skips. Final-head native
and hosted proof will be recorded in the ignored ledger and maintainer handoff.
PR #3 remains draft and unmerged.

Directory claims now inventory native descendant identities without following
links or crossing known mount boundaries. Complete disjoint inventories allow
two independent directory jobs; shared objects, ancestry, private staging and
Trash namespaces still conflict. Incomplete or changed inventories retain broad
claims, including through quarantine. This is process-local coordination with
fresh validation, not isolation from arbitrary external writers. Windows
non-ASCII namespace comparisons still serialize where native case equivalence
cannot be established by the existing path policy.

Rename permits a destination that resolves to the selected entry only after
proving the same native parent and object, an exact selected directory-entry
name and no separately enumerated target name. Native exclusive/no-replace
rename remains mandatory. Success also requires the requested filename spelling
and object identity. Native fixtures cover Unicode case changes, files and
nonempty directories, hard-link refusal and source replacement. Windows/macOS
fixtures require an actually case-insensitive test volume; Linux exercises its
sensitive volume. No extra FFI or unsafe allowance was added. The Apple native
exclusive-rename path explicitly supports the same-entry case-only operation.

Exclusive rename maps ENOSYS, ENOTSUP and EOPNOTSUPP as unsupported mechanisms.
EINVAL retains its native error and errno because invalid ancestry and paths
can produce it. An actual descendant-rename regression reproduced the earlier
misclassification. No error enables an overwrite or copy/delete fallback.

Linux Trash now shares its immutable approval with the native adapter. Each
root receives a complete, cancellable, no-follow membership and metadata check
immediately before native submission. This replaces the separate whole-job
recapture and the per-root construction of another Preview map; direct
`LinuxTrash::put` callers retain a full fresh recursive guard. Distinct approved
roots can finish independently when another root changes. Native mount-table
evidence starts before preparation and covers the final walk and rename.

A deterministic native fixture reproduced successful recycling of a nested file
changed after confirmation validation. The corrected path refuses late writes,
replacement objects, new descendants and substituted directory symlinks, and
acknowledges cancellation without recycling. It preserves original/replacement
bytes and identities, the outside sentinel and existing Trash contents; owned
reservation metadata is cleaned up. Destructive claim acquisition reuses only
the immutable approved identities; execution still validates current scope.
Future destination names also claim their native parent identity, protecting
opaque bind/junction aliases, and private staging requires native identity.

For 5,000 owned files, the measured capture/approval/final-scope validation
subtotal was 203.41 ms before. The revised confirmation-plus-final-validation
subtotal was 134.90 ms; the removed scan measured 69.21 ms in the revised
fixture. These figures exclude coordinator acquisition, native recycling and
fixture creation; the final filesystem walk remains mandatory. No watcher,
shared modal framework, dependency or additional security exception was added.
