# PR #4 review dispositions

The remediation preserves filesystem identity checks and operation-time validation.

| Finding | Disposition |
| --- | --- |
| #1 Parent registration | Retain fail-closed fallback. A current-directory subscription alone does not cover parent namespace changes. A native regression registers the current directory, renames its parent before the second registration and requires failure. The suggested mode-711 failure did not reproduce on the local Linux host; it is not the basis for the regression. |
| #2 Recursive totals | Retain on-demand cached count snapshots. Unchanged selected-entry snapshots preserve completed counts; explicit recount/refresh, relevant entry changes, navigation and errors invalidate them. No recursive watcher or automatic recount. |
| #3 Reconcile failure | Retry failed listings promptly with capped backoff. Keep stale rows, blocked operations and detail invalidation until an identity-verified listing succeeds. Generation and context checks still reject stale completions. |
| #4 Registration flapping | Preserve exponential backoff through register/error cycles. Require 30 seconds of stability before resetting backoff or reporting recovery. Repeated error hints cannot trigger extra scans or warning transitions. |
| #5 Parent via link | Capture the lexical entry's bracketed no-follow identity in the listing worker. Preserve it for the navigation context; same-target refresh cannot adopt a replacement link. Prefer suitable saved visits, and restore only visible matching identities. |
| #6 Degraded scans | After the initial catch-up, scan at intervals of 2, 4, 8, 16 and then 30 seconds, capped at 30 seconds. Listing failures retry at 1, 2, 4, 8, 16 and then 30 seconds. Watcher registration retries independently with the same one-to-30-second backoff. Deadlines defer behind active work/navigation. |
| #7 Readiness catch-up | Retain the post-registration catch-up to cover the listing/subscription gap. After watcher errors, catch-up follows stable recovery; fallback remains active during probation. |
| #8 Independent panes | Retain independent automatic Linux/macOS refresh for hidden panes, including two panes showing the same directory. No shared listing cache. |
| #9 Worker wake | Retain the 100 ms bounded worker wake. No measured evidence justifies a redesign. |
| #10 Windows dependency | Restrict `notify` and its implementation to Linux/macOS. Windows remains manual, with no watcher owners, timed retries or automatic scans. Cargo's all-target lock graph still resolves `notify`'s Windows backend, so retain the exact `windows-sys 0.60.2` exception and lockfile entries. Windows target builds exclude that edge. CC0/ISC permissions and notices remain necessary for Unix builds. |

The capped cadence trades degraded-mode freshness for fewer idle scans. Healthy
Unix watchers retain 200 ms debounce, one-second sustained-event throttling and
30-second reconciliation. The historical CPU figures in TODO.md describe
Windows polling; Linux/macOS degraded-mode CPU cost remains unmeasured.
