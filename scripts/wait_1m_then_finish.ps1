# wait_1m_then_finish.ps1
# Cursor-independent: poll until TARGET (500k) overnight GPU job completes, then run finish_overnight_1m_to_paper.py.
# Does NOT kill training. Poll interval 15-30 min (jittered).

$ErrorActionPreference = "Continue"
$Repo = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Repo "scripts\finish_overnight_1m_to_paper.py"))) {
  $Repo = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
}
$Artifacts = Join-Path $Repo "brand\artifacts"
$Latest = Join-Path $Artifacts "overnight_gpu_rl_arch_latest.json"
$DoneFlag = Join-Path $Artifacts "overnight_gpu_DONE.flag"
$LogFile = Join-Path $Artifacts "wait_1m_finisher.log"
$PidFile = Join-Path $Artifacts "wait_1m_finisher.pid"
$Finisher = Join-Path $Repo "scripts\finish_overnight_1m_to_paper.py"
$Py = Join-Path $Repo ".venv_gpu\Scripts\python.exe"
$TargetIters = 500000
$HighIterFloor = 450000  # if training gone after this, treat as finished enough to attempt finisher

function UtcNow { (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ") }
function Log([string]$Message) {
  $line = "$(UtcNow) WAIT1M $Message"
  Add-Content -Path $LogFile -Value $line -Encoding UTF8
  Write-Output $line
}

function Get-LatestIter {
  if (-not (Test-Path $Latest)) { return 0 }
  try {
    $j = Get-Content $Latest -Raw -Encoding UTF8 | ConvertFrom-Json
    return [int]($j.iter)
  } catch { return 0 }
}

function Get-HistoryMaxIter {
  try {
    if (-not (Test-Path $Latest)) { return 0 }
    $j = Get-Content $Latest -Raw -Encoding UTF8 | ConvertFrom-Json
    $runDir = [string]$j.run_dir
    if (-not $runDir) { return 0 }
    $hist = Join-Path $runDir "history.jsonl"
    if (-not (Test-Path $hist)) { return 0 }
    $last = Get-Content $hist -Tail 1 -ErrorAction SilentlyContinue
    if (-not $last) { return 0 }
    $row = $last | ConvertFrom-Json
    if ($null -ne $row.iter) { return [int]$row.iter }
    if ($null -ne $row.iteration) { return [int]$row.iteration }
    return 0
  } catch { return 0 }
}

function Test-TrainingAlive {
  $procs = @(Get-CimInstance Win32_Process -Filter "Name='python.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -and ($_.CommandLine -match 'overnight_gpu_rl_arch\.py') })
  return ($procs.Count -gt 0)
}

# Record our PID
$myPid = $PID
Set-Content -Path $PidFile -Value "$myPid" -Encoding ASCII
Log "started pid=$myPid target=$TargetIters poll=15-30min finisher=$Finisher"

$finished = $false
while (-not $finished) {
  $iterLatest = Get-LatestIter
  $iterHist = Get-HistoryMaxIter
  $iter = [Math]::Max($iterLatest, $iterHist)
  $alive = Test-TrainingAlive
  $doneExists = Test-Path $DoneFlag

  Log "poll iter_latest=$iterLatest iter_hist=$iterHist max=$iter training_alive=$alive done_flag=$doneExists"

  if ($doneExists) {
    Log "DONE flag present — proceeding to finisher"
    $finished = $true
    break
  }
  if ($iter -ge $TargetIters) {
    Log "iter>=$TargetIters — proceeding to finisher"
    $finished = $true
    break
  }
  if (-not $alive -and $iter -ge $HighIterFloor) {
    Log "training gone after high iter ($iter >= $HighIterFloor) — proceeding to finisher"
    $finished = $true
    break
  }
  if (-not $alive -and $iter -lt $HighIterFloor) {
    Log "WARN training not alive but iter=$iter below floor; still waiting (babysit may restart)"
  }

  $sleepSec = Get-Random -Minimum 900 -Maximum 1801  # 15-30 min
  Log "sleep ${sleepSec}s"
  Start-Sleep -Seconds $sleepSec
}

# Re-check iter once more for finisher gate
$iterLatest = Get-LatestIter
$iterHist = Get-HistoryMaxIter
$iter = [Math]::Max($iterLatest, $iterHist)
Log "invoking finisher iter=$iter py=$Py"

if (-not (Test-Path $Py)) {
  Log "ERROR missing python $Py"
  exit 1
}
if (-not (Test-Path $Finisher)) {
  Log "ERROR missing finisher $Finisher"
  exit 1
}

$finLog = Join-Path $Artifacts "wait_1m_finisher_run.log"
try {
  $p = Start-Process -FilePath $Py -ArgumentList @($Finisher) -WorkingDirectory $Repo `
    -RedirectStandardOutput $finLog -RedirectStandardError (Join-Path $Artifacts "wait_1m_finisher_run.err.log") `
    -NoNewWindow -Wait -PassThru
  Log "finisher exit_code=$($p.ExitCode) stdout_log=$finLog"
  if ($p.ExitCode -ne 0) {
    # Finisher returns 2 if not done yet — retry a few times if race with last writes
    for ($r = 1; $r -le 6; $r++) {
      Log "finisher retry $r/6 after 120s (exit=$($p.ExitCode))"
      Start-Sleep -Seconds 120
      $p = Start-Process -FilePath $Py -ArgumentList @($Finisher) -WorkingDirectory $Repo `
        -RedirectStandardOutput $finLog -RedirectStandardError (Join-Path $Artifacts "wait_1m_finisher_run.err.log") `
        -NoNewWindow -Wait -PassThru
      Log "finisher retry exit_code=$($p.ExitCode)"
      if ($p.ExitCode -eq 0) { break }
      $iter = [Math]::Max((Get-LatestIter), (Get-HistoryMaxIter))
      if ($iter -lt $TargetIters -and -not (Test-Path $DoneFlag)) {
        Log "still not at target iter=$iter — back to wait loop briefly"
      }
    }
  }
} catch {
  Log "ERROR finisher exception: $_"
  exit 1
}

Log "complete"
exit 0
