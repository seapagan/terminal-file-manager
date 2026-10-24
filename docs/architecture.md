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
