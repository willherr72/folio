param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [Parameter(Mandatory=$true)][string]$FilePath,
    [ValidateSet('Open','Save')][string]$Action = 'Open',
    [string]$DialogTitle = '',
    [switch]$NativeFileName
)
# English Windows test-dialog automation, restricted to the recorded Folio process. Save uses the default filename in the isolated sample directory.
$ErrorActionPreference = 'Stop'
if (!$DialogTitle) { $DialogTitle = if ($Action -eq 'Open') { 'Open a PDF' } else { 'Save a PDF copy' } }
if (($Action -eq 'Open' -and $DialogTitle -notin @('Open a PDF','Place an image','Import a font')) -or ($Action -eq 'Save' -and $DialogTitle -ne 'Save a PDF copy')) { throw 'Unexpected acceptance-test dialog title.' }
if ($Action -eq 'Open' -and !(Test-Path -LiteralPath $FilePath -PathType Leaf)) {
    throw "The test source does not exist: $FilePath"
}
if ($Action -eq 'Open') { $FilePath = (Resolve-Path -LiteralPath $FilePath).Path }
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class FolioDialogAutomation {
    private delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumThreadWindows(uint threadId, EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] private static extern int GetDlgCtrlID(IntPtr hwnd);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint message, IntPtr wparam, string text, uint flags, uint timeout, out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint message, IntPtr wparam, StringBuilder text, uint flags, uint timeout, out IntPtr result);

    private static IntPtr FileNameEdit(int appPid, string title) {
        IntPtr dialog = Find(appPid, title), edit = IntPtr.Zero;
        int matches = 0;
        if (dialog == IntPtr.Zero) return IntPtr.Zero;
        EnumChildWindows(dialog, (child, unused) => {
            if (GetDlgCtrlID(child) == 1148 && ClassName(child) == "Edit") { edit = child; matches++; }
            return true;
        }, IntPtr.Zero);
        return matches == 1 ? edit : IntPtr.Zero;
    }
    public static string ReadFileName(int appPid, string title) {
        IntPtr edit = FileNameEdit(appPid, title);
        if (edit == IntPtr.Zero) return null;
        var text = new StringBuilder(32768);
        IntPtr result;
        // Cross-process edit text requires WM_GETTEXT, not GetWindowText:
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowtexta
        if (SendMessageTimeout(edit, 0x000D, new IntPtr(text.Capacity), text, 2, 2000, out result) == IntPtr.Zero) return null;
        return text.ToString();
    }
    public static bool SetFileName(int appPid, string title, string path) {
        IntPtr edit = FileNameEdit(appPid, title), result;
        return edit != IntPtr.Zero && SendMessageTimeout(edit, 0x000C, IntPtr.Zero, path, 2, 2000, out result) != IntPtr.Zero
            && ReadFileName(appPid, title) == path;
    }


    private static string ClassName(IntPtr hwnd) {
        var buffer = new StringBuilder(128); GetClassName(hwnd, buffer, buffer.Capacity); return buffer.ToString();
    }
    private static string Caption(IntPtr hwnd) {
        var buffer = new StringBuilder(512); GetWindowText(hwnd, buffer, buffer.Capacity); return buffer.ToString().Replace("&", "");
    }
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    public static bool Accept(int appPid, string title) {
        IntPtr dialog = Find(appPid, title);
        if (dialog == IntPtr.Zero) return false;
        IntPtr accept = IntPtr.Zero;
        EnumChildWindows(dialog, (child, unused) => {
            if (GetDlgCtrlID(child) == 1 && ClassName(child) == "Button") accept = child;
            return true;
        }, IntPtr.Zero);
        return accept != IntPtr.Zero && PostMessage(accept, 0x00F5, IntPtr.Zero, IntPtr.Zero);
    }
    public static IntPtr Find(int appPid, string title) {
        IntPtr target = IntPtr.Zero;
        EnumProc inspect = (window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid != appPid || ClassName(window) != "#32770" || Caption(window) != title) return true;
            bool hasAccept = false;
            EnumChildWindows(window, (child, unused) => {
                if (GetDlgCtrlID(child) == 1 && ClassName(child) == "Button") hasAccept = true;
                return true;
            }, IntPtr.Zero);
            if (hasAccept) target = window;
            return true;
        };
        EnumWindows(inspect, IntPtr.Zero);
        if (target == IntPtr.Zero) {
            using (var process = Process.GetProcessById(appPid))
                foreach (ProcessThread thread in process.Threads)
                    EnumThreadWindows((uint)thread.Id, inspect, IntPtr.Zero);
        }
        return target;


    }
}
"@
$deadline = [DateTime]::UtcNow.AddSeconds(45)
do {
    $dialogHandle = [FolioDialogAutomation]::Find($AppProcessId, $DialogTitle)
    if ($dialogHandle -ne [IntPtr]::Zero) {
        $dialog = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $AppProcessId)) | Where-Object { $_.Current.NativeWindowHandle -eq $dialogHandle.ToInt32() } | Select-Object -First 1
        if (!$dialog) { Start-Sleep -Milliseconds 200; continue }
        $controls = $dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)

        $filename = $null

        foreach ($control in $controls) {
            $kind = $control.Current.ControlType.ProgrammaticName

            if ($kind -eq 'ControlType.Edit' -and $control.Current.Name -eq 'File name:') { $filename = $control }

        }

        if ($filename -and !$NativeFileName) {
            # Current Windows exposes the Open split button as a UIA pane and its
            # Win32 caption may be empty. Verify the action through accessibility
            # before clicking control ID 1 in the PID/title-scoped native dialog.
            $acceptControl = $controls | Where-Object { $_.Current.AutomationId -eq '1' -and $_.Current.Name -eq $Action -and $_.Current.ClassName -eq 'Button' } | Select-Object -First 1
            if (!$acceptControl) { Start-Sleep -Milliseconds 100; continue }
            $value = $filename.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            if ($Action -eq 'Open') { $value.SetValue($FilePath) }
            # Shell initialization can replace the filename just after ValuePattern.SetValue.
            # Retry within the bounded dialog deadline; never accept a different path.
            if ($Action -eq 'Open' -and $value.Current.Value -ne $FilePath) { Start-Sleep -Milliseconds 100; continue }
            if ($Action -eq 'Save' -and $value.Current.Value -notin @([IO.Path]::GetFileName($FilePath), [IO.Path]::GetFileNameWithoutExtension($FilePath))) { throw ('Unexpected default filename in isolated save test: ' + $value.Current.Value) }
            if (![FolioDialogAutomation]::Accept($AppProcessId, $DialogTitle)) { throw 'Owned dialog accept button unavailable.' }
            Write-Output "$Action dialog submitted through accessibility controls."
            exit 0
        }
        # Some WebView/Windows sessions expose the native filename Edit as a
        # generic UIA Pane with no ValuePattern. Restrict fallback to the unique
        # standard filename control within this already PID/title-scoped dialog.
        $acceptControl = $controls | Where-Object { $_.Current.AutomationId -eq '1' -and $_.Current.Name -eq $Action -and $_.Current.ClassName -eq 'Button' } | Select-Object -First 1
        $nativeName = [FolioDialogAutomation]::ReadFileName($AppProcessId, $DialogTitle)
        if ($acceptControl -and $null -ne $nativeName) {
            if ($Action -eq 'Open' -and ![FolioDialogAutomation]::SetFileName($AppProcessId, $DialogTitle, $FilePath)) { Start-Sleep -Milliseconds 100; continue }
            if ($Action -eq 'Save' -and $nativeName -notin @([IO.Path]::GetFileName($FilePath), [IO.Path]::GetFileNameWithoutExtension($FilePath))) { throw ('Unexpected default filename in isolated save test: ' + $nativeName) }
            if (![FolioDialogAutomation]::Accept($AppProcessId, $DialogTitle)) { throw 'Owned dialog accept button unavailable.' }
            Write-Output "$Action dialog submitted through scoped native filename control."
            exit 0
        }
    }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
throw "Could not submit the $Action file dialog for process $AppProcessId."
