param(
    [Parameter(Mandatory=$true)][int]$AppProcessId,
    [Parameter(Mandatory=$true)][string]$FilePath,
    [ValidateSet('Open','Save')][string]$Action = 'Open'
)
# Automation helper for Folio's native file-dialog smoke tests.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$condition = [Windows.Automation.PropertyCondition]::new(
    [Windows.Automation.AutomationElement]::ProcessIdProperty, $AppProcessId)
$deadline = [DateTime]::UtcNow.AddSeconds(20)
do {
    $windows = [Windows.Automation.AutomationElement]::RootElement.FindAll(
        [Windows.Automation.TreeScope]::Children, $condition)
    foreach ($window in $windows) {
        $edits = $window.FindAll([Windows.Automation.TreeScope]::Descendants,
            [Windows.Automation.PropertyCondition]::new(
                [Windows.Automation.AutomationElement]::ControlTypeProperty,
                [Windows.Automation.ControlType]::Edit))
        foreach ($edit in $edits) {
            if ($edit.Current.Name -match '^File name') {
                $pattern = $edit.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern)
                $pattern.SetValue($FilePath)
                $buttons = $window.FindAll([Windows.Automation.TreeScope]::Descendants,
                    [Windows.Automation.PropertyCondition]::new(
                        [Windows.Automation.AutomationElement]::ControlTypeProperty,
                        [Windows.Automation.ControlType]::Button))
                foreach ($button in $buttons) {
                    if ($button.Current.Name.Replace('&','') -eq $Action) {
                        $invoke = $button.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern)
                        $invoke.Invoke()
                        Write-Output "$Action dialog submitted."
                        exit 0
                    }
                }
            }
        }
    }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
throw "Could not find the $Action file dialog for process $AppProcessId."
