param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [Parameter(Mandatory=$true)][string]$FilePath,
    [ValidateSet('Open','Save')][string]$Action = 'Open',
    [ValidateSet('Open a PDF','Import a font','Save a PDF copy')][string]$Title = 'Open a PDF'
)
# Release-test dialog automation restricted to one owned process and exact dialog title.
# Every accepted filename is read back from the exact native filename control.
if ($Action -eq "Save" -and (Test-Path -LiteralPath $FilePath)) { throw "Release tests never overwrite an existing output." }
$ErrorActionPreference = 'Stop'
if ($Action -eq 'Open' -and !(Test-Path -LiteralPath $FilePath -PathType Leaf)) {
    throw "The test source does not exist: $FilePath"
}
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


    private static string ClassName(IntPtr hwnd) {
        var buffer = new StringBuilder(128); GetClassName(hwnd, buffer, buffer.Capacity); return buffer.ToString();
    }
    private static string Caption(IntPtr hwnd) {
        var buffer = new StringBuilder(512); GetWindowText(hwnd, buffer, buffer.Capacity); return buffer.ToString().Replace("&", "");
    }
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr hwnd, uint message, IntPtr wparam, IntPtr lparam);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")] private static extern IntPtr SendText(IntPtr hwnd,uint message,IntPtr wparam,string text,uint flags,uint timeout,out IntPtr result);
    [DllImport("user32.dll", CharSet=CharSet.Unicode, EntryPoint="SendMessageTimeoutW")] private static extern IntPtr ReadText(IntPtr hwnd,uint message,IntPtr wparam,StringBuilder text,uint flags,uint timeout,out IntPtr result);
    [DllImport("user32.dll", EntryPoint="SendMessageTimeoutW")] private static extern IntPtr SendScalar(IntPtr hwnd,uint message,IntPtr wparam,IntPtr lparam,uint flags,uint timeout,out IntPtr result);
    public static bool SetFilename(IntPtr dialog, string path, string action) {
        IntPtr edit=IntPtr.Zero;
        EnumChildWindows(dialog,(child,unused)=>{if(GetDlgCtrlID(child)==(action=="Save"?1001:1148) && ClassName(child)=="Edit")edit=child;return true;},IntPtr.Zero);
        if(edit==IntPtr.Zero)return false;
        IntPtr result;
        if(SendScalar(edit,0x00B1,IntPtr.Zero,(IntPtr)(-1),2,1000,out result)==IntPtr.Zero)return false;
        if(SendText(edit,0x00C2,(IntPtr)1,path,2,1000,out result)==IntPtr.Zero)return false;
        var actual=new StringBuilder(32768);
        return ReadText(edit,0x000D,(IntPtr)actual.Capacity,actual,2,1000,out result)!=IntPtr.Zero && actual.ToString()==path;
    }
    public static bool Accept(int appPid, string action, string title) {
        IntPtr dialog = Find(appPid, action, title);
        if (dialog == IntPtr.Zero) return false;
        IntPtr accept = IntPtr.Zero;
        EnumChildWindows(dialog, (child, unused) => {
            if (GetDlgCtrlID(child) == 1 && ClassName(child) == "Button" && Caption(child) == action) accept = child;
            return true;
        }, IntPtr.Zero);
        return accept != IntPtr.Zero && (action=="Save" ? PostMessage(accept, 0x00F5, IntPtr.Zero, IntPtr.Zero) : PostMessage(dialog, 0x0111, (IntPtr)1, accept));
    }
    public static IntPtr Find(int appPid, string action, string title) {
        IntPtr target = IntPtr.Zero;

        EnumProc inspect = (window, data) => {
            uint pid; GetWindowThreadProcessId(window, out pid);
            if (pid != appPid || ClassName(window) != "#32770" || Caption(window) != title) return true;
            bool hasAccept = false;
            EnumChildWindows(window, (child, unused) => {
                if (GetDlgCtrlID(child) == 1 && ClassName(child) == "Button" && Caption(child) == action) hasAccept = true;
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
$deadline = [DateTime]::UtcNow.AddSeconds(15)
do {
    $dialogHandle = [FolioDialogAutomation]::Find($AppProcessId, $Action, $Title)
    if ($dialogHandle -ne [IntPtr]::Zero) {
        # Hidden Shell dialogs expose their native Edit as a UIA Pane without ValuePattern.
        # Address only the verified dialog's filename control; read it back before acceptance.
        if ([FolioDialogAutomation]::SetFilename($dialogHandle, $FilePath, $Action)) {
            if (![FolioDialogAutomation]::Accept($AppProcessId, $Action, $Title)) { throw 'Owned dialog accept button unavailable.' }
            Write-Output "$Action dialog submitted through its scoped native filename control."
            exit 0
        }
    }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
throw "Could not submit the $Action file dialog for process $AppProcessId."
