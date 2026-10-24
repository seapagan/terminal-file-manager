using System;
using System.IO;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading;

public sealed class OperationResult {
    public string operation, target;
    public bool success;
    public int error;
    public string error_origin;
    public long started, ended;
    public int writes;
    public bool already_absent;
}

public static class WatchProbe {
    static readonly JsonSerializerOptions Json = new JsonSerializerOptions { IncludeFields=true, WriteIndented=true };
    static readonly List<object> Cases = new List<object>();
    static string script, helper, output, root;
    static object host;
    static bool isolationOnly;
    static string Current(string fixture) { return Path.Combine(fixture, "grand", "parent", "current"); }
    static string MakeFixture(string label, bool empty = false) {
        string fixture = Path.Combine(root, label + "-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Current(fixture));
        File.WriteAllText(Path.Combine(fixture, ".tfm-probe-owned"), "TFM isolated watch research");
        if (!empty) {
            File.WriteAllText(Path.Combine(Current(fixture), "file.bin"), "initial");
            Directory.CreateDirectory(Path.Combine(Current(fixture), "nested"));
        }
        return fixture;
    }
    static void Save() {
        File.WriteAllText(output, JsonSerializer.Serialize(new { host, cases = Cases.ToArray() }, Json));
    }
    static void Add(object value) { Cases.Add(value); Save(); }
    static ProbeWatch Open(string mode, string fixture, string current, Capture capture) {
        switch(mode) {
            case "none": return new EmptyWatch();
            case "pidl-only": case "pidl-resolve": case "pidl-il": return new PidlWatch(current, mode=="pidl-il", mode=="pidl-resolve");
            case "shell-raw": case "shell-il": case "shell-only-raw": return new ShellWatch(current,mode!="shell-only-raw",capture,true,mode=="shell-il");
            case "handle-child": case "handle-parent": case "handle-both": return new HandleWatch(current, mode);
            case "notify-pair": case "notify-root": return new NotifyWatch(helper, fixture, current, mode == "notify-root", capture);
            case "shell-only": case "shell-interrupt": return new ShellWatch(current, mode == "shell-interrupt", capture);
            default: throw new ArgumentException(mode);
        }
    }
    static int Handles() { using(var process = Process.GetCurrentProcess()) return process.HandleCount; }
    static bool AwaitHint(Capture capture, long started) {
        long deadline = Environment.TickCount64 + 3000;
        while(capture.FirstAfter(started)==0 && Environment.TickCount64<deadline) Native.Pump(20);
        return capture.FirstAfter(started)!=0;
    }
    static object Ready(string mode, string fixture, Capture capture) {
        if(mode=="none" || mode.StartsWith("handle-") || mode.StartsWith("pidl-")) return new { native_handle_control=true };
        Native.Pump(300); capture.Reset();
        var pulse=Writer(fixture,"warmup");
        bool external=AwaitHint(capture,pulse[0].started);
        bool shellControl=false;
        if(mode.StartsWith("shell-only") && !external) {
            capture.Reset(); var control=Writer(fixture,"warmup-control");
            shellControl=AwaitHint(capture,control[0].started);
        }
        Writer(fixture,"warmup-clean"); Native.Pump(600); capture.Reset();
        return new { external_delivery_before_mutation=external, explicit_shell_control_before_mutation=shellControl };
    }
    static OperationResult[] Writer(string fixture, string operation, bool pump = true) {
        var start = new ProcessStartInfo(Environment.ProcessPath) {
            RedirectStandardOutput=true, RedirectStandardError=true, UseShellExecute=false
        };
        foreach(var arg in new[] {"-NoProfile", "-File", script, "-Worker", "-Fixture", fixture, "-Operation", operation})
            start.ArgumentList.Add(arg);
        using(var process = Process.Start(start)) {
            var stdout = process.StandardOutput.ReadToEndAsync();
            var stderr = process.StandardError.ReadToEndAsync();
            var limit = Environment.TickCount64 + 20000;
            while(!process.HasExited && Environment.TickCount64 < limit) {
                if(pump) Native.Pump(10); else Thread.Sleep(10);
            }
            if (!process.HasExited) { process.Kill(true); throw new TimeoutException("writer process exceeded 20 seconds"); }
            if(process.ExitCode != 0) throw new InvalidOperationException("writer failed: " + stderr.GetAwaiter().GetResult());
            return JsonSerializer.Deserialize<OperationResult[]>(stdout.GetAwaiter().GetResult(), Json);
        }
    }
    static void RenameCase(string mode, string operation) {
        string fixture = MakeFixture(isolationOnly ? mode+"-雪-🙂-e\u0301" : mode, operation == "delete-chain");
        string current = Current(fixture);
        var capture = new Capture(current);
        var clock = Stopwatch.StartNew();
        ProbeWatch watch = null;
        try {
            watch = Open(mode, fixture, current, capture);
            double startup = clock.Elapsed.TotalMilliseconds;
            object readiness=Ready(mode,fixture,capture);
            object active = watch.State();
            var results = Writer(fixture, operation);
            Native.Pump(600);
            object hints = capture.Summary();
            clock.Restart(); watch.Dispose(); object released = watch.State(); watch = null;
            double teardown = clock.Elapsed.TotalMilliseconds;
            bool retryAfterRelease = false;
            OperationResult[] retry = null;
            if (Array.Exists(results, result => !result.success)) {
                retry = Writer(fixture, operation == "delete-chain" ? "delete-remaining" : operation);
                retryAfterRelease = Array.TrueForAll(retry, result => result.success);
            }
            Directory.Delete(fixture, true);
            Add(new { kind="mutation", mode, operation, active, startup_ms=startup, teardown_ms=teardown,
                results, readiness, hints, released, retry_after_release=retryAfterRelease, retry, fixture_removed=!Directory.Exists(fixture) });
            if(mode == "none" && Array.Exists(results, result => !result.success))
                throw new InvalidOperationException("no-watcher baseline mutation failed");
        } catch(Exception exception) {
            if(watch != null) watch.Dispose();
            Add(new {kind="mutation-error", mode, operation, error=exception.ToString()});
            if(Directory.Exists(fixture)) Directory.Delete(fixture, true);
            if(mode == "none") throw;
        }
    }
    static void EventCase(string mode, string operation) {
        string fixture = MakeFixture(mode);
        string current = Current(fixture);
        var capture = new Capture(current);
        ProbeWatch watch = null;
        try {
            watch = Open(mode, fixture, current, capture);
            Native.Pump(600); capture.Reset();
            var results = Writer(fixture, operation);
            long start = results[0].started;
            Native.Pump(operation == "burst" ? 2500 : 1800);
            long first = capture.FirstAfter(start);
            bool direct = first != 0;
            object active = watch.State();
            object hints = capture.Summary();
            int files = Directory.GetFiles(current).Length;
            bool sweepDetected = false;
            double sweepMs = 0;
            long lateHint = 0;
            if(mode == "shell-only" && operation == "create" && !direct) {
                double elapsed = (Stopwatch.GetTimestamp() - start)*1000.0/Stopwatch.Frequency;
                Native.Pump((int)Math.Max(0, 30000 - elapsed));
                lateHint = capture.FirstAfter(start);
                sweepDetected = File.Exists(Path.Combine(current, "created.txt"));
                sweepMs = (Stopwatch.GetTimestamp() - start)*1000.0/Stopwatch.Frequency;
            }
            watch.Dispose(); object released=watch.State(); watch = null;
            Directory.Delete(fixture, true);
            Add(new { kind="event", mode, operation, results, active, direct_hint=direct,
                first_hint_latency_upper_bound_ms=direct ? (double?)(first-start)*1000.0/Stopwatch.Frequency : null,
                hints, released, files, observed_at_30s_probe_sweep=sweepDetected, sweep_ms=sweepMs,
                native_hint_before_30s_sweep=lateHint!=0,
                native_hint_before_sweep_latency_ms=lateHint!=0 ? (double?)(lateHint-start)*1000.0/Stopwatch.Frequency : null,
                no_periodic_enumeration_during_native_observation=true, fixture_removed=!Directory.Exists(fixture) });
        } catch(Exception exception) {
            if(watch != null) watch.Dispose();
            Add(new {kind="event-error", mode, operation, error=exception.ToString()});
            if(Directory.Exists(fixture)) Directory.Delete(fixture, true);
        }
    }
    static void Lifecycle(string mode) {
        string fixture = MakeFixture(mode);
        var capture = new Capture(Current(fixture));
        var cycle = new List<object>();
        string other = Path.Combine(fixture,"grand","parent","other"); Directory.CreateDirectory(other);
        int initial = Handles();
        for(int i=0; i<25; i++) {
            var clock = Stopwatch.StartNew();
            using(var watch = Open(mode, fixture, i%2==0 ? Current(fixture) : other, capture))
                cycle.Add(new { iteration=i, startup_ms=clock.Elapsed.TotalMilliseconds, active=watch.State(), handles=Handles() });
            Native.Pump(10);
        }
        GC.Collect(); GC.WaitForPendingFinalizers(); Native.Pump(200);
        int before = Handles();
        using(var watch = Open(mode, fixture, Current(fixture), capture)) {
            Native.Pump(600); capture.Reset();
            using(var process = Process.GetCurrentProcess()) {
                process.Refresh(); long memory = process.WorkingSet64;
                var cpu = process.TotalProcessorTime;
                object activeBefore = watch.State();
                var clock = Stopwatch.StartNew(); Native.Pump(3000); process.Refresh();
                Add(new {kind="idle", mode, idle_ms=clock.Elapsed.TotalMilliseconds,
                    cpu_ms=(process.TotalProcessorTime-cpu).TotalMilliseconds,
                    rss_before=memory, rss_after=process.WorkingSet64, handles_before=before,
                    handles_active=process.HandleCount, threads_active=process.Threads.Count,
                    events=capture.Summary(), active_before=activeBefore, active=watch.State()});
            }
        }
        GC.Collect(); GC.WaitForPendingFinalizers(); Native.Pump(200);
        Directory.Delete(fixture, true);
        Add(new {kind="lifecycle", mode, initial_handles=initial, cycles=cycle.ToArray(), handles_after=Handles(),
            fixture_removed=!Directory.Exists(fixture), deregistration_error_count=0});
    }
    static void Special(string operation, string watched) {
        string fixture = MakeFixture("special");
        string current = Current(fixture);
        if(watched == "long") {
            current = LongPath(fixture);
            Directory.CreateDirectory(current);
        } else if(watched == "junction") {
            current = Path.Combine(fixture,"alias");
            var start = new ProcessStartInfo("cmd.exe") { UseShellExecute=false, RedirectStandardOutput=true, RedirectStandardError=true };
            foreach(var arg in new[] {"/c", "mklink", "/J", current, Current(fixture)}) start.ArgumentList.Add(arg);
            using(var process=Process.Start(start)) {
                var text=process.StandardOutput.ReadToEndAsync(); var errors=process.StandardError.ReadToEndAsync();
                if(!process.WaitForExit(5000)) { process.Kill(true); throw new TimeoutException("junction setup timed out"); }
                if(process.ExitCode != 0) throw new InvalidOperationException("junction setup failed: "+text.GetAwaiter().GetResult()+errors.GetAwaiter().GetResult());
            }
        }
        var capture = new Capture(current);
        try {
            using(var watch = Open("shell-interrupt", fixture, current, capture)) {
                Native.Pump(600); capture.Reset();
                var result = Writer(fixture, operation); Native.Pump(1800);
                Add(new {kind="special-path", operation, registered_path=current, result,
                    hints=capture.Summary(), direct_hint=capture.FirstAfter(result[0].started)!=0});
            }
        } catch(Exception exception) { Add(new {kind="special-path-error", operation, error=exception.ToString()}); }
        if(watched == "junction") Directory.Delete(current);
        Directory.Delete(fixture,true);
    }
    static void GapAndRebind(string mode) {
        string fixture=MakeFixture("rebind");
        string current=Current(fixture), other=Path.Combine(Path.GetDirectoryName(current),"other");
        Directory.CreateDirectory(other);
        Writer(fixture,"create");
        var oldCapture=new Capture(current); var nextCapture=new Capture(other);
        ProbeWatch old=null, next=null;
        try {
            old=Open(mode,fixture,current,oldCapture); Native.Pump(800);
            object startupHints=oldCapture.Summary();
            bool catchup=File.Exists(Path.Combine(current,"created.txt"));
            oldCapture.Reset(); Writer(fixture,"modify",false);
            object active=old.State(); old.Dispose(); old=null;
            next=Open(mode,fixture,other,nextCapture); Native.Pump(600); nextCapture.Reset();
            Writer(fixture,"modify"); Native.Pump(1200);
            object oldPathAfterRebind=nextCapture.Summary(); nextCapture.Reset();
            var write=Writer(fixture,"other-create"); bool newDelivery=AwaitHint(nextCapture,write[0].started);
            Add(new {kind="registration-gap-rebind",mode, existed_before_registration=true,
                post_registration_enumeration_found_file=catchup, startupHints,
                old_registration_active_before_release=active, oldPathAfterRebind,
                new_directory_delivered=newDelivery, new_directory_hints=nextCapture.Summary()});
        } finally {
            if(old!=null) old.Dispose(); if(next!=null) next.Dispose();
            Directory.Delete(fixture,true);
        }
    }
    public static string LongPath(string fixture) {
        return @"\\?\" + Path.Combine(Current(fixture),new string('a',100),new string('b',100),new string('c',100));
    }
    public static void Run(string writerScript, string notifyHelper, string outputFile, bool isolation=false) {
        isolationOnly=isolation;
        script=Path.GetFullPath(writerScript); helper=Path.GetFullPath(notifyHelper); output=Path.GetFullPath(outputFile);
        Exception failure=null;
        var thread=new Thread(() => { try { Execute(); } catch(Exception error) { failure=error; } }) { IsBackground=true };
        thread.SetApartmentState(ApartmentState.STA); thread.Start();
        if(!thread.Join(TimeSpan.FromMinutes(12))) throw new TimeoutException("probe exceeded 12 minutes");
        if(failure != null) throw new InvalidOperationException("probe failed",failure);
    }
    static void Execute() {
        root=Path.Combine(Path.GetTempPath(),"tfm-watch-probe-"+Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(root);
        int hr=Native.CoInitializeEx(IntPtr.Zero,2);
        if(hr<0) Marshal.ThrowExceptionForHR(hr);
        host=new { os=Environment.OSVersion.ToString(), runtime=Environment.Version.ToString(),
            architecture=RuntimeInformation.ProcessArchitecture.ToString(), frequency=Stopwatch.Frequency,
            explorer_processes=Process.GetProcessesByName("explorer").Length,
            shell_window_present=Native.GetShellWindow()!=IntPtr.Zero,
            interactive=Environment.UserInteractive, session=Process.GetCurrentProcess().SessionId,
            filesystem=new DriveInfo(Path.GetPathRoot(root)).DriveFormat,
            shell_only_sources="0x8002 (ShellLevel + NewDelivery)",
            shell_interrupt_sources="0x8003 (ShellLevel + InterruptLevel + NewDelivery)",
            untested=new[]{"UNC/SMB", "ReFS", "removable media", "volume GUID paths", "Explorer manually stopped", "production TFM Shell adapter"}};
        Save();
        try {
            if(isolationOnly) {
                foreach(var mode in new[]{"none","pidl-only","pidl-resolve","pidl-il","shell-only-raw","shell-raw","shell-il"})
                    foreach(var operation in new[]{"rename-current","rename-parent","rename-grandparent"})
                        RenameCase(mode,operation);
                foreach(var mode in new[]{"shell-only-raw","shell-raw","shell-il"})
                    foreach(var operation in new[]{"create","modify","attributes","read"}) EventCase(mode,operation);
                return;
            }
            foreach(var mode in new[]{"none","handle-child","handle-parent","handle-both","notify-pair","shell-only","shell-interrupt","notify-root"})
                foreach(var operation in new[]{"rename-current","rename-parent","rename-grandparent","delete-chain","replace-current","replace-parent","replace-grandparent"})
                    RenameCase(mode,operation);
            foreach(var mode in new[]{"notify-pair","shell-only","shell-interrupt","notify-root"}) {
                foreach(var operation in new[]{"create","delete","rename-file","modify","attributes","read","sibling","descendant","unicode","trailing-dot","unpaired","sustained","burst","shell-control"})
                    EventCase(mode,operation);
                try { Lifecycle(mode); }
                catch(Exception error) { Add(new {kind="lifecycle-error",mode,error=error.ToString()}); }
                if(mode.StartsWith("shell-")) {
                    try { GapAndRebind(mode); }
                    catch(Exception error) { Add(new {kind="rebind-error",mode,error=error.ToString()}); }
                }
            }
            Lifecycle("none");
            Special("long-create","long"); Special("create","junction");
        } finally {
            Native.CoUninitialize();
            Directory.Delete(root,true);
            Add(new {kind="final-cleanup", owned_root_removed=!Directory.Exists(root), root,
                explorer_processes_after=Process.GetProcessesByName("explorer").Length,
                shell_window_after=Native.GetShellWindow()!=IntPtr.Zero});
        }
    }
    static OperationResult NativeResult(string operation,string target,Func<bool> action) {
        var result=new OperationResult {operation=operation,target=target,started=Stopwatch.GetTimestamp(),error_origin="Win32 GetLastError"};
        result.success=action(); result.error=result.success?0:Marshal.GetLastWin32Error();
        result.ended=Stopwatch.GetTimestamp(); return result;
    }
    public static string Write(string fixture,string operation) {
        fixture=Path.GetFullPath(fixture);
        if(!fixture.StartsWith(Path.Combine(Path.GetTempPath(),"tfm-watch-probe-"),StringComparison.OrdinalIgnoreCase)
            || File.ReadAllText(Path.Combine(fixture,".tfm-probe-owned"))!="TFM isolated watch research")
            throw new InvalidOperationException("writer rejected unowned fixture");
        string current=Current(fixture);
        var results=new List<OperationResult>();
        if(operation.StartsWith("rename-") && operation!="rename-file" || operation.StartsWith("replace-")) {
            string level=operation.Substring(operation.IndexOf('-')+1);
            string target=level=="current"?current:level=="parent"?Path.GetDirectoryName(current):Path.GetDirectoryName(Path.GetDirectoryName(current));
            var result=NativeResult(operation,target,()=>Native.MoveFileExW(target,target+"-moved",0)); results.Add(result);
            if(result.success && operation.StartsWith("replace-")) { Directory.CreateDirectory(current); File.WriteAllText(Path.Combine(current,"replacement"),"new"); }
        } else if(operation=="delete-chain" || operation=="delete-remaining") {
            for(int i=0;i<3;i++) {
                string target=current;
                if(operation=="delete-remaining" && !Directory.Exists(target))
                    results.Add(new OperationResult {operation=operation,target=target,success=true,already_absent=true});
                else results.Add(NativeResult(operation,target,()=>Native.RemoveDirectoryW(target)));
                current=Path.GetDirectoryName(current);
            }
        } else {
            var result=new OperationResult {operation=operation,started=Stopwatch.GetTimestamp()};
            try { result.writes=WriteEvent(fixture,operation); result.success=true; }
            catch(Exception error) { result.error=error.HResult&0xffff; result.error_origin="managed HRESULT low word"; result.success=false; }
            result.ended=Stopwatch.GetTimestamp(); results.Add(result);
        }
        return JsonSerializer.Serialize(results.ToArray(),Json);
    }
    static int WriteEvent(string fixture,string operation) {
        string current=Current(fixture), file=Path.Combine(current,"file.bin");
        switch(operation) {
            case "warmup": File.WriteAllText(Path.Combine(current,".tfm-ready"),"ready"); break;
            case "warmup-control":
                IntPtr ready=Marshal.StringToCoTaskMemUni(Path.Combine(current,".tfm-ready"));
                try {Native.SHChangeNotify(2,0x3005,ready,IntPtr.Zero);} finally {Marshal.FreeCoTaskMem(ready);} break;
            case "warmup-clean": File.Delete(Path.Combine(current,".tfm-ready")); break;
            case "create": File.WriteAllText(Path.Combine(current,"created.txt"),"created"); break;
            case "other-create": File.WriteAllText(Path.Combine(Path.GetDirectoryName(current),"other","new.txt"),"new"); break;
            case "delete": File.Delete(file); break;
            case "rename-file": File.Move(file,file+"-renamed"); break;
            case "modify": using(var stream=new FileStream(file,FileMode.Open,FileAccess.Write,FileShare.ReadWrite|FileShare.Delete)) {stream.Write(new byte[8192]);stream.Flush(true);} break;
            case "attributes": File.SetAttributes(file,FileAttributes.Hidden); break;
            case "read": for(int i=0;i<100;i++) File.ReadAllBytes(file); break;
            case "sibling": File.WriteAllText(Path.Combine(Path.GetDirectoryName(current),"unrelated.txt"),"sibling"); break;
            case "descendant": File.WriteAllText(Path.Combine(current,"nested","child.txt"),"descendant"); break;
            case "unicode": File.WriteAllText(Path.Combine(current,"雪-🙂-e\u0301.txt"),"unicode"); break;
            case "trailing-dot": File.WriteAllText(@"\\?\"+Path.Combine(current,"trailing."),"dot"); break;
            case "unpaired": File.WriteAllText(Path.Combine(current,"unpaired-\ud800.txt"),"surrogate"); break;
            case "long-create": File.WriteAllText(Path.Combine(LongPath(fixture),"created.txt"),"long"); break;
            case "sustained": for(int i=0;i<300;i++) {File.WriteAllBytes(file,new byte[100+i]);Thread.Sleep(5);} return 300;
            case "burst": for(int i=0;i<2000;i++) File.WriteAllText(Path.Combine(current,"burst"+i.ToString("D4")),"data"); return 2000;
            case "shell-control":
                string created=Path.Combine(current,"controlled.txt"); File.WriteAllText(created,"control");
                IntPtr text=Marshal.StringToCoTaskMemUni(created);
                try {Native.SHChangeNotify(2,0x3005,text,IntPtr.Zero);} finally {Marshal.FreeCoTaskMem(text);} break;
            default: throw new ArgumentException(operation);
        }
        return 1;
    }
    public static void SelfCheck() {
        if(Marshal.SizeOf<Native.Entry>()!=(IntPtr.Size==8?16:8)) throw new Exception("entry ABI mismatch");
        if(Marshal.SizeOf<Native.Message>()!=(IntPtr.Size==8?48:32)) throw new Exception("message ABI mismatch");
        var capture=new Capture(@"C:\owned\current");
        capture.Add(2,@"C:\owned\current\file",null);
        capture.Add(2,@"C:\owned\sibling",null);
        if(capture.RelevantCount()!=1) throw new Exception("attribution failed");
        for(int i=0;i<1000;i++) capture.Add(0,null,null);
        if(capture.sample.Count!=512 || capture.omitted!=490) throw new Exception("capture was not bounded");
    }
}
