param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [Parameter(Mandatory=$true)][string]$FilePath,
    [ValidateSet('Open','Save')][string]$Action = 'Open'
)
# English Windows test-dialog automation, restricted to the recorded Folio process. Save uses the default filename in the isolated sample directory.
$ErrorActionPreference = 'Stop'
if ($Action -eq 'Open' -and !(Test-Path -LiteralPath $FilePath -PathType Leaf)) {
    throw "The test source does not exist: $FilePath"
}
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
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


    private static string ClassName(IntPtr hwnd) {
        var buffer = new StringBuilder(128); GetClassName(hwnd, buffer, buffer.Capacity); return buffer.ToString();
    }
    private static string Caption(IntPtr hwnd) {
        var buffer = new StringBuilder(512); GetWindowText(hwnd, buffer, buffer.Capacity); return buffer.ToString().Replace("&", "");
    }
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    public static bool Accept(int appPid, string action) {
        IntPtr dialog = Find(appPid, action);
        if (dialog == IntPtr.Zero) return false;
        IntPtr accept = IntPtr.Zero;
        EnumChildWindows(dialog, (child, unused) => {
            if (GetDlgCtrlID(child) == 1 && ClassName(child) == "Button" && Caption(child) == action) accept = child;
            return true;
        }, IntPtr.Zero);
        return accept != IntPtr.Zero && PostMessage(accept, 0x00F5, IntPtr.Zero, IntPtr.Zero);
    }
    public static IntPtr Find(int appPid, string action) {
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
        return target;


    }
}
"@
$deadline = [DateTime]::UtcNow.AddSeconds(45)
do {
    $dialogHandle = [FolioDialogAutomation]::Find($AppProcessId, $Action)
    if ($dialogHandle -ne [IntPtr]::Zero) {
        $dialog = [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $AppProcessId)) | Where-Object { $_.Current.NativeWindowHandle -eq $dialogHandle.ToInt32() } | Select-Object -First 1
        if (!$dialog) { Start-Sleep -Milliseconds 200; continue }
        $controls = $dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)

        $filename = $null

        foreach ($control in $controls) {
            $kind = $control.Current.ControlType.ProgrammaticName

            if ($kind -eq 'ControlType.Edit' -and $control.Current.Name -eq 'File name:') { $filename = $control }

        }

        if ($filename) {
            $value = $filename.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            if ($Action -eq 'Open') { $value.SetValue($FilePath) }
            if ($Action -eq 'Open' -and $value.Current.Value -ne $FilePath) { throw 'Native filename field did not retain the exact test path.' }
            if ($Action -eq 'Save' -and $value.Current.Value -notin @([IO.Path]::GetFileName($FilePath), [IO.Path]::GetFileNameWithoutExtension($FilePath))) { throw ('Unexpected default filename in isolated save test: ' + $value.Current.Value) }
            if (![FolioDialogAutomation]::Accept($AppProcessId, $Action)) { throw 'Owned dialog accept button unavailable.' }
            Write-Output "$Action dialog submitted through accessibility controls."
            exit 0
        }
    }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
throw "Could not submit the $Action file dialog for process $AppProcessId."