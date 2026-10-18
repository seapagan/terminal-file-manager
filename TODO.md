# Post-v1 TODO

These items are deferred contracts, without dormant implementations. Each needs
separate approved scope and review; none removes a v1 acceptance obligation.

| Priority / scope | Rationale | Acceptance before shipping |
| --- | --- | --- |
| First: byte-level transfer resume | Avoid repeating large transfers while preserving integrity | Persist/validate stage and source identities and verified prefix; changed sources, capability mismatch, cleanup, committed destination preservation; local/SSH and approved relay; direct only after its separate route exists |
| First: one-way Update and Mirror together | Reuse comparison/planning/execution with explicit deletion semantics | Preview, direction, fresh validation and opt-in Mirror; Update keeps destination-only entries; Mirror uses Trash-first planned removals; local/local, local/SSH, SSH/SSH cancel/partial/recovery/error tests; optional rsync never bypasses verified plans or route approval |
| Genuine direct SSH-to-SSH transfer | V1 explicitly labelled relay is sufficient; exact-path and supervision proof is incomplete | Both directions: existing trust/auth, exact-byte namespace/frame/stage, independent payload-bypass accounting and relay control, finite owned worker identity, reachable cancel acknowledgement and partition reconciliation; no forwarded/copied credentials, silent route switch or unverified removal |
| Internal text editor | Keep v1 read-only viewing focused | Evaluate embeddable component; undo, dirty state, search, staged save and conflict tests |
| Remote external editing | Requires a safe upload lifecycle | Download/edit/revalidate/upload/commit and cleanup; no silent overwrite |
| Keyboard/right-click context menu | Additional action access beyond v1 dispatcher | Shared dispatcher, visible disabled reasons and accessible navigation |
| Configurable pane-header path display | Choose absolute paths with smart middle elision (default) or paths relative to an explicit base, likely startup; consider basename-only if justified | Design the base, navigation outside it, independently navigated panes and Windows roots before implementing a display toggle |
| Detached transfers and persistent queue | Requires durable ownership and recovery | Reconnect/orphan/cancel lifecycle, desktop/OS notifications including Wayland/headless fallback; no v1 daemon or OS notifications |
| Recursive content search | V1 searches filenames | Optional local/remote ripgrep with safe framing and usable fallback |
| Bidirectional synchronization | Needs history and conflict/deletion model | Separate model after one-way Update/Mirror; conflict, stale history and deletion/recovery tests |
| Syntax highlighting and hex viewer | Optional richer viewing | Preserve bounded memory and text/terminal safety |
| Extract lsplus or git-diff-out libraries | Only if reuse proves worthwhile | Demonstrate stable filesystem/clipboard boundary and consumer tests; no v1 extraction mandate |
| Advanced actions, pane-contained previews, richer clipboard or agent forwarding | Need a later explicit product/security request | Reviewed action semantics, capability errors and targeted security/native proof |

Multimedia/image/Kitty/Sixel previews, implicit external execution/launchers,
embedded terminal, plugin/scripting ecosystem, drag-and-drop and cloud backends
remain excluded from v1. No sync/resume/editor implementation is added merely
for future readiness.

## Deferred native Windows ownership

Resolve Windows owner/group accounts with native SID/account semantics in a
separately approved change. M1 intentionally displays `unavailable` for both
Names and Numeric modes on Windows; lsplus does not implement this lookup.
Cover local/domain and missing/unresolvable accounts, access failures and
worker-side caching on native Windows without blocking rendering.
