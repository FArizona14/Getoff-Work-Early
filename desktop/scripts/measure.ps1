param([int]$Seconds = 10)
$ErrorActionPreference = 'Stop'
$rootProcess = Get-Process | Where-Object { $_.ProcessName -eq 'early-off-work' -or $_.ProcessName -eq '早点下班-0.1.0-alpha.1-x64' } | Select-Object -First 1
if (!$rootProcess) { throw '请先运行桌面小样' }
$processList = Get-CimInstance Win32_Process
$processIds = [System.Collections.Generic.HashSet[int]]::new()
[void]$processIds.Add($rootProcess.Id)
do {
  $added = $false
  foreach ($item in $processList) {
    if ($processIds.Contains([int]$item.ParentProcessId) -and $processIds.Add([int]$item.ProcessId)) { $added = $true }
  }
} while ($added)
function Read-Sample {
  $p = Get-Process -Id @($processIds) -ErrorAction SilentlyContinue
  [pscustomobject]@{ cpu = ($p | Measure-Object CPU -Sum).Sum; working = ($p | Measure-Object WorkingSet64 -Sum).Sum; private = ($p | Measure-Object PrivateMemorySize64 -Sum).Sum; count = $p.Count }
}
$a = Read-Sample
$watch = [System.Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $Seconds
$b = Read-Sample
$watch.Stop()
[pscustomobject]@{
  seconds = [math]::Round($watch.Elapsed.TotalSeconds,2)
  processCount = $b.count
  cpuPercentWholeMachine = [math]::Round(100 * ($b.cpu - $a.cpu) / $watch.Elapsed.TotalSeconds / [Environment]::ProcessorCount,3)
  workingSetMiB = [math]::Round($b.working / 1MB,2)
  privateMiB = [math]::Round($b.private / 1MB,2)
  note = '包含主进程及采样开始时的 WebView2 子进程；短时观察，不代表长期性能'
} | ConvertTo-Json
