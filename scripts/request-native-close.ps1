param([Parameter(Mandatory=$true)][int]$AppProcessId)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class FolioCloseTest {
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window, uint message, IntPtr wparam, IntPtr lparam);
    public static void Request(int pid) {
        var process = Process.GetProcessById(pid);
        var window = process.MainWindowHandle;
        if (window == IntPtr.Zero) throw new InvalidOperationException("The Folio test window is not available.");
        if (!PostMessage(window, 0x0010, IntPtr.Zero, IntPtr.Zero)) throw new InvalidOperationException("Could not request window close.");
    }
}
"@
[FolioCloseTest]::Request($AppProcessId)
