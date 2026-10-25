# TODO

## Pre-v1 compatibility

Enable permission changes for unreadable owned files and directories on macOS
before v1 without weakening object-identity or no-follow safety guarantees.
The current implementation deliberately fails closed. Never introduce an unsafe
pathname fallback.

### NTFS compressed transfers

Support native NTFS-compressed source transfers before v1. M2 refuses files
with `FILE_ATTRIBUTE_COMPRESSED` before writing their payload because the
current sparse allocation query does not establish compression fidelity.
The source and any previous destination remain intact.

Acceptance: create an actually compressed file with `compact.exe` on NTFS,
then prove Copy and Move preserve all logical bytes, native compression and
bounded allocation, including cancellation and destination collision. Until
that support exists, the native refusal regression must preserve both original
objects and their identities. See the
[Claude correction round](docs/acceptance.md#m2-claude-review-corrections--24-october-2026).

## Pre-v1 reliability

Define durable ownership, discovery, quarantine and reconciliation for orphan
`.tfm-stage-*` artifacts after a crash or panic. Current payload sync and
session-local recovery records do not establish ownership after restart.
Do not sweep stages by filename or erase uncertain payloads.

Before shipping, exercise process termination before and after stage commit and
source removal, missing records and replaced identities. Discovery must preserve
the only remaining payload, quarantine uncertainty, avoid deleting user files
and require validated evidence before cleanup or any new mutation. See the
[M2 correction checkpoint](docs/acceptance.md#m2-deepseek-correction-round--23-october-2026).

## Post-v1 TODO

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
| Terminal identification through SSH and multiplexers | `VTE_VERSION` may not reach TFM, leaving terminal-specific faint behaviour unidentified | Test whether forwarding `VTE_VERSION` is necessary and effective across SSH; compare GNOME Terminal, Ptyxis and Ghostty with and without forwarding; verify OpenSSH `SendEnv`/`AcceptEnv` behaviour and the effect of multiplexers; investigate terminal capability queries and reliable fallbacks. Only after confirming the results, add concise README troubleshooting with tested configuration examples and a verification command |
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

## Future Windows automatic refresh investigation

Windows currently requires `r` to refresh. Manual mode avoids persistent watch
handles and idle directory enumeration; it is the approved policy, not a hidden
polling fallback. Linux/macOS retain native automatic refresh.

Before revisiting Windows auto-refresh, read the private local archive at
`specs/windows-refresh-investigations-2026-10/FULL_REPORT.md`. This Git-ignored
path is unavailable in public clones. External readers can use the signed
[Windows polling performance report](https://github.com/seapagan/terminal-file-manager/blob/a8b205564ba6c2af272a271f719bdf6e4b2b8caa/research/windows-polling-report.md)
and [Shell registration isolation](https://github.com/seapagan/terminal-file-manager/actions/runs/40123944312).

On Windows Server 2025, per-directory `ReadDirectoryChangesW`/`notify` and Shell
registrations blocked ancestor renames despite deletion sharing. Shell-only
notifications missed ordinary external writes; interrupt delivery still blocked
renames and failed extended-path registration. Wider-root recursive watching
passed the owned rename topology but produced sibling noise, without whole-volume,
UNC, overflow or resource proof.

Two-second polling passed correctness tests but consumed about 17% of one logical
core at 2,000 entries and 72–75% at 10,000 in release TUI measurements. Identical
and hidden panes duplicated scans. Buffered NTFS metadata then returned stale
3-byte sizes where fresh inspection returned 35 bytes for hard links and open
writers; matching file IDs and birth stamps did not establish metadata freshness.
Do not repeat these approaches without new evidence addressing their failures.

Fresh one-handle inspection, scan coalescing, wider-root notification or opt-in
automatic refresh remain unselected alternatives requiring separate approval.
Before reintroduction, prove native Windows 11 desktop and Server behaviour,
ordinary external writers, raw/long paths and mount/UNC variation, genuine denied
cases, complete metadata parity, current/parent/grandparent rename during activity
and after cleanup, bounded CPU/resources, stale generations and independent M2
operation-time validation. A stable-fixture API benchmark alone is insufficient.

## Future Windows-focused PR: attributes and ownership

Add compact `RHSA` attributes with `-` for unset flags, retaining the existing
full hexadecimal display as an alternative. Propose compact RHSA as the Windows
default, subject to implementation review. Select the display through the
existing View-menu mechanisms and keep Info presentation consistent. Reuse
`lsplus` behaviour and tests where appropriate.

Include native SID/account resolution for Windows owner/group names and IDs in
the same PR. Cover local and domain users, unresolved identities, lookup and
access failures, and worker-side caching without blocking rendering. M1 keeps
`unavailable` for Names and Numeric modes until this work is approved and
implemented; `lsplus` does not implement account lookup.
