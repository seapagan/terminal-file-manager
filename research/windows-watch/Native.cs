using System;
using System.Runtime.InteropServices;
using System.Text;

public static class Native {
    public delegate IntPtr WindowProc(IntPtr window, uint message, UIntPtr wp, IntPtr lp);
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    public struct WindowClass {
        public uint style; public WindowProc proc; public int clsExtra, wndExtra;
        public IntPtr instance, icon, cursor, brush;
        public string menu, name;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct Message {
        public IntPtr window; public uint message; public UIntPtr wp; public IntPtr lp;
        public uint time; public int x, y; public uint extra;
    }
    [StructLayout(LayoutKind.Sequential)]
    public struct Entry { public IntPtr pidl; public int recursive; }
    [DllImport("ole32.dll")] public static extern int CoInitializeEx(IntPtr reserved, uint mode);
    [DllImport("ole32.dll")] public static extern void CoUninitialize();
    [DllImport("ole32.dll")] public static extern void CoTaskMemFree(IntPtr memory);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr GetModuleHandleW(string name);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern ushort RegisterClassW(ref WindowClass value);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool UnregisterClassW(string name, IntPtr instance);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateWindowExW(uint ex, string cls, string title, uint style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr param);
    [DllImport("user32.dll")] public static extern IntPtr DefWindowProcW(IntPtr window, uint message, UIntPtr wp, IntPtr lp);
    [DllImport("user32.dll")] public static extern bool PeekMessageW(out Message message, IntPtr window, uint min, uint max, uint remove);
    [DllImport("user32.dll")] public static extern IntPtr DispatchMessageW(ref Message message);
    [DllImport("user32.dll")] public static extern uint MsgWaitForMultipleObjectsEx(uint count, IntPtr handles, uint timeout, uint mask, uint flags);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool DestroyWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern IntPtr GetShellWindow();
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)] public static extern int SHParseDisplayName(string path, IntPtr context, out IntPtr pidl, uint attributes, out uint result);
    [DllImport("shell32.dll")] public static extern uint SHChangeNotifyRegister(IntPtr window, int sources, int events, uint message, int count, ref Entry entry);
    [DllImport("shell32.dll")] public static extern bool SHChangeNotifyDeregister(uint id);
    [DllImport("shell32.dll")] public static extern IntPtr SHChangeNotification_Lock(IntPtr change, uint process, out IntPtr pidls, out int changeEvent);
    [DllImport("shell32.dll")] public static extern bool SHChangeNotification_Unlock(IntPtr handle);
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)] public static extern bool SHGetPathFromIDListEx(IntPtr pidl, StringBuilder path, uint count, uint options);
    [DllImport("shell32.dll")] public static extern void SHChangeNotify(int changeEvent, uint flags, IntPtr first, IntPtr second);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateFileW(string path, uint access, uint sharing, IntPtr security, uint creation, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError=true)] public static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool MoveFileExW(string source, string destination, uint flags);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] public static extern bool RemoveDirectoryW(string path);

    public static void Pump(int milliseconds) {
        var end = Environment.TickCount64 + milliseconds;
        do {
            Message message;
            int dispatched = 0;
            while (dispatched++ < 256 && PeekMessageW(out message, IntPtr.Zero, 0, 0, 1))
                DispatchMessageW(ref message);
            int remaining = (int)Math.Max(0, end - Environment.TickCount64);
            if (remaining == 0) break;
            MsgWaitForMultipleObjectsEx(0, IntPtr.Zero, (uint)remaining, 0x04ff, 4);
        } while (Environment.TickCount64 < end);
    }
    public static string PidlPath(IntPtr pidl) {
        if (pidl == IntPtr.Zero) return null;
        var text = new StringBuilder(32768);
        return SHGetPathFromIDListEx(pidl, text, 32768, 0) ? text.ToString() : null;
    }
}
