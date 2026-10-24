using System;
using System.IO;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
using System.Threading.Tasks;

public sealed class Hint {
    public long tick; public int flags; public string first, second; public bool relevant;
}

public sealed class Capture {
    public readonly List<Hint> sample = new List<Hint>();
    public long total, relevant, omitted, updatedir, lockFailures, unlockFailures, callbackErrors;
    public double maxCallbackMs;
    public long firstRelevantTick, lastRelevantTick;
    public readonly string current;
    public Capture(string path) { current = Normalize(path); }
    static string Normalize(string path) {
        if (path == null) return null;
        return path.StartsWith(@"\\?\", StringComparison.Ordinal) ? path.Substring(4) : path;
    }
    bool Matches(string path) {
        path = Normalize(path);
        if (path == null) return false;
        int separator = path.LastIndexOf('\\');
        string parent = separator < 0 ? null : path.Substring(0, separator);
        return String.Equals(path, current, StringComparison.OrdinalIgnoreCase)
            || String.Equals(parent, current, StringComparison.OrdinalIgnoreCase)
            || current.StartsWith(path.TrimEnd('\\') + "\\", StringComparison.OrdinalIgnoreCase);
    }
    public void Add(int flags, string first, string second, bool force = false) {
        var hint = new Hint { tick = Stopwatch.GetTimestamp(), flags = flags, first = first, second = second,
            relevant = force || Matches(first) || Matches(second) || (first == null && second == null) };
        lock (sample) {
            total++; if (hint.relevant) {
                relevant++; if (firstRelevantTick == 0) firstRelevantTick = hint.tick;
                lastRelevantTick = hint.tick;
            }
            if ((flags & 0x1000) != 0) updatedir++;
            if (sample.Count < 512) sample.Add(hint); else omitted++;
        }
    }
    public long RelevantCount() { lock(sample) return relevant; }
    public long FirstAfter(long tick) {
        lock (sample) {
            if (firstRelevantTick >= tick) return firstRelevantTick;
            foreach (var hint in sample) if (hint.relevant && hint.tick >= tick) return hint.tick;
            return lastRelevantTick >= tick ? lastRelevantTick : 0;
        }
    }
    public void Reset() {
        lock(sample) { sample.Clear(); total = relevant = omitted = updatedir = firstRelevantTick = lastRelevantTick = 0; }
    }
    public object Summary() {
        lock(sample) return new { total, relevant, omitted, updatedir, lockFailures, unlockFailures, callbackErrors,
            maxCallbackMs, firstRelevantTick, lastRelevantTick, sample = sample.ToArray(), sample_capacity = 512 };
    }
}

public interface ProbeWatch : IDisposable { object State(); }
public sealed class PidlWatch : ProbeWatch {
    readonly List<IntPtr> pidls = new List<IntPtr>();
    readonly List<string> resolvedPaths = new List<string>();
    public int freed;
    public PidlWatch(string current, bool il, bool resolve) {
        try {
            foreach(string path in new[]{current, Path.GetDirectoryName(current)}) {
                IntPtr pidl;
                if(il) {
                    pidl=Native.ILCreateFromPathW(path);
                    if(pidl==IntPtr.Zero) throw new InvalidOperationException("ILCreateFromPathW returned null");
                } else {
                    uint attributes; int hr=Native.SHParseDisplayName(path,IntPtr.Zero,out pidl,0,out attributes);
                    if(hr<0) Marshal.ThrowExceptionForHR(hr);
                }
                pidls.Add(pidl);
                if(resolve) {
                    string converted=Native.PidlPath(pidl);
                    resolvedPaths.Add(converted);
                    if(converted==null) throw new InvalidOperationException("PIDL path conversion failed in resolution control");
                }
            }
        } catch { Dispose(); throw; }
    }
    public object State() { return new { retained_pidls=pidls.Count, freed, registrations=0, resolved_paths=resolvedPaths.ToArray() }; }
    public void Dispose() {
        foreach(var pidl in pidls) { Native.CoTaskMemFree(pidl); freed++; }
        pidls.Clear();
    }
}
public sealed class EmptyWatch : ProbeWatch {
    bool disposed;
    public object State() { return new { active = !disposed, registrations = 0 }; }
    public void Dispose() { disposed = true; }
}

public sealed class HandleWatch : ProbeWatch {
    readonly List<IntPtr> handles = new List<IntPtr>();
    public int closed;
    public HandleWatch(string current, string mode) {
        try {
            if (mode != "handle-parent") Open(current);
            if (mode != "handle-child") Open(Path.GetDirectoryName(current));
        } catch { Dispose(); throw; }
    }
    void Open(string path) {
        var handle = Native.CreateFileW(path, 1, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero);
        if (handle == new IntPtr(-1)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        handles.Add(handle);
    }
    public object State() { return new { active_handles = handles.Count, closed }; }
    public void Dispose() {
        foreach (var handle in handles) {
            if (!Native.CloseHandle(handle)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            closed++;
        }
        handles.Clear();
    }
}

public sealed class ShellWatch : ProbeWatch {
    readonly Capture capture;
    readonly bool raw, il;
    readonly string className = "TFMProbe" + Guid.NewGuid().ToString("N");
    readonly List<uint> ids = new List<uint>();
    readonly List<IntPtr> pidls = new List<IntPtr>();
    readonly Native.WindowProc callback;
    IntPtr window;
    ushort atom;
    public int deregistered, freed;
    public bool destroyed;
    public ShellWatch(string current, bool interrupts, Capture target, bool rawEvents=false, bool ilPaths=false) {
        capture = target; callback = OnMessage; raw=rawEvents; il=ilPaths;
        try {
            var cls = new Native.WindowClass { proc = callback, name = className, instance = Native.GetModuleHandleW(null) };
            atom = Native.RegisterClassW(ref cls);
            if (atom == 0) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            window = Native.CreateWindowExW(0, className, "", 0x08000000, 0, 0, 0, 0, IntPtr.Zero, IntPtr.Zero, cls.instance, IntPtr.Zero);
            if (window == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            Register(current, interrupts, 0x8001);
            Register(Path.GetDirectoryName(current), interrupts, 0x8002);
        } catch { Dispose(); throw; }
    }
    void Register(string path, bool interrupts, uint message) {
        IntPtr pidl; uint attributes;
        if(il) {
            pidl=Native.ILCreateFromPathW(path);
            if(pidl==IntPtr.Zero) throw new InvalidOperationException("ILCreateFromPathW returned null");
        } else {
            int hr = Native.SHParseDisplayName(path, IntPtr.Zero, out pidl, 0, out attributes);
            if (hr < 0) Marshal.ThrowExceptionForHR(hr);
        }
        pidls.Add(pidl);
        var entry = new Native.Entry { pidl = pidl, recursive = 0 };
        uint id = Native.SHChangeNotifyRegister(window, 0x8002 | (interrupts ? 1 : 0), 0x0002381f, message, 1, ref entry);
        if (id == 0) throw new InvalidOperationException("SHChangeNotifyRegister returned 0 (API supplies no native error code)");
        ids.Add(id);
    }
    IntPtr OnMessage(IntPtr hwnd, uint message, UIntPtr wp, IntPtr lp) {
        if (message == 0x8001 || message == 0x8002) Receive(wp, lp);
        return Native.DefWindowProcW(hwnd, message, wp, lp);
    }
    void Receive(UIntPtr wp, IntPtr lp) {
        var started = Stopwatch.GetTimestamp(); IntPtr held = IntPtr.Zero;
        try {
            IntPtr items; int flags;
            held = Native.SHChangeNotification_Lock(new IntPtr(unchecked((long)wp.ToUInt64())), unchecked((uint)lp.ToInt64()), out items, out flags);
            if (held == IntPtr.Zero) { capture.lockFailures++; return; }
            if(raw) { capture.Add(flags,null,null,true); return; }
            string first = items == IntPtr.Zero ? null : Native.PidlPath(Marshal.ReadIntPtr(items));
            string second = items == IntPtr.Zero ? null : Native.PidlPath(Marshal.ReadIntPtr(items, IntPtr.Size));
            capture.Add(flags, first, second);
        } catch { capture.callbackErrors++; }
        finally {
            if (held != IntPtr.Zero && !Native.SHChangeNotification_Unlock(held)) capture.unlockFailures++;
            capture.maxCallbackMs = Math.Max(capture.maxCallbackMs, (Stopwatch.GetTimestamp() - started) * 1000.0 / Stopwatch.Frequency);
        }
    }
    public object State() { return new { active_registrations = ids.Count, deregistered, freed, destroyed }; }
    public void Dispose() {
        foreach (uint id in ids) {
            if (!Native.SHChangeNotifyDeregister(id)) throw new InvalidOperationException("deregister failed");
            deregistered++;
        }
        ids.Clear();
        if (window != IntPtr.Zero) {
            if (!Native.DestroyWindow(window)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            destroyed = true; window = IntPtr.Zero;
        }
        foreach (var pidl in pidls) { Native.CoTaskMemFree(pidl); freed++; }
        pidls.Clear();
        if (atom != 0) {
            if (!Native.UnregisterClassW(className, Native.GetModuleHandleW(null))) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
            atom = 0;
        }
        GC.KeepAlive(callback);
    }
}

public sealed class NotifyWatch : ProbeWatch {
    readonly Process process;
    readonly Task reader;
    readonly ManualResetEventSlim ready = new ManualResetEventSlim(false);
    bool registered;
    long rawEvents, relevantEvents;
    public bool cleanExit;
    bool disposed;
    public int lastHandles;
    public NotifyWatch(string executable, string root, string current, bool recursive, Capture capture) {
        var start = new ProcessStartInfo(executable) { RedirectStandardInput=true, RedirectStandardOutput=true, RedirectStandardError=true, UseShellExecute=false };
        foreach (var arg in new[] {root, current, recursive ? "root" : "pair"}) start.ArgumentList.Add(arg);
        process = Process.Start(start);
        try {
            reader = Task.Run(async () => {
                string line;
                while ((line = await process.StandardOutput.ReadLineAsync()) != null) {
                    if (line == "READY") { registered = true; ready.Set(); }
                    else if (line.StartsWith("HINT")) {
                        var fields = line.Split(' ');
                        Interlocked.Add(ref relevantEvents, long.Parse(fields[1]));
                        Interlocked.Exchange(ref rawEvents, long.Parse(fields[2]));
                        capture.Add(0, null, null, true);
                    } else if (line.StartsWith("STATS")) {
                        var fields = line.Split(' ');
                        Interlocked.Exchange(ref rawEvents, long.Parse(fields[1]));
                        Interlocked.Exchange(ref relevantEvents, long.Parse(fields[2]));
                    }
                }
                ready.Set();
            });
            if (!ready.Wait(5000) || !registered) throw new InvalidOperationException("notify owner did not become ready");
        } catch {
            if (!process.HasExited) { process.StandardInput.WriteLine("quit"); process.StandardInput.Flush(); if (!process.WaitForExit(5000)) process.Kill(true); }
            if (reader != null) reader.Wait(5000);
            process.Dispose(); ready.Dispose(); disposed = true; throw;
        }
    }
    public object State() {
        if (disposed) return new { active = false, cleanExit, native_events=rawEvents, relevant_native_events=relevantEvents };
        if (!process.HasExited) { process.Refresh(); lastHandles = process.HandleCount; }
        return new { active = !process.HasExited, helper_handles = lastHandles, cleanExit,
            helper_cpu_ms = process.TotalProcessorTime.TotalMilliseconds, helper_rss = process.WorkingSet64,
            native_events = Interlocked.Read(ref rawEvents), relevant_native_events = Interlocked.Read(ref relevantEvents) };
    }
    public void Dispose() {
        if (disposed) return;
        if (!process.HasExited) {
            process.StandardInput.WriteLine("quit"); process.StandardInput.Flush();
            if (!process.WaitForExit(5000)) { process.Kill(true); throw new InvalidOperationException("notify owner shutdown timed out"); }
        }
        cleanExit = process.ExitCode == 0;
        if (!cleanExit) throw new InvalidOperationException("notify owner failed: " + process.StandardError.ReadToEnd());
        if (!reader.Wait(5000)) throw new TimeoutException("notify stdout reader did not exit");
        process.Dispose(); ready.Dispose(); disposed = true;
    }
}
