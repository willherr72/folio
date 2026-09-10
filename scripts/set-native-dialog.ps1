param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [Parameter(Mandatory=$true)][string]$FilePath,
    [ValidateSet('Open','Save')][string]$Action = 'Open'
)
# Windows common-dialog automation, restricted to the Folio process under test.
$ErrorActionPreference = 'Stop'
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
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr wparam, string lparam);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wparam, IntPtr lparam);
    private static string ClassName(IntPtr hwnd) {
        var buffer = new StringBuilder(128); GetClassName(hwnd, buffer, buffer.Capacity); return buffer.ToString();
    }
    private static string Caption(IntPtr hwnd) {
        var buffer = new StringBuilder(512); GetWindowText(hwnd, buffer, buffer.Capacity); return buffer.ToString().Replace("&", "");
    }
    public static bool FocusSave(int appPid) {
        IntPtr target = IntPtr.Zero;
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid == appPid && ClassName(window) == "#32770" && Caption(window) == "Save a PDF copy") target = window;
            return true;
        }, IntPtr.Zero);
        if (target == IntPtr.Zero) return false;
        SetForegroundWindow(target);
        return GetForegroundWindow() == target;
    }
    public static bool Submit(int appPid, string path, string action) {
        bool submitted = false;
        EnumWindows((window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid != appPid || ClassName(window) != "#32770") return true;
            IntPtr filename = IntPtr.Zero, accept = IntPtr.Zero;
            EnumChildWindows(window, (child, unused) => {
                int id = GetDlgCtrlID(child);
                if (id == 1148 && ClassName(child) == "Edit") filename = child;
                if (id == 1 && ClassName(child) == "Button" && Caption(child) == action) accept = child;
                return true;
            }, IntPtr.Zero);
            if (filename == IntPtr.Zero || accept == IntPtr.Zero) return true;
            if (SendMessage(filename, 0x000C, IntPtr.Zero, path) == IntPtr.Zero)
                throw new InvalidOperationException("The file dialog refused its filename.");
            submitted = PostMessage(accept, 0x00F5, IntPtr.Zero, IntPtr.Zero);
            return !submitted;
        }, IntPtr.Zero);
        return submitted;
    }
}
"@
$deadline = [DateTime]::UtcNow.AddSeconds(20)
do {
    if ($Action -eq 'Save' -and [FolioDialogAutomation]::FocusSave($AppProcessId)) {
        # The modern Save dialog ignores WM_SETTEXT on its hidden compatibility edit.
        # Use its real filename accelerator only after verifying the owned dialog has focus.
        Add-Type -AssemblyName System.Windows.Forms
        $escapedPath = -join ($FilePath.ToCharArray() | ForEach-Object {
            if ('+^%~(){}[]'.Contains([string]$_)) { '{' + $_ + '}' } else { [string]$_ }
        })
        [System.Windows.Forms.SendKeys]::SendWait('%n')
        [System.Windows.Forms.SendKeys]::SendWait('^a')
        if (![FolioDialogAutomation]::FocusSave($AppProcessId)) { throw 'Folio Save dialog lost focus.' }
        [System.Windows.Forms.SendKeys]::SendWait($escapedPath)
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
        Write-Output 'Save dialog submitted.'
        exit 0
    }
    if ($Action -eq 'Open' -and [FolioDialogAutomation]::Submit($AppProcessId, $FilePath, $Action)) {
        Write-Output "$Action dialog submitted."
        exit 0
    }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
throw "Could not find the $Action file dialog for process $AppProcessId."
