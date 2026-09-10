param([Parameter(Mandatory=$true)][int]$AppProcessId)
$ErrorActionPreference='Stop'
Add-Type -TypeDefinition @"
using System;
using System.Text;
using System.Diagnostics;
using System.Runtime.InteropServices;
public static class FolioPrintCancel {
 public delegate bool Callback(IntPtr window,IntPtr data);
 [DllImport("user32.dll")] static extern bool EnumThreadWindows(uint thread,Callback callback,IntPtr data);
 [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window,out uint pid);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr window,StringBuilder text,int count);
 [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
 [DllImport("user32.dll")] static extern bool PostMessage(IntPtr window,uint message,IntPtr wparam,IntPtr lparam);
 public static bool Cancel(int pid){
  IntPtr target=IntPtr.Zero;
  Callback inspect=(window,data)=>{
   uint owner;GetWindowThreadProcessId(window,out owner);
   if(owner==(uint)pid&&IsWindowVisible(window)){
    var text=new StringBuilder(512);GetWindowText(window,text,text.Capacity);
    if(text.ToString()=="Print"){target=window;return false;}
   }
   return true;
  };
  foreach(ProcessThread thread in Process.GetProcessById(pid).Threads){EnumThreadWindows((uint)thread.Id,inspect,IntPtr.Zero);if(target!=IntPtr.Zero)break;}
  return target!=IntPtr.Zero&&PostMessage(target,0x0111,new IntPtr(2),IntPtr.Zero);
 }
}
"@
$deadline=[DateTime]::UtcNow.AddSeconds(45)
while([DateTime]::UtcNow -lt $deadline){
 if([FolioPrintCancel]::Cancel($AppProcessId)){Write-Output 'Cancelled native Print dialog for the test process.';exit 0}
 Start-Sleep -Milliseconds 200
}
throw 'Native Print dialog was not found for the supplied test process.'
