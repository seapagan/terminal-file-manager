# Architecture

M0-B has a standard-library CLI entry point and a library dispatch seam.
`src/main.rs` owns OS argument collection, output and exit status;
`src/lib.rs` implements help/version/argument dispatch. There are no dormant
file-manager modules or fabricated feature tests.

Future milestones preserve native local byte/UTF-16 path identities separately
from display text. Remote POSIX paths remain exact bytes on every client OS;
Windows native paths are not remote identifiers. Stable entry/job identities,
generation-tagged asynchronous results and bounded queues prevent stale results
from authorizing mutations. Startup is always one pane; navigation/session
state is not persisted. Only preferences, bookmarks and successful manual
histories may persist, lazily and atomically.

The selected future transport is embedded russh with ring, RSA disabled,
and Yazi's byte-preserving SFTP boundary. It requires corrected opaque handles,
acknowledged close/write, bounded session resources, cancellation/teardown,
trust/auth/config and native compatibility proof. System SSH/SFTP executables
are optional integrations, not the core backend. No dependency is added until
its exact compiler, security, licence and eight-target graph gates pass.

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

These are future implementation contracts, not completed M0-B functionality.
