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
partial outcomes and retry lineage; unknown outcomes quarantine claims and block
retry/exit. Q freezes the navigation snapshot before waiting for acknowledgement.
Reconciliation and final native acceptance remain in progress.

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

Trash remains mandatory. Initial Linux support is restricted to validated
same-filesystem rename preserving link identity, with mount, ownership,
permissions, collision and identity checks. Unsupported/cross-device/uncertain
paths fail explicitly. No silent permanent-delete fallback is allowed; permanent
deletion needs separate explicit confirmation. Native macOS/Windows Trash and
safe cross-device behaviour require their own proof.

V1 transfers include local/embedded-SFTP paths and explicitly approved, labelled
client relay. Genuine server-to-server transfer is post-v1. A relay is never
reported as direct. The default scheduler permits two jobs. Viewer/search/
comparison must remain bounded and cancellable; incomplete or failed scans never
prove equality or authorize deletion. Optional capabilities expose unavailable
reasons without preventing unrelated basic browsing.

M2 implements local operations on the M1 browsing foundation. M3–M8 add the
remaining application contracts, including SSH sessions.
