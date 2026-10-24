# Disposable Windows watcher experiment

This spike is based on `8a275fe18a3c02d1dff8d302bbf711814b82a8b0`.
It changes no production source, dependency graph or original regression.
Its branch-only workflow is temporary research infrastructure, not a new gate
for PR #4. The original native suite runs without changed expectations and
keeps the workflow red while ancestor rename fails.

Run on Windows with PowerShell 7:

```powershell
cargo make build
$notify = Get-ChildItem target/debug/deps/libnotify-*.rlib | Select-Object -First 1
$nativeArgs = @(Get-ChildItem target/debug/build/windows_x86_64_msvc-*/output | Get-Content | Where-Object { $_ -like 'cargo:rustc-link-search=native=*' } | ForEach-Object { '-L'; $_.Substring('cargo:rustc-link-search='.Length) })
rustc @nativeArgs --edition=2024 research/windows-watch/notify_probe.rs -L dependency=target/debug/deps --extern "notify=$($notify.FullName)" -o research/windows-watch/notify-probe.exe
./research/windows-watch/run.ps1 -NotifyHelper ./research/windows-watch/notify-probe.exe -Output probe.json
```

`run.ps1 -CompileOnly` compiles the managed code and checks ABI sizes, event
attribution and the sample cap; it supplies no Windows execution evidence.

The probe creates GUID-named temporary roots and launches separate PowerShell
writer processes. No ordinary writer operation calls `SHChangeNotify`.
The explicit `shell-control` case does call it, as a labelled delivery control.
Native mutation results retain exact Win32 status; managed writer failures
retain the low word of their HRESULT. Registrations remain active during
current/parent/grandparent renames, deletion chains and replacement attempts.
Failed mutations repeat after release to distinguish persistent watch handles.

Shell-only uses `ShellLevel | NewDelivery` (`0x8002`); the second Shell mode adds
`InterruptLevel` (`0x8003`). Both use one nonrecursive entry per registration,
current plus immediate parent, a hidden window on an STA owner, and locked
delivery. No direct directory handle belongs to either managed Shell watcher.
That fact alone does not establish rename safety: the native mutation results
measure the effect of Shell-held resources too.

The notify helper uses the existing exact dependency and no unsafe code. Its
callbacks retain counters only; it forwards coalesced hints at most every 10 ms.
Reported notify latency therefore includes that forwarding delay. The wider
root comparison is recursive only within the owned fixture, not a whole drive.

Native observation performs no periodic listing. A missed Shell-only create
also gets one labelled enumeration at 30 seconds. This is a probe sweep,
not execution of TFM's coordinator. The probe does not implement a production
debouncer, cache, listing pool or Windows backend.

Event samples stop at 512 per case; total/relevant/omitted counters continue.
Shared-memory lock/unlock failures and callback errors are recorded. Idle
measurement uses message waits, and lifecycle cases rebind 25 times before
checking process handles and deleting the owned fixture. C# code is original
managed P/Invoke research code; no GPL implementation was copied.

Primary references inspected:

- [Microsoft registration contract](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shchangenotifyregister)
- [Microsoft lock/unlock contract](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shchangenotification_lock)
- [Microsoft rename contract](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-fsa/87f86c9b-6c2a-4803-84b7-131a74a434fa)
- [Explorer++ Shell watcher at b6dc6d5](https://github.com/derceg/explorerplusplus/blob/b6dc6d5b193fd283b84ecfda1cf6935a49da6e8b/Explorer%2B%2B/Explorer%2B%2B/ShellWatcherManager.cpp)
- [Explorer++ folder handling](https://github.com/derceg/explorerplusplus/blob/b6dc6d5b193fd283b84ecfda1cf6935a49da6e8b/Explorer%2B%2B/Explorer%2B%2B/ShellBrowser/DirectoryModificationHandler.cpp)
- [Double Commander modes at 422a72c](https://github.com/doublecmd/doublecmd/blob/422a72cc499551e4ff759942d611ad6fbc82889f/src/platform/ufilesystemwatcher.pas)
- [Files limited Shell watcher at ee399b0](https://github.com/files-community/Files/blob/ee399b0d3e09084aa24d7261c9342d92bc89a5da/src/Files.Storage/Windows/Managers/WindowsFolderChangeWatcher.cs)

UNC/SMB, ReFS, removable media, volume GUID paths, deliberate Explorer shutdown
and production TFM Shell integration remain untested. Report actual host shell
presence instead of assuming a desktop Explorer process exists.

The follow-up workflow uses `-Isolation`. It compares retained PIDLs without a
registration (`SHParseDisplayName`, with/without path conversion, and
`ILCreateFromPathW`) against active Shell registrations whose callbacks only
lock/count/unlock and never resolve event paths. Mutation fixtures include
Unicode ancestor names. Raw-event variants deliberately perform no path
attribution; their counters cannot prove nonrecursive filtering. These controls
separate registration effects from PIDL creation and event-path conversion.
The original full matrix remains available without `-Isolation`.
