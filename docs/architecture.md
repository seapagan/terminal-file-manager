# Architecture

## Implemented local browser and foundation

M0 includes the standard-library CLI, quality and security tooling, Unix and
PowerShell installers, binary/archive verifiers and the nonpublishing
eight-target release rehearsal. [Acceptance](acceptance.md) records the completed
native checkpoints and the remaining production-release gates.

`src/main.rs` owns native CLI arguments, output and exit status. `src/lib.rs`
parses help/version, native startup locations and explicit session options.
Help has aligned short/long options and an interactive-help hint; version output
includes the executable name, package version, copyright and manifest licence.
Both use `colored_text`'s stdout policy. Closing an output pipe early exits
quietly; other stdout write failures produce a normal CLI error.

`cli_diagnostics` renders stderr errors with the `tfm:` prefix and exit code 2.
It normalises CRLF to LF, preserves blank/trailing lines and applies `ui::clean`
to each line, leaving the TUI sanitiser unchanged. Standalone carriage returns,
terminal escapes and unsafe Unicode controls remain escaped. Semantic styling
distinguishes headings, unambiguous absolute paths, numeric gutters and pointers.
Only the explanation immediately following a pointer receives summary styling;
an intervening gutter clears that state. Explicit quoted valid-option lists
receive an accent. The stderr colour policy, including `NO_COLOR`, remains owned
by `colored_text`; plain and coloured output retain the same diagnostic text.

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
navigation schema: current native locations, relative-path base and layout.
Unix byte arrays
and Windows UTF-16 arrays preserve identity without Unicode coercion. Q saves
after teardown through the same owned atomic staging; normal quit never replaces
session bytes. Restore is opt-in and validates both paths without fallback.

The M1 browsing workers do not mutate selected files; completed M2 operations
use the separate planner and executor below. Only deliberate preference/session
actions write TFM's own configuration data. Unimplemented viewer/editor and
direct-path/zoxide actions explain their later scope instead of falling back to
a subprocess.

### Implemented path navigation

The virtual `..` row uses a native parent cursor target outside `entries` and
`visible`. Listing row coordinates include it; selection counts, marks, metadata
and operation operands continue to use filesystem entries. Enter and double-click
reuse `App::parent`, including identity-aware exited-directory restoration.

Header geometry projects the existing native path elision onto lexical ancestor
slices. Display text never reconstructs a navigation target. A one-cell ellipsis
control opens the shared full-path shell; complete components wrap where possible,
and oversized components wrap on grapheme boundaries. Its pane/path-bound state
captures input and yields to operation approvals and the existing Ctrl+C exit flow.
The configurable `FullPath` action (Alt+l) opens the same picker for the active
pane regardless of truncation. Both routes select the current directory and use
the measured terminal area to reveal it. Immediate Enter normally closes without
a listing request. An explicit current-directory choice supersedes any pending
navigation. Successful directory changes close that pane's picker and child Help
before the next input event. Ctrl+l retains the reserved manual-path action.
The picker registers a `FullPath` context-Help topic and uses the shared title
Help control, prose renderer and input layer. Help closes before its parent;
shared resize handling retains a visible path selection when Help closes.
The same semantic `PathHover` foreground resolves through `colored_text` on both
surfaces. `path_hover` configures its colour; monochrome uses bold. No persistent
row, new navigation engine, dependency or clipboard behavior is introduced.

Both panes use the same cyan breadcrumb foreground. `InactivePaneBorder` gives
the inactive border a neutral grey foreground, configurable with
`inactive_pane_border`; the active border retains `Header` cyan and bold.
Monochrome keeps the inactive border plain and the active border bold. Border
styling leaves pane contents and path hover unchanged.

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

## Local operations and future application contracts

### Implemented M2 operation engine

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
text never substitutes for the selected operand. Operation outcomes use the
shared status and notification owner described below. Toast expiry and dismissal
leave Jobs evidence and session history intact; unresolved incidents retain
their independent warning and safe-exit controls.

Future milestones preserve native local byte/UTF-16 path identities separately
from display text. Remote POSIX paths remain exact bytes on every client OS;
Windows native paths are not remote identifiers. Stable entry/job identities,
generation-tagged asynchronous results and bounded queues prevent stale results
from authorizing mutations. Navigation sessions remain distinct from named
view profiles: profiles contain only display/sort/visibility policies, while
sessions contain local locations, the captured relative-path base and layout.
Full navigation history persistence
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

Help groups the effective action keymap into one document. One metadata table
owns each action's description, section and display priority; row order puts
common commands before specialised and unavailable commands. That definition
generates an exhaustive Action match, so missing metadata fails compilation.
Effective bindings still come from the live keymap, independently of the Action enum order. Measured wrapped rows
supply section/action anchors, scrolling bounds and landing highlights. The
shared modal shell contains the temporary fuzzy jump picker; it keeps the main
document and its scroll position. Search ranks title/action matches before keys
and bounded word abbreviations, retaining document order for ties. Fuzzy matches
start at a word boundary and keep at least half the spanned letters; separate
fields cannot combine scattered letters into a result. Phrase words may appear
in either order within a description. Input routing consumes
picker events before browser and operation handlers.

The shared modal shell measures a safe title-border close control and an optional
Help control, displayed as `[?]─[×]`. A close click enters the owning layer
through its existing Esc route, including preview cleanup and safe exit. Controls
that cannot fit beside the full title stay hidden without resizing the shell.
View Options, View Profiles and Save Profile also use the shared measured shell.
Their menu rows and title controls use its body and header rectangles for input;
Save Profile keeps literal question marks in the name field and cancels back to
the profile list.
The default ? and F1 bindings open the same Help: the main document from the
file manager and contextual Help in applicable dialogs. Printable characters
remain literal in text fields; F1 supplies Help there and in narrow shells. Context Help stores
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

## Responsive status and session notifications

PM3-05 is complete and merged through [PR #10](https://github.com/seapagan/terminal-file-manager/pull/10)
at `1ece1a1`, with final feature head `b924083`. The [acceptance record](acceptance.md#pm3-05-accepted-and-merged--6-november-2026)
records completed independent reviews, maintainer acceptance and feature/merge
verification. [PR #11](https://github.com/seapagan/terminal-file-manager/pull/11),
merged as `41a913e`, subsequently narrowed notification delivery to noteworthy
events. PM3-06 path navigation is complete through
[PR #13](https://github.com/seapagan/terminal-file-manager/pull/13), merged as
`56bf891`; its [acceptance record](acceptance.md#pm3-06-accepted-and-merged--8-november-2026)
records the final feature-head verification. PM3-07 OS path-text copying is complete
through [PR #14](https://github.com/seapagan/terminal-file-manager/pull/14), merged
as `7e55683`; its [acceptance record](acceptance.md#pm3-07-accepted-and-merged--12-november-2026)
separates feature-head and merge-head proof.
The internal file-operation clipboard is deferred indefinitely; snapshot testing
remains deferred.

The application owns notification events independently of filesystem jobs.
Toasts reference those events in chronological order, with newer groups nearest
the configured terminal corner. Complete cards and spacing use at most 80% of
terminal height. Before drawing, the runtime retires displaced presentations and
presentations that cannot fit a tiny terminal; they cannot return after dismissal
or resize. Rendering only reads state. Expiry and dismissal retain session history. Operation notifications
link to Jobs records for full evidence and recovery actions. Application events,
including safety-related rejected requests and filesystem-watch warnings,
create no filesystem jobs. Routine input feedback, pattern marking and profile
changes use existing status or inline form fields without recording an event.
Pre-submission name/path validation and disabled controls stay contextual;
changed operation scope, failed previews and queue rejections remain retained
notifications. Empty and NUL-containing destinations receive distinct inline
messages; editing a form clears its stale validation error. Pending destructive
previews request waiting, while failed previews require cancellation and reopening.
Neither route enables confirmation early. Save-profile name errors remain in
that dialog, and marking feedback reports the resulting visible selection count.
Concise toast titles/messages remain separate from
retained diagnostic text. Within two seconds, matching failures on the same
native operation targets share a toast across Job IDs, with occurrence counts;
each Job keeps its own history link and evidence. Repeats extend the same group
when each arrives within two seconds of its predecessor. Indexed event lookup
and stored group counts avoid scanning session history during publication or
toast rendering. History remains unlimited for the session. A newer result for a job
replaces its pending toast while history keeps its earlier result notification.

The status composer orders structured current-context, selection, Jobs and
history segments by priority, with shared render/hit-test geometry. It uses one
row by default and at most two for critical information; short terminals and
open modals preserve their content space. Critical incidents and unresolved
outcomes remain independent of notification expiry. Notification history uses
the existing measured modal shell for a compact selectable list. The detail
toggle retains full diagnostics; Enter opens the selected Job's evidence. Esc
returns to the underlying surface. Incoming events update the open viewer and
follow the newest selection without disturbing an older selection or its detail
scroll. Opening history clears pending toasts; events received there remain in
history without subsequent toast presentation.

Toasts render above other interfaces, including critical-incident surfaces.
Mouse hit-testing and configurable global dismissal actions run before modal
input. A left click dismisses one visual group and consumes the event. Alt+k
dismisses the oldest visible group; Ctrl+Alt+k clears all pending presentations.
Neither action changes operation state, retained events or Jobs evidence.
Passive per-entry metadata diagnostics stay inline; accepted worker failures
and listing errors retain Error notifications. Watch recovery remains Info.
Placement and expiry use the existing strict TOML settings and atomic save path.

## Footer workflows and OS path copy

`workflow::Flow` owns ordered conditional stages, enabled-choice defaults,
highlights, Backspace, cancellation, physical key/click history and the final
semantic choice path. Direct letters and Enter activate choices; arrow and Tab
keys are consumed without traversal.
Consumer predicates select relevant stages from the accepted semantic path; Back
visits the actual stage history. Definitions require an enabled starting choice.
If a reachable later stage has no enabled choices, the engine reports the blocked
stage and restores both stages' prior highlights without accepting a partial path.
Consumer-marked completing choices bypass later stages and invoke the same
one-shot callback with the accepted semantic path. The callback's generic result
supports distinct actions without domain branching in the engine.
Letter shortcuts distinguish lowercase and Shift
using the shared binding normalization and explicit Shift+letter display.
Its opaque context and one-shot handler keep it independent of filesystem and
clipboard rules. The measured workflow renderer shares complete text-cell
rectangles with mouse input and occupies only `Geometry.footer`. Pane/status
allocation and scroll state remain unchanged. Choices share square brackets;
selected choices retain bold/reverse emphasis, unselected brackets use Muted and
disabled choices use Disabled. Clicking a disabled choice returns its existing
reason through the generic engine. Resize events reach normal viewport handling
while the flow retains keyboard and mouse ownership. Higher input owners invalidate
the flow before the next draw; global toast dismissal retains priority.

The engine retains at most 1,024 physical inputs, including activation, for its
completion callback. A successful callback receives the complete ordered history.
The next event after the limit cancels with an explicit error and no callback;
the engine does not truncate a successful history. Backspace at the root cancels.
The renderer reserves the Esc mouse target when at least three cells are available,
and Backspace plus Esc when a non-root stage has at least thirteen cells.

The path-copy consumer presents Copy Path / From and captures the originating
pane, native highlighted and
visible marked identities, current directory and relative reference. Startup
captures the first explicit CLI directory (or process cwd without arguments) as
an absolute native startup base. The selected relative base is session state.
From offers Marked, Highlighted and Current folder sources followed by the Format stage.
Each format completes immediately, including Relative using the captured session
base. Set base and Other base complete directly from From with a distinct
base-set outcome: they validate and capture a native directory path, confirm the
change and do not queue clipboard text. Other base requires a valid logical
opposite pane and revalidates its snapshot before setting the base.

Activation captures cached directory identities and waits for a valid listing
when identity evidence is unavailable. Completion checks current pane/selection
state using a visible-path index, then queues frozen work. The existing clipboard
worker revalidates lexical and physical directories and selected entries without
owner/group resolution, constructs the bounded Unicode payload and invokes a
provider only after the whole batch passes. Relative copies also open the selected
base directory in that worker to check existence, directory type and read access;
failures preserve the selected base and clipboard. The lexical calculation retains
the original native spelling, including directory links. Name and Full path do
not validate the relative base. Base actions use the same worker;
the UI rechecks pane context before applying a validated base result. Lexical
relative paths never follow symlinks or cross incomparable roots. Capturing the
opposite pane works while
maximised or hidden. Subsequent navigation cannot change a captured base.
Normal path components retain exact case during lexical relative-path comparison.
Windows supports per-directory case sensitivity, and macOS volumes may be case
sensitive. Global case folding could therefore select a different path. TFM keeps
exact component comparison without filesystem queries; paths can be non-minimal
on case-insensitive filesystems. Windows drive-prefix comparison remains unchanged.

Existing session saves persist native path units; explicit/automatic restoration
uses that path, with the first restored pane as the fallback for older sessions.
Ordinary exit does not save it.
Remote source identity and host-qualified output require a separate future
consumer; local paths never masquerade as remote references.

The clipboard router adapts git-diff-out's provider and fallback implementation
and tests. External providers use literal stdin and bounded command/output I/O.
One background request at a time keeps provider waits outside event processing.
OSC52 preparation returns bytes to the terminal owner for serial emission between
frames, and reports sent with acceptance unconfirmed. Normal exit drains pending
work for at most four seconds before terminal teardown; a blocked worker is not
joined, and timeout/error reports remain visible after terminal restoration.
External-provider EPIPE closes stdin while preserving stderr/exit diagnostics,
the original deadline and failure process-group cleanup. Local providers report
copied only after a successful invocation. Preflight failures never invoke a
provider or clear the clipboard. Clipboard settings use the existing strict,
atomic preference storage. In-progress workflow state never persists.
