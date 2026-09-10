param([Parameter(Mandatory=$true)][int]$AppProcessId)
$ErrorActionPreference = 'Stop'
$rootProcess = Get-Process -Id $AppProcessId -ErrorAction Stop
$all = @(Get-CimInstance Win32_Process)
$ids = [Collections.Generic.HashSet[int]]::new()
[void]$ids.Add($AppProcessId)
do {
    $added = $false
    foreach ($item in $all) {
        if ($ids.Contains([int]$item.ParentProcessId) -and $ids.Add([int]$item.ProcessId)) { $added = $true }
    }
} while ($added)
$processes = @()
foreach ($processId in $ids) {
    $item = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if ($item) {
        $processes += [pscustomobject][ordered]@{id=$item.Id;name=$item.ProcessName;workingSetBytes=$item.WorkingSet64;privateBytes=$item.PrivateMemorySize64;processLifetimePeakWorkingSetBytes=$item.PeakWorkingSet64}
    }
}
[ordered]@{
    utc=[DateTime]::UtcNow.ToString('o')
    appProcessId=$AppProcessId
    currentTreeWorkingSetBytes=($processes | Measure-Object -Property workingSetBytes -Sum).Sum
    currentTreePrivateBytes=($processes | Measure-Object -Property privateBytes -Sum).Sum
    processes=$processes
    note='Snapshot of native app and descendant WebView processes. Lifetime process peaks are not a simultaneous tree peak. Shared pages can be counted in multiple working sets.'
} | ConvertTo-Json -Depth 5
