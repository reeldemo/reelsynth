# overnight_gpu_babysit_1m.ps1
# Cursor-independent babysit for dense 500k overnight GPU RL.
# NEVER kills python. Treats .venv_gpu launcher + system-python worker as one job.
# Completes ONLY when iter >= 500_000 (or explicit DONE after that). Soft wall = 240h.
# Does NOT write overnight_gpu_DONE.flag on soft deadline (would stop the watchdog mid-run).

$ErrorActionPreference = "Continue"
$Repo = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Repo "scripts\overnight_gpu_rl_arch.py"))) {
  $Repo = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
}
$Artifacts = Join-Path $Repo "brand\artifacts"
$Watch = Join-Path $Artifacts "overnight_gpu_watchdog.log"
$Latest = Join-Path $Artifacts "overnight_gpu_rl_arch_latest.json"
$DoneFlag = Join-Path $Artifacts "overnight_gpu_DONE.flag"
$PidFile = Join-Path $Artifacts "overnight_gpu_rl_arch.pid"
$SummaryOut = Join-Path $Artifacts "overnight_gpu_final_summary.json"
$Py = Join-Path $Repo ".venv_gpu\Scripts\python.exe"
$Script = Join-Path $Repo "scripts\overnight_gpu_rl_arch.py"
$HeartbeatSec = 1800
$TargetIters = 500000
$Deadline = (Get-Date).AddHours(240)
$script:LastRestart = [datetime]::MinValue
# PPO+GA+PBT+depth+MoE; seed 1902771841; max-hours 240; MUST match live training (not complex_arch / 1M)
$SeedFitted = Join-Path $Artifacts "models\gpu-rl-arch-20260719T065704Z\fitted\champion_iter_001964_fitted.json"
$script:LastArgs = @($Script, "--iters", "500000", "--device", "cuda", "--max-hours", "240", "--history-every", "1", "--seed", "1902771841", "--pop-size", "12", "--algo-tag", "PPO+GA+PBT+NAS+depth+MoE", "--plateau-adapt-every", "1000", "--seed-fitted", $SeedFitted)
$script:KnownLauncher = 0
$script:KnownWorker = 0

# Seed known PIDs from pidfile / live scan
if (Test-Path $PidFile) {
  $raw = (Get-Content $PidFile -Raw -ErrorAction SilentlyContinue).Trim()
  $n = 0
  if ([int]::TryParse($raw, [ref]$n) -and $n -gt 0) { $script:KnownWorker = $n }
}

function UtcNow { (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ") }
function Log([string]$Message) {
  $line = "$(UtcNow) BABYSIT1M $Message"
  Add-Content -Path $Watch -Value $line -Encoding UTF8
  Write-Output $line
}

function Get-OvernightProcs {
  return @(Get-CimInstance Win32_Process -Filter "Name='python.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -and ($_.CommandLine -match 'overnight_gpu_rl_arch\.py') })
}

function Get-JobPair {
  $jobs = Get-OvernightProcs
  if ($jobs.Count -eq 0) {
    return @{ alive = $false; launcherPid = $null; workerPid = $null; pids = @(); cmd = $null }
  }
  $launcher = $jobs | Where-Object { $_.CommandLine -match '\\.venv_gpu\\' } | Select-Object -First 1
  $worker = $jobs | Where-Object { $_.CommandLine -notmatch '\\.venv_gpu\\' } |
    Sort-Object WorkingSetSize -Descending | Select-Object -First 1
  if (-not $worker -and $launcher) { $worker = $launcher }
  if (-not $launcher -and $worker) { $launcher = $worker }
  $cmd = $null
  if ($launcher) { $cmd = $launcher.CommandLine }
  elseif ($worker) { $cmd = $worker.CommandLine }
  $lp = $null; $wp = $null
  if ($launcher) { $lp = [int]$launcher.ProcessId }
  if ($worker) { $wp = [int]$worker.ProcessId }
  return @{
    alive = $true
    launcherPid = $lp
    workerPid = $wp
    pids = @($jobs | ForEach-Object { [int]$_.ProcessId })
    cmd = $cmd
  }
}

function Test-JobAliveRobust {
  for ($i = 0; $i -lt 3; $i++) {
    $pair = Get-JobPair
    if ($pair.alive) { return $pair }
    $kl = $null; $kw = $null
    if ($script:KnownLauncher -gt 0) {
      $kl = Get-Process -Id $script:KnownLauncher -ErrorAction SilentlyContinue
    }
    if ($script:KnownWorker -gt 0) {
      $kw = Get-Process -Id $script:KnownWorker -ErrorAction SilentlyContinue
    }
    if ($kl -or $kw) {
      return @{
        alive = $true
        launcherPid = $(if ($kl) { $script:KnownLauncher } else { $null })
        workerPid = $(if ($kw) { $script:KnownWorker } else { $null })
        pids = @($(if ($kl) { $script:KnownLauncher }), $(if ($kw) { $script:KnownWorker })) | Where-Object { $_ }
        cmd = $null
      }
    }
    Start-Sleep -Seconds 5
  }
  return @{ alive = $false; launcherPid = $null; workerPid = $null; pids = @(); cmd = $null }
}

function Read-Latest {
  if (-not (Test-Path $Latest)) { return $null }
  try { return (Get-Content $Latest -Raw | ConvertFrom-Json) } catch { return $null }
}

function Is-OneMillionDone {
  param($j)
  if ($j -and ([int]$j.iter -ge $TargetIters)) { return $true }
  $d = Get-ChildItem (Join-Path $Artifacts "models") -Directory -Filter "gpu-rl-arch-*" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if ($d -and (Test-Path (Join-Path $d.FullName "final_summary.json"))) {
    try {
      $fs = Get-Content (Join-Path $d.FullName "final_summary.json") -Raw | ConvertFrom-Json
      $done = 0
      if ($fs.iters_done) { $done = [int]$fs.iters_done }
      elseif ($fs.iter) { $done = [int]$fs.iter }
      if ($done -ge $TargetIters) { return $true }
    } catch {}
  }
  if (Test-Path $DoneFlag) {
    if ($j -and ([int]$j.iter -ge $TargetIters)) { return $true }
  }
  return $false
}

function Write-FinalSummary([string]$reason) {
  $j = Read-Latest
  $pair = Get-JobPair
  $baseline = $null
  $champ = $null
  if ($j) {
    $baseline = [double]$j.baseline_dual_cosine
    $champ = [double]$j.champion_residual
  }
  $summary = [ordered]@{
    written_at = (UtcNow)
    reason = $reason
    target_iters = $TargetIters
    current_iters = $(if ($j) { $j.iter } else { $null })
    current_run_dir = $(if ($j) { $j.run_dir } else { $null })
    current_pid_launcher = $pair.launcherPid
    current_pid_worker = $pair.workerPid
    champion_residual = $champ
    dual_cosine_baseline = $baseline
    delta_vs_dual_cosine = $(if (($null -ne $champ) -and ($null -ne $baseline)) { $champ - $baseline } else { $null })
    fitted_dir = $(if ($j) { $j.fitted_dir } else { $null })
    unfitted_dir = $(if ($j) { $j.unfitted_dir } else { $null })
    wall_deadline = $Deadline.ToUniversalTime().ToString("o")
    note = "500k babysit; launcher+child one job; never kill venv child as duplicate"
  }
  $json = $summary | ConvertTo-Json -Depth 6
  Set-Content -Path $SummaryOut -Value $json -Encoding utf8
  Log "FINAL_SUMMARY wrote $SummaryOut champ=$champ baseline=$baseline reason=$reason"
  return $SummaryOut
}

function Start-JobSafe {
  $pair = Test-JobAliveRobust
  if ($pair.alive) {
    Log ("RESTART_SKIP still alive pids=[" + ($pair.pids -join ",") + "]")
    return
  }
  if (-not (Test-Path $Py)) { Log "RESTART_FAIL missing venv"; return }
  $since = ((Get-Date) - $script:LastRestart).TotalSeconds
  if ($since -lt 120) { Log "RESTART_SKIP backoff"; return }
  Start-Sleep -Seconds 25
  $pair2 = Test-JobAliveRobust
  if ($pair2.alive) {
    Log ("RESTART_SKIP other relaunched pids=[" + ($pair2.pids -join ",") + "]")
    if ($pair2.launcherPid) { $script:KnownLauncher = $pair2.launcherPid }
    if ($pair2.workerPid) { $script:KnownWorker = $pair2.workerPid }
    return
  }
  Log ("RESTART via .venv_gpu args=[" + ($script:LastArgs -join " ") + "]")
  $stamp = Get-Date -Format "yyyyMMdd_HHmmss"
  $outLog = Join-Path $Artifacts "overnight_gpu_babysit_restart_$stamp.out.log"
  $errLog = Join-Path $Artifacts "overnight_gpu_babysit_restart_$stamp.err.log"
  $p = Start-Process -FilePath $Py -ArgumentList $script:LastArgs -WorkingDirectory $Repo `
    -RedirectStandardOutput $outLog -RedirectStandardError $errLog -PassThru -WindowStyle Hidden
  $script:LastRestart = Get-Date
  Start-Sleep -Seconds 12
  if ($p -and (-not $p.HasExited)) {
    $script:KnownLauncher = [int]$p.Id
    $np = Get-JobPair
    if ($np.workerPid) {
      $script:KnownWorker = $np.workerPid
      Set-Content -Path $PidFile -Value $np.workerPid -Encoding ascii -NoNewline
    } else {
      Set-Content -Path $PidFile -Value $p.Id -Encoding ascii -NoNewline
    }
    Log "RESTART launcher_pid=$($p.Id) worker=$($script:KnownWorker)"
  } else {
    $err = ""
    if (Test-Path $errLog) { $err = Get-Content $errLog -Raw }
    Log "RESTART_FAIL err=$err"
  }
}

# Seed known PIDs from live job
$seed = Get-JobPair
if ($seed.alive) {
  if ($seed.launcherPid) { $script:KnownLauncher = $seed.launcherPid }
  if ($seed.workerPid) { $script:KnownWorker = $seed.workerPid }
}

Log "START target_iters=$TargetIters deadline=$($Deadline.ToUniversalTime().ToString('o')) known_launcher=$($script:KnownLauncher) known_worker=$($script:KnownWorker)"

while ($true) {
  $j = Read-Latest

  if (Is-OneMillionDone $j) {
    Write-FinalSummary "iters_500k" | Out-Null
    if (-not (Test-Path $DoneFlag)) {
      Set-Content -Path $DoneFlag -Value (UtcNow) -Encoding ascii
    }
    Write-Output 'AGENT_BABYSIT1M_COMPLETE {"reason":"iters_500k"}'
    exit 0
  }

  if ((Get-Date) -ge $Deadline) {
    # Soft deadline: write summary but DO NOT set DONE flag (watchdog must keep going).
    Write-FinalSummary "soft_deadline_240h_incomplete" | Out-Null
    Log "SOFT_DEADLINE reached without 500k - exiting babysit only; job/watchdog continue"
    Write-Output 'AGENT_BABYSIT1M_COMPLETE {"reason":"soft_deadline_no_done_flag"}'
    exit 0
  }

  $pair = Test-JobAliveRobust
  $gpu = "n/a"
  try { $gpu = (nvidia-smi --query-gpu=utilization.gpu,memory.used,temperature.gpu --format=csv,noheader) } catch {}

  if (-not $pair.alive) {
    Log "DEAD confirmed after retries - safe restart"
    Start-JobSafe
    Start-Sleep -Seconds 20
    continue
  }

  if ($pair.launcherPid) { $script:KnownLauncher = $pair.launcherPid }
  if ($pair.workerPid) { $script:KnownWorker = $pair.workerPid }

  $iters = if ($j) { $j.iter } else { "?" }
  $champ = if ($j) { $j.champion_residual } else { "?" }
  $base = if ($j) { $j.baseline_dual_cosine } else { "?" }
  $elapsed = if ($j) { $j.elapsed_sec } else { "?" }
  $pidList = ($pair.pids -join ",")
  Log "HEARTBEAT status=alive launcher=$($pair.launcherPid) worker=$($pair.workerPid) pids=[$pidList] gpu=[$gpu] iters=$iters champ=$champ baseline=$base elapsed_sec=$elapsed"

  $waited = 0
  while ($waited -lt $HeartbeatSec) {
    $j = Read-Latest
    if (Is-OneMillionDone $j) { break }
    if ((Get-Date) -ge $Deadline) { break }
    $probe = Test-JobAliveRobust
    if (-not $probe.alive) {
      Log "JOB_GONE confirmed during wait"
      break
    }
    if ($probe.launcherPid) { $script:KnownLauncher = $probe.launcherPid }
    if ($probe.workerPid) { $script:KnownWorker = $probe.workerPid }
    Start-Sleep -Seconds 30
    $waited += 30
  }
}
