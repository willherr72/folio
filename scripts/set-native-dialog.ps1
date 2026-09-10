param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [Parameter(Mandatory=$true)][string]$FilePath,
    [ValidateSet('Open','Save')][string]$Action = 'Open'
)
# English Windows common-dialog automation, restricted to the Folio test process.
$ErrorActionPreference = 'Stop'
if ($Action -eq 'Open' -and !(Test-Path -LiteralPath $FilePath -PathType Leaf)) {
    throw "The test source does not exist: $FilePath"
}
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class FolioDialogAutomation {
    private delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern int GetDlgCtrlID(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] private static extern IntPtr GetForegroundWindow();
    private static string ClassName(IntPtr hwnd) {
        var buffer = new StringBuilder(128); GetClassName(hwnd, buffer, buffer.Capacity); return buffer.ToString();
    }
    private static string Caption(IntPtr hwnd) {
        var buffer = new StringBuilder(512); GetWindowText(hwnd, buffer, buffer.Capacity); return buffer.ToString().Replace("&", "");
    }
    public static bool Focus(int appPid, string action) {
        IntPtr target = IntPtr.Zero;
        string title = action == "Open" ? "Open a PDF" : "Save a PDF copy";
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid != appPid || ClassName(window) != "#32770" || Caption(window) != title) return true;
            bool hasAccept = false;
            EnumChildWindows(window, (child, unused) => {
                if (GetDlgCtrlID(child) == 1 && ClassName(child) == "Button" && Caption(child) == action) hasAccept = true;
                return true;
            }, IntPtr.Zero);
            if (hasAccept) target = window;
            return true;
        }, IntPtr.Zero);
        if (target == IntPtr.Zero) return false;
        SetForegroundWindow(target);
        return GetForegroundWindow() == target;
    }
}
"@
$escapedPath = -join ($FilePath.ToCharArray() | ForEach-Object {
    if ('+^%~(){}[]'.Contains([string]$_)) { '{' + $_ + '}' } else { [string]$_ }
})
$deadline = [DateTime]::UtcNow.AddSeconds(20)
do {
    if ([FolioDialogAutomation]::Focus($AppProcessId, $Action)) {
        # WM_SETTEXT can reach a hidden compatibility edit without updating the
        # actual dialog. Use its filename accelerator after verifying ownership/focus.
        [System.Windows.Forms.SendKeys]::SendWait('%n')
        [System.Windows.Forms.SendKeys]::SendWait('^a')
        if (![FolioDialogAutomation]::Focus($AppProcessId, $Action)) { throw 'Folio file dialog lost focus.' }
        [System.Windows.Forms.SendKeys]::SendWait($escapedPath)
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        Write-Output "$Action dialog submitted."
        exit 0
    }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
throw "Could not focus the $Action file dialog for process $AppProcessId."
