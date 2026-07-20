# overnight_1m_durable_finisher.ps1
# Cursor-independent: polls until TARGET 500k (or DONE after target), then plots + Klaut paper v4 + commit/push.
# Launch with Start-Process -WindowStyle Hidden so it survives Cursor exit.

$ErrorActionPreference = "Continue"
$Repo = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
$Meta = "C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta"
$Art = Join-Path $Repo "brand\artifacts"
$Log = Join-Path $Art "overnight_1m_durable_finisher.log"
$PidFile = Join-Path $Art "overnight_1m_durable_finisher.pid"
$StateFile = Join-Path $Art "overnight_1m_durable_finisher_state.json"
$Latest = Join-Path $Art "overnight_gpu_rl_arch_latest.json"
$DoneFlag = Join-Path $Art "overnight_gpu_DONE.flag"
$FinishPy = Join-Path $Repo "scripts\finish_overnight_1m_to_paper.py"
$VenvPy = Join-Path $Repo ".venv_gpu\Scripts\python.exe"
$TargetIters = 500000
$PollSec = 120
$MaxWaitHours = 240

Set-Content -Path $PidFile -Value $PID -Encoding ascii -NoNewline

function Log([string]$msg) {
  $line = "$(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssK') FINISHER $msg"
  Add-Content -Path $Log -Value $line -Encoding UTF8
  Write-Output $line
}

function Write-State([hashtable]$h) {
  $h["ts"] = (Get-Date).ToUniversalTime().ToString("o")
  $h["finisher_pid"] = $PID
  ($h | ConvertTo-Json -Compress) | Set-Content -Path $StateFile -Encoding utf8
}

function Read-Latest {
  if (-not (Test-Path $Latest)) { return $null }
  try { return Get-Content $Latest -Raw | ConvertFrom-Json } catch { return $null }
}

function Test-TrainingAlive {
  $jobs = @(Get-CimInstance Win32_Process -Filter "Name='python.exe'" -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -and ($_.CommandLine -match 'overnight_gpu_rl_arch\.py') })
  return ($jobs.Count -gt 0), $jobs
}

function Test-OneMillionDone {
  $j = Read-Latest
  if ($j -and ([int]$j.iter -ge $TargetIters)) { return $true, $j }
  if (Test-Path $DoneFlag) {
    if ($j -and ([int]$j.iter -ge $TargetIters)) { return $true, $j }
  }
  $dirs = Get-ChildItem (Join-Path $Art "models") -Directory -Filter "gpu-rl-arch-*" -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending
  foreach ($d in $dirs) {
    $fs = Join-Path $d.FullName "final_summary.json"
    if (Test-Path $fs) {
      try {
        $s = Get-Content $fs -Raw | ConvertFrom-Json
        $doneIters = 0
        if ($s.iters_done) { $doneIters = [int]$s.iters_done }
        elseif ($s.iter) { $doneIters = [int]$s.iter }
        if ($doneIters -ge $TargetIters) { return $true, $j }
      } catch {}
    }
  }
  return $false, $j
}

function Ensure-JobIfDead {
  $alive, $jobs = Test-TrainingAlive
  if ($alive) { return }
  Log "TRAINING_DEAD - restarting via start_overnight_gpu_detached.ps1 (500k max-hours 240 PPO+GA+depth+MoE)"
  $launcher = Join-Path $Repo "scripts\start_overnight_gpu_detached.ps1"
  $seedFitted = Join-Path $Art "models\gpu-rl-arch-20260719T065704Z\fitted\champion_iter_001964_fitted.json"
  if (Test-Path $launcher) {
    & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $launcher -Iters 500000 -MaxHours 240 -HistoryEvery 1 -Device cuda -Seed 1902771841 -AlgoTag "PPO+GA+PBT+NAS+depth+MoE" -SeedFitted $seedFitted
  } else {
    if (-not (Test-Path $VenvPy)) { Log "NO_VENV"; return }
    $argList = @(
      (Join-Path $Repo "scripts\overnight_gpu_rl_arch.py"),
      "--iters", "500000", "--device", "cuda", "--max-hours", "240", "--history-every", "1", "--seed", "1902771841", "--pop-size", "12", "--algo-tag", "PPO+GA+PBT+NAS+depth+MoE", "--plateau-adapt-every", "1000", "--seed-fitted", $seedFitted
    )
    Start-Process -FilePath $VenvPy -ArgumentList $argList -WorkingDirectory $Repo -WindowStyle Hidden
  }
  Start-Sleep -Seconds 15
}

function Invoke-FinishPipeline {
  Log "START_FINISH plots+paper"
  Write-State @{ phase = "finishing"; iter = $TargetIters }

  if (-not (Test-Path $VenvPy)) { throw "Missing venv python" }
  if (-not (Test-Path $FinishPy)) { throw "Missing finish script" }

  $outLog = Join-Path $Art "overnight_1m_finish_run.out.log"
  $errLog = Join-Path $Art "overnight_1m_finish_run.err.log"
  $env:RESEARCH_PAPERS_DIR = Join-Path $Meta "paper\klaut_artifacts"
  $env:OLLAMA_BASE_URL = "http://127.0.0.1:11434/v1"
  $env:OLLAMA_MODEL = "qwen3.5:9b"
  $env:OLLAMA_API_KEY = "ollama"
  $env:KLAUT_RESEARCH_MODE = "local"

  $proc = Start-Process -FilePath $VenvPy -ArgumentList @($FinishPy) `
    -WorkingDirectory $Repo -Wait -PassThru -NoNewWindow `
    -RedirectStandardOutput $outLog -RedirectStandardError $errLog
  Log ("finish_py exit=" + $proc.ExitCode + " out=" + $outLog)
  if ($proc.ExitCode -ne 0) {
    Write-State @{ phase = "finish_failed"; exit_code = $proc.ExitCode }
    throw ("finish_overnight_1m_to_paper.py failed exit=" + $proc.ExitCode)
  }

  $pdf = Join-Path $Meta "paper\v4\main.pdf"
  if (-not (Test-Path $pdf)) {
    $idFile = Join-Path $Meta "paper\v4\KLAUT_PAPER_ID.txt"
    if (Test-Path $idFile) {
      $pidName = (Get-Content $idFile -Raw).Trim()
      $curFile = Join-Path $Meta "paper\klaut_artifacts\$pidName\CURRENT"
      $ver = "v01"
      if (Test-Path $curFile) { $ver = (Get-Content $curFile -Raw).Trim() }
      $srcPdf = Join-Path $Meta "paper\klaut_artifacts\$pidName\$ver\main.pdf"
      if (Test-Path $srcPdf) {
        Copy-Item $srcPdf $pdf -Force
        Log ("copied pdf from klaut " + $srcPdf)
      }
    }
  }
  if (-not (Test-Path $pdf)) {
    Write-State @{ phase = "pdf_missing" }
    throw "paper/v4/main.pdf missing after finish"
  }

  Log "GIT commit/push"
  Push-Location $Meta
  try {
    git add paper/v4 paper/klaut_artifacts 2>$null
    $pending = git status --porcelain paper/v4 paper/klaut_artifacts
    if ($pending) {
      git commit -m "docs(paper): ship DenoiseOpt v4 after 500k overnight RL/NAS run"
      git push origin HEAD
      Log "meta pushed"
    } else { Log "meta nothing to commit" }
  } catch { Log ("meta git err " + $_) }
  finally { Pop-Location }

  Push-Location $Repo
  try {
    git add scripts/overnight_1m_durable_finisher.ps1 scripts/finish_overnight_1m_to_paper.py 2>$null
    git add brand/artifacts/figures 2>$null
    git add brand/artifacts/overnight_gpu_final_summary.json 2>$null
    git add brand/artifacts/overnight_1m_pipeline_status.json 2>$null
    $st = git status --porcelain scripts/ brand/artifacts/figures brand/artifacts/overnight_gpu_final_summary.json brand/artifacts/overnight_1m_pipeline_status.json
    if ($st) {
      git commit -m "chore: 500k overnight figures and durable finisher artifacts"
      git push origin HEAD
      Log "reelsynth pushed"
    } else { Log "reelsynth nothing critical to commit" }
  } catch { Log ("reelsynth git err " + $_) }
  finally { Pop-Location }

  Write-State @{
    phase = "complete"
    pdf = $pdf
    pdf_exists = $true
  }
  Log ("COMPLETE pdf=" + $pdf)
}

# --- main ---
Log ("START pid=" + $PID + " repo=" + $Repo + " poll=" + $PollSec + "s maxWaitH=" + $MaxWaitHours + " target=" + $TargetIters)
Write-State @{ phase = "waiting"; script = $PSCommandPath }

$started = Get-Date
while ($true) {
  $elapsedH = ((Get-Date) - $started).TotalHours
  if ($elapsedH -ge $MaxWaitHours) {
    Log ("SOFT_DEADLINE " + $MaxWaitHours + "h without target iters - exiting (no false DONE)")
    Write-State @{ phase = "soft_deadline"; elapsed_h = $elapsedH }
    exit 2
  }

  Ensure-JobIfDead

  $done, $j = Test-OneMillionDone
  $alive, $procs = Test-TrainingAlive
  $iter = 0
  if ($j) { $iter = [int]$j.iter }
  $champ = $null
  if ($j) { $champ = $j.champion_residual }
  $wp = $null
  foreach ($pr in $procs) {
    if ($pr.CommandLine -notmatch '\\.venv_gpu\\') { $wp = $pr.ProcessId; break }
  }
  if (-not $wp -and $procs.Count -gt 0) { $wp = $procs[0].ProcessId }

  $histLines = 0
  if ($j -and $j.run_dir) {
    $hist = Join-Path $j.run_dir "history.jsonl"
    if (Test-Path $hist) {
      try { $histLines = (Get-Content $hist | Measure-Object -Line).Lines } catch {}
    }
  }

  $phase = "waiting"
  if ($done) { $phase = "ready_to_finish" }
  Write-State @{
    phase = $phase
    iter = $iter
    champ = $champ
    training_alive = $alive
    worker_pid = $wp
    history_lines = $histLines
  }
  Log ("HEARTBEAT iter=" + $iter + "/" + $TargetIters + " alive=" + $alive + " worker=" + $wp + " hist=" + $histLines + " champ=" + $champ)

  if ($done) {
    try {
      Invoke-FinishPipeline
      exit 0
    } catch {
      $errMsg = "$_"
      Log ("FINISH_ERROR " + $errMsg)
      Write-State @{ phase = "finish_error"; error = $errMsg }
      Start-Sleep -Seconds 1800
      continue
    }
  }

  Start-Sleep -Seconds $PollSec
}
