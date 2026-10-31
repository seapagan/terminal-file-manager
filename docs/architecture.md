# Architecture

## Implemented local browser and foundation

M0 includes the standard-library CLI, quality and security tooling, Unix and
PowerShell installers, binary/archive verifiers and the nonpublishing
eight-target release rehearsal. [Acceptance](acceptance.md) records the completed
native checkpoints and the remaining production-release gates.

`src/main.rs` owns native CLI arguments, output and exit status. `src/lib.rs`
parses help/version, native startup locations and explicit session options.
The M1 browser retains two independent pane models, with configurable one/two-pane
startup, bounded divider adjustment and temporary active-pane maximisation.
Cursor, scroll, marks, filter and back/forward history are session-only;
preferences persist lazily.

`app` owns pure transitions and request generations; `input` maps keyboard and
mouse to the same application actions. `ui` renders cached data through Ratatui,
using width-based geometry shared with hit testing. `colored_text` owns colour
capability/downgrade policy; a Labeldeck-derived adapter maps its public resolved
colours into semantic Ratatui styles. No backend calls occur in input or draw.

`fs` supplies no-follow local snapshots, platform metadata, refreshed ignore
rules and cancellable descendant totals. Exact PathBuf/OsString identities stay
separate from sanitized display text. Unix identity uses device/inode; Windows
uses native volume/file ID. The private Windows boundary has exactly two
approved function-scoped unsafe exceptions for read-only reparse-tag and
no-follow identity queries. Search handles use FindClose through RAII; File
handles use standard ownership. Neither adjusts privileges or changes files.
Unsupported reparse points remain visible and cannot be entered or recursed.

`workers` bounds filesystem work to six lanes (listing, metadata, totals for
each pane), with one running and one replaceable pending request per lane.
Metadata is debounced and shared by sidebar/popup; generations reject stale
listing, selection and count results. Cancellation is cooperative: a running
kernel read cannot be forcibly interrupted, and no mutation is submitted.
Traversal has a 128-directory depth bound and bounded error diagnostics;
errors, cancellation and overflow never become complete totals.

On Linux and macOS, `watch` keeps one logical native subscription per local pane,
including hidden panes, with nonrecursive current-directory and immediate-parent targets.
Two bounded background owners register, retry and release native watchers;
callbacks coalesce invalidations and errors into pane-local atomic flags.
Changing the directory context replaces those flags, so old callbacks cannot
invalidate the new context. Registration completion requests a catch-up scan.

The deterministic coordinator debounces changes for 200 ms, caps continuous
deferral at one second and spaces sustained scan starts by at least one second.
It retains late dirty state through an in-flight scan. Each pane also receives
an unconditional 30-second reconciliation on those platforms. Native errors request
one catch-up listing, then fallback intervals of 2, 4, 8, 16 and 30 seconds, capped
at 30 seconds. Repeated error hints do not request extra listings. Registration
retries use 1, 2, 4, 8, 16 and then 30 seconds; only 30 seconds of stable registration
resets that backoff and reports recovery with a catch-up listing. A successful
current-directory watch cannot compensate for a failed parent subscription:
TFM retains fallback to cover namespace replacement and rename.

A failed listing retains its unsafe state and retries after 1, 2, 4, 8, 16 and
then 30 seconds, capped at 30 seconds. These deadlines start when the coordinator
observes an idle failed listing; navigation and active listings take precedence.
Only an identity-verified successful listing clears the error. This bounds retry
work while allowing prompt recovery from a transient read failure. Degraded idle
changes may take 30 seconds to appear. Linux/macOS CPU cost has not been measured;
the historical large-directory CPU measurements concern Windows polling.

Windows uses intentional manual refresh. Its coordinator creates no native watch
owner and schedules no timed listing, reconciliation, retry or failure warning.
Both panes retain their last listing until deliberate refresh or navigation;
hidden panes do not stay automatically current. The `r` action refreshes the
active pane through the existing bounded workers.

All enumeration uses the existing six read-only lanes. Manual refresh and M2
completion use the same coalesced ordinary refresh intent; navigation retains
priority and its separate pending path/history state.

The Linux/macOS-only `notify` 8.2.0 has no event-kind configuration mask. TFM filters
access and access-time-only events in its adapter, along with unrelated parent
sibling paths. Its Linux backend still subscribes to open events. Notifications
describe possible changes; snapshots determine the displayed result.
Ordinary refresh requires the same physical directory identity, including
available creation evidence, and native namespace. Enumeration brackets that
evidence before accepting a snapshot. Missing or changed evidence retains the last
good view and blocks new mutation approvals. Explicit reopening establishes a new context. A missing
cursor chooses the nearest surviving row; marks require object continuity.
Metadata and count generations invalidate on changed snapshots and explicit
refresh. Recursive totals remain cached count snapshots, not continuously
updated descendant totals; explicit recount/refresh,
selection changes, replacement or errors invalidate them.

Linux/macOS watcher shutdown signals its owners without waiting for a potentially stalled
filesystem call on the UI thread. Normal owners release their watches on exit;
an owner stalled inside a kernel call releases them when that call returns.
Native notifications may miss changes on network or pseudo-filesystems. Periodic
enumeration supplies eventual reconciliation on Linux/macOS where enumeration
still works. Windows manual refresh avoids persistent watch handles and idle
enumeration, although deliberate listings still use transient handles and can
race external operations. Native directory watches blocked ancestor renames;
regular polling consumed substantial CPU in large directories. Buffered NTFS
records returned stale hard-link/open-writer metadata despite matching identities.
The original listing and independent M2 validation remain authoritative.
See [acceptance evidence](acceptance.md) for historical and current proof.
Remote refresh remains a separate M3 capability.

`terminal` restores attempted raw/alternate-screen/mouse/paste/keyboard modes on
normal exit, I/O errors and unwind. Keyboard protocol pushes are capability
negotiated and paired with pops. Unix selects Crossterm's level-triggered
use-dev-tty input source: its default Mio path can lose a keyboard readiness
edge when resize is returned first. Native PTY/ConPTY tests exercise resize,
quit and error/unwind cleanup. `settings` loads before terminal setup and saves
changed preferences after teardown through an owned same-directory stage.
Linked config files fail explicitly. `session` owns a separate strict version-1
navigation schema: current native locations and layout only. Unix byte arrays
and Windows UTF-16 arrays preserve identity without Unicode coercion. Q saves
after teardown through the same owned atomic staging; normal quit never replaces
session bytes. Restore is opt-in and validates both paths without fallback.

M1 never creates, renames, copies, moves, deletes, trashes, opens or launches a
selected file. Only deliberate preference/session actions write TFM's own
configuration data.
Viewer/editor, mutation and navigation integrations explain their later milestone
instead of silently falling back to a subprocess.

### Implemented pattern selection

`app::PatternSelection` captures the active pane's visible selectable native
names and identities, including directories. Its shared modal edits wildcard
or regex expressions independently of the fuzzy listing filter. Matching uses
native filename bytes rather than display text. Rendering caches preview counts;
Apply independently revalidates the captured directory and complete visible
identity set before changing any marks. Mark and Unmark affect matching entries;
Replace changes only the captured visible working set and preserves excluded marks.

An edit exceeding 4,096 bytes is rejected atomically and blocks all Apply routes
until deliberate correction or cancellation. Rendering, focus, mouse targets and
keyboard eligibility share measured modal geometry. Input stays within the active
form; a higher-priority operation/conflict transition consumes its triggering
event, and Ctrl+C retains Quit. Marked rows retain `[*]` independently of cursor
and focus styling.

## Planned application contracts

### M2 operation engine

`operations::Plan` captures native operands from the visible working set at
submission. It rejects pending listings, unresolved ignore-hidden scope and
changed marked objects. The shared coordinator acquires source-read and
destructive subtree/object claims together, with at most two active jobs.
Workers revalidate the captured objects; they do not consult later pane state.

The local transfer executor recursively merges directories and stages regular
files or links in private destination-local directories. It verifies byte counts,
size, applicable metadata and optional SHA-256 before commit. Conflict decisions
default to Skip; apply-to-all stays within one job and type pair. Keep both uses
native no-clobber commit. Unsupported atomic type replacements require Keep both
or Skip. Partial moves retain skipped children and their parent directories.

Rename and no-clobber stage/Trash commits use Linux `renameat2(RENAME_NOREPLACE)`,
macOS `renamex_np(RENAME_EXCL)` or Windows `MoveFileExW` with zero flags.
An unavailable native primitive refuses the operation; there is no
hard-link/unlink or cross-volume copy/delete fallback.

Regular-file transfers coalesce zero sectors into destination holes while
verifying logical bytes and optional SHA-256. Windows marks only the held owned
stage sparse when the source has the sparse attribute. Allocation validation
refuses substantial expansion of a sparse/compressed source before commit.
This preserves tested sparse space behaviour without promising identical extent
maps or compression. Destination access and modification times come from held
source metadata captured before content reads; timestamps are applied after
checksum reads. Metadata/allocation failures preserve the source.

New Unix transfer directories request mode `0700` at creation, before receiving children.
Only a newly created, fully populated directory receives the source's final
permissions, after fresh source-parent and object checks. Unix final modes come
from the frozen source snapshot. Cancellation and incomplete children retain private ancestors.
Merges preserve existing directory permissions, including permissive parents;
they do not promise to make those parents private. A retry treats directories
retained from an earlier attempt as existing merge targets and preserves their
private mode. Directory finalisation uses the existing no-follow attribute
descriptor. Incomplete parents retain source/destination/parent evidence and
completed child records. Validated retained sources produce partial outcomes;
uncertain identities stay unknown and quarantined. A retry carries directory
evidence into worker-side validation before any new mutation; retaining evidence
alone does not authorize retry. Reconciliation rechecks ownership without
repeating source removal.

Linux permission edits pin the selected object with `O_PATH | O_NOFOLLOW` and
change its mode through the held descriptor's `/proc/self/fd` reference. They
require accessible procfs, but no content-read permission. Parent/namespace and
selected-object checks precede chmod; a replacement name cannot retarget the
held descriptor. Windows retains its native attribute handle. macOS keeps its
no-follow read handle and rejects unreadable objects, as the maintainer approved
on 23 October 2026. Native tests showed that `O_EVTONLY` could not remove the
read requirement. A pathname fallback would weaken the selected-inode guarantee
and is not permitted. Native regressions check unchanged modes/identity on
rejection and descriptor safety for readable macOS objects.

Workers check parent identity and namespace around reads, commit and source
removal. A move removes each source only after destination commit and fresh
source/destination checks. Cancellation waits for owned cleanup; uncertain
cleanup or removal retains recovery evidence and reports OutcomeUnknown.
These checks do not isolate files from external writers or detect every
same-size/time change. Checksums describe the verified read, not an atomic
source snapshot. Crash durability remains filesystem-dependent.

The browser submits pure operation intents to a bounded coordinator, with at
most two mutation threads. Progress is coalesced; conflicts and terminal records
remain reliable. The terminal restores before queue teardown waits on writers.
Destructive previews run on workers and need separate confirmation. Jobs retain
skipped, failed, partial, cancelled, unknown and critical outcomes separately.
An all-skipped or empty result does not count as completed work. Retry and
reconciliation controls share guards with the browser bridge and queue;
reconciliation requires an unknown entry with retained ownership evidence.
Unknown outcomes quarantine claims and block retry/exit. Q freezes the navigation
snapshot before waiting for acknowledgement; a new exit request abandons the
earlier waiting intent.
While an exit waits, visible conflict and Jobs controls remain usable; ordinary
pane actions, retries and reconciliation stay blocked, including under
diagnostics. Esc cancels a visible conflict, closes Jobs, or abandons a pending
exit when neither surface is active. Abandoning exit discards its frozen session
snapshot. Diagnostics hides conflicts without taking their input; Esc there
abandons the pending exit. Keyboard and rendered mouse controls share these
rules. A conflict reply alone does not acknowledge worker completion.
Reconciliation runs under the original quarantined claims. It checks retained
identities and cleans only proven owned staging; missing evidence keeps the job
unknown. It never repeats source removal.

An observed native permanent deletion during Trash remains a critical incident,
with its record and claims intact. After all mutation workers acknowledge
completion or cancellation, a separate exit dialog accepts manually typed
`ACKNOWLEDGE` followed by F10. Enter/Esc stay in the manager; paste and key
release/repeat events cannot confirm. An ordinary unknown outcome blocks this
path, including when it accompanies an incident. Acknowledgement does not
reconcile an incident, retry a mutation or write a recovery file. Terminal
restoration precedes navigation-session persistence. Job diagnostics remain
session-local.

`ui::modal` supplies the shared measured shell for operation dialogs and floating
metadata. Content rows wrap prose and fields, while structured file/result rows
truncate filenames and right-align indivisible sizes. Lexical common parents
reduce repeated source paths without changing native operands. Standard action
groups return typed intents with safe defaults; M2 controllers retain approval,
preview, conflict and incident authorization. The docked metadata sidebar keeps
its own layout and shares its existing fields and cache with the popup.

File confirmations show selected roots, with type indicators and directory
suffixes. Recursive Trash/Delete totals describe the completed worker preview;
they do not expand the visible root list. Context and editable fields stay above
the list, while errors, the permanent-delete warning and actions stay below it.
PgUp/PgDown and wheel input over the list scroll its measured viewport. Short
dialogs shrink to content; long lists normally use at most 75% of terminal
height. Small terminals may exceed that height target to retain fixed controls.
The shared shell centres its bounded height in the original usable terminal
area. Scroll cues occupy a measured interior gap; a muted context divider
collapses before it can remove the last operand row. Trash/Delete controls
remain visible while previews run, but stay disabled until a completed preview,
the required context/warnings and at least one readable operand row are visible.
Mouse hit-testing and the submit guard use that same measured eligibility.
Standard preview errors have a reserved layout budget to keep the controls
stable through counting, success and failure. Unused space collapses before
operand rows disappear; longer errors may need more room.
Enlarge the terminal or cancel when approval is disabled.
Editable values retain their normal style and show a caret at the editing end;
long values scroll horizontally without changing the underlying input.
Create directory and Create empty file share the same form, field editor,
measured geometry and controls. Both disable Create for empty or invalid names;
the parent path uses the muted semantic role. Shared `name_valid()` and
`name_error()` methods supply form eligibility and submit feedback while retaining
distinct directory and native file-name rules. Windows file-name feedback
identifies reserved device names, prohibited characters and trailing dots/spaces
before generic path checks, including drive-like input such as `a:b`.

CreateFile uses the existing operation plan, directory guard, coordinator and
Jobs results, with native exclusive creation preserving collisions. The plan
captures the dialog's originating pane and retains it through Jobs retries;
registration does not redirect focus to whichever pane is active. A per-pane
cursor intent waits for success and a subsequent listing. Ordinary refresh
failures retain it, and completion during an older scan schedules a deferred
refresh. Navigation or newer cursor/selection changes discard the intent;
listing visibility rules still apply. The footer retains its existing layout
with compact `F7 Dir` and `Alt+N File` labels; `Alt+Enter Info` remains visible
and clickable at 80 columns.

Copy/Move align From/To values and describe the next SHA-256 toggle action in
the F2 button.
Detailed floating metadata retains the same available native sizes as the
sidebar; a compact directory row's em dash denotes an uncomputed payload size.

Operation dialogs size to wrapped content within terminal bounds. The renderer
and input controller share exact button, field and job-row rectangles. Jobs use
a bounded selection viewport and a separate details scroll position; completed
execution duration freezes at acknowledgement. Display sanitization preserves
the model's native paths. Detail offsets use full-size row indices and rendering
passes only visible rows to the terminal widget. Geometry and rendering reuse
the same wrapped layout for each draw. Non-Press events stay inside active
operation surfaces, including paging while safe exit waits. Non-Unicode Rename
shows escaped native text and requires an explicit replacement edit; display
text never substitutes for the selected operand. The existing Notice queue holds
at most two five-second
success/warning/error notices without taking focus. Rendering uses the existing
status row and semantic Theme roles; a non-expiring incident warning takes
precedence. Jobs retain request failures and worker evidence after notices
expire. These operation surfaces do not implement the later generic status
framework.

Future milestones preserve native local byte/UTF-16 path identities separately
from display text. Remote POSIX paths remain exact bytes on every client OS;
Windows native paths are not remote identifiers. Stable entry/job identities,
generation-tagged asynchronous results and bounded queues prevent stale results
from authorizing mutations. Navigation sessions remain distinct from named
view profiles: profiles contain only display/sort/visibility policies, while
sessions contain local locations/layout. Full navigation history persistence
remains outside the implemented contract. Future preference/bookmark/history
writes must remain lazy and atomic.

The selected future transport is embedded russh with ring, RSA disabled,
and Yazi's byte-preserving SFTP boundary. It requires corrected opaque handles,
acknowledged close/write, bounded session resources, cancellation/teardown,
trust/auth/config and native compatibility proof. System SSH/SFTP executables
are optional integrations, not the core backend. The
[selected-dependency handover](m0-dependency-feasibility.md) records accepted
eight-target feasibility, the required HANDLE correction and the remaining
production obligations. Adopt dependencies only after checking the shipping
source, compiler, security, licence and native graph.

Mutations use owned staging and three layers: exact identity where guaranteed,
fresh best-effort revalidation at every boundary, and explicit residual race
limits. Check-then-rename/remove is not an atomic conditional operation.
Detectable change, uncertainty or an ambiguous sent request blocks destructive
finalization and source removal. Cancellation after send preserves unknown
outcomes for reconciliation; retries never invent safe completion.

Linux Trash uses a checked same-filesystem rename into home Trash or Freedesktop
per-volume storage. Per-volume discovery resolves the physical source parent,
refuses ambiguous stacks and source-ancestor/subtree bind topology, and validates
sticky shared or private
user storage before reserving matching payload/metadata names. It never follows
the selected final link or uses cross-device copy/delete. Native Trash results
retain platform-specific recovery references. One top-level acknowledgement does
not fabricate descendant operations: recursive inventory is labelled as validated
before the move. Unknown results retain locks; a native permanent-deletion report
during F8 is a critical safety incident. Permanent deletion needs separate explicit
confirmation.

Stage construction cleans only a newly owned, revalidated empty directory on
ordinary failure. Foreign content or uncertain ownership preserves the artifact
and reports uncertainty. Crash recovery has no durable ownership catalogue or
startup sweep; the pre-v1 reliability item in [TODO](../TODO.md#pre-v1-reliability)
defines that remaining contract.

macOS calls Foundation's native Trash method with lossless filesystem URLs and
keeps successful recycling separate from its optional resulting URL. Windows
uses a dedicated STA, one frozen Shell item and a non-agile progress sink. It
requests recycling/undo, checks per-item acknowledgements and abort state, and
extracts an opaque recovery reference while the returned Shell item is live.
Optional metadata failure does not erase a recycle acknowledgement. No auto-Yes
flag authorizes permanent deletion; unexpected native Shell behaviour remains a
residual risk. A successful delete callback with no recycled item is a critical
incident, never Trash success. Native recoverability tests use owned fixtures and
supported Shell moves; Linux cross-builds do not establish this native proof.

V1 transfers include local/embedded-SFTP paths and explicitly approved, labelled
client relay. Genuine server-to-server transfer is post-v1. A relay is never
reported as direct. The default scheduler permits two jobs. Viewer/search/
comparison must remain bounded and cancellable; incomplete or failed scans never
prove equality or authorize deletion. Optional capabilities expose unavailable
reasons without preventing unrelated basic browsing.

M2 implements local operations on the M1 browsing foundation. M3–M8 add the
remaining application contracts, including SSH sessions.

Jobs uses a responsive viewport derived from terminal size, with fixed history,
safety and action regions. Job selection, action eligibility and Summary/Evidence
changes leave those rectangles fixed. Summary projects the frozen plan and
record into top-level rows, one meaningful lexical source root and an applicable
destination. It shares component-aware path elision with pane titles. Evidence
retains per-entry outcomes, full source paths, unique native receipt targets and
recovery metadata; display labels never become operation operands. Switching
jobs or detail mode resets detail scroll. Each new Jobs opening starts in Summary.

The browser requests animation redraws every 150 ms while a destructive preview
is visible and pending. Monotonic elapsed time selects one of four ASCII frames;
readiness, worker generations and destructive confirmation remain independent.
Hidden, ready, failed and cancelled previews request no animation redraws.

Jobs centres Evidence/Summary and Close in a stable primary action row. A second
reserved band centres only eligible contextual controls, using the shared action
packing and hit rectangles. At constrained sizes, those controls become
informational shortcuts in the existing footer. Whole shortcut labels pack into
at most the available two footer rows; an ellipsis marks omitted hints. Warnings
retain their first row, including unknown-outcome retry/exit restrictions. The
renderer creates no hit targets for shortcut hints or unused action-band space.

Conflict dialogs present cached Incoming/Existing snapshots as side-by-side
columns or stacked fields. One meaningful lexical root replaces repeated
absolute paths. Comparison paths reuse component-aware title elision and share
a grapheme-safe endpoint fallback for oversized final components; pane/Jobs
formatting retains its existing policy. Semantic key/value rows create no input
targets. UTC display omits fractional seconds, while newer direction compares
the full frozen SystemTime values under the recorded availability contract.

Replace/Newer controls remain visible but disabled when their existing guards
reject them. Arrow navigation skips those choices; keyboard and mouse decisions
still pass the existing input checks. Skip starts selected for each prompt.
Apply-to-All uses a neutral, fixed-size On/Off control. The modifier wraps the
actual submitted choice and retains current-job/type-pair scope. Replace and
Newer require the measured filename/root and comparison rows to fit; both input
routing and mouse eligibility enforce this guard. Skip and Cancel remain safe
responses when comparison context is hidden. A new visible conflict resets detail scrolling; clipped conflict
footers prioritise safety warnings. The filesystem engine and native identities
remain unchanged.

Copy/Move workers retain execution-time overlap and native-identity checks.
Refusals distinguish the same output item, a source folder containing its
output, and aliases of the same native item. Renamed outputs and Keep both keep
their existing eligibility; the planner does not pre-reject an entire batch.
Notifications show a recorded primary reason, prioritising uncertain outcomes,
with the existing lifecycle-derived counts. Jobs Summary groups exact recorded
(outcome, reason) pairs once with an affected count, before the operand rows.
It preserves distinct causes and uncertainty restrictions. Evidence retains each
full individual reason, native source/target spelling, receipts and recovery
records. Mixed/special-object selection summaries use neutral entry counts;
recursive preview accounting and lifecycle-derived job counts are unchanged.

## Categorised Help

Help groups the effective action keymap into one document. Measured wrapped rows
supply section/action anchors, scrolling bounds and landing highlights. The
shared modal shell contains the temporary fuzzy jump picker; it keeps the main
document and its scroll position. Search ranks title/action matches before keys
and fuzzy descriptions, retaining document order for ties. Input routing consumes
picker events before browser and operation handlers.

Shared modal specifications opt into a measured title-border Help control.
Narrow shells keep their title and the F1 keyboard route. Context Help stores
only its topic and scroll; the parent retains its fields, focus, selection and
geometry. Its read-only input layer consumes events before the parent, while
the existing exact Ctrl+c safe-exit route remains available. Fixed dialog keys
use lowercase labels for case-insensitive controls; uppercase-only destructive
confirmation keeps its existing restrictions.

Effective shortcut hints share the same preferred-alias lookup as the footer.
Metadata count/cancel/refresh hints, its measured Close action and Jobs detail
routes follow configured bindings. Shared modal actions own their display labels
so runtime key text uses the same measurement and hit regions as fixed controls.

Help section headings use the dedicated SectionHeader role: light cyan
(`#89dceb`) resolved for the terminal, bold, and underlined in monochrome.
Create dialogs present the parent as a Label/Value row. Informational progress, search feedback and reasons
for unavailable actions use readable text roles; disabled controls retain the
Disabled role. Other theme roles and operation eligibility remain unchanged.

The jump picker gives sections one selectable row and commands a title with a
separate shortcut/category line. Its plain query field and separator sit outside
result hit regions. Measured result heights keep the selected entry in view.
Context Help carries literal shortcut fragments through Help-local word wrapping
and semantic Key styling. Long tokens fall back to existing grapheme-safe cell
wrapping; metadata and other shared text renderers keep their existing rules.
