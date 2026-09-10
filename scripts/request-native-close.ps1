param([Parameter(Mandatory=$true)][int]$AppProcessId)
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class FolioCloseTest {
    public delegate bool Callback(IntPtr window,IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumThreadWindows(uint thread,Callback callback,IntPtr data);
    [DllImport("user32.dll",CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window,StringBuilder title,int count);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool PostMessage(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
    public static void Request(int pid) {
        IntPtr target=IntPtr.Zero;
        Callback inspect=(window,data)=>{
            var title=new StringBuilder(256);GetWindowText(window,title,title.Capacity);
            if(IsWindowVisible(window)&&title.ToString()=="Folio"){target=window;return false;}
            return true;
        };
        foreach(ProcessThread thread in Process.GetProcessById(pid).Threads){
            EnumThreadWindows((uint)thread.Id,inspect,IntPtr.Zero);
            if(target!=IntPtr.Zero)break;
        }
        if(target==IntPtr.Zero)throw new InvalidOperationException("The Folio test window is not available.");
        if(!PostMessage(target,0x0010,IntPtr.Zero,IntPtr.Zero))throw new InvalidOperationException("Could not request window close.");
    }
}
"@
[FolioCloseTest]::Request($AppProcessId)
