# overnight_gpu_watchdog.ps1
# Detached durable watchdog: every 30m ensure overnight_gpu_rl_arch.py (CUDA) is alive.
# Stops when brand/artifacts/overnight_gpu_DONE.flag exists OR after 24h from start.
# Does not depend on Cursor / any agent.

$ErrorActionPreference = "Continue"
$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $RepoRoot "scripts\overnight_gpu_rl_arch.py"))) {
    $RepoRoot = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
}
Set-Location $RepoRoot

$Artifacts = Join-Path $RepoRoot "brand\artifacts"
$LogPath = Join-Path $Artifacts "overnight_gpu_watchdog.log"
$LaunchLog = Join-Path $Artifacts "overnight_gpu_rl_arch_launch.log"
$PidFile = Join-Path $Artifacts "overnight_gpu_rl_arch.pid"
$DoneFlag = Join-Path $Artifacts "overnight_gpu_DONE.flag"
$IntervalSec = 30 * 60
$MaxHours = 240
$StartedAt = Get-Date
$JobScript = Join-Path $RepoRoot "scripts\overnight_gpu_rl_arch.py"
# Keep aligned with live 500k overnight; only used if watchdog must restart a dead job.
# MUST match babysit / start_overnight_gpu_detached: PPO+GA+PBT+NAS+depth+MoE @ 500k (not complex_arch / 1M).
$SeedFitted = Join-Path $Artifacts "models\gpu-rl-arch-20260719T065704Z\fitted\champion_iter_001964_fitted.json"
$JobArgs = @($JobScript, "--iters", "500000", "--device", "cuda", "--max-hours", "240", "--history-every", "1", "--seed", "1902771841", "--pop-size", "12", "--algo-tag", "PPO+GA+PBT+NAS+depth+MoE", "--plateau-adapt-every", "1000", "--seed-fitted", $SeedFitted)

function Write-Heartbeat([string]$Message) {
    $ts = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
    $line = "[$ts] $Message"
    Add-Content -Path $LogPath -Value $line -Encoding UTF8
}

function Resolve-PythonExe {
    $venvGpu = Join-Path $RepoRoot ".venv_gpu\Scripts\python.exe"
    if (Test-Path $venvGpu) { return $venvGpu }
    $candidates = @(
        (Join-Path $RepoRoot ".venv\Scripts\python.exe"),
        (Join-Path $RepoRoot ".venv312\Scripts\python.exe")
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { return $c }
    }
    return "python"
}

function Get-JobProcesses {
    $matches = @()
    try {
        $procs = Get-CimInstance Win32_Process -Filter "Name = 'python.exe'" -ErrorAction SilentlyContinue
        foreach ($p in $procs) {
            $cl = $p.CommandLine
            if ($null -eq $cl) { continue }
            if ($cl -match "overnight_gpu_rl_arch\.py") {
                $matches += $p
            }
        }
    } catch {}
    return $matches
}

function Test-JobAlive {
    # Prefer pidfile if present and process still alive + matching
    if (Test-Path $PidFile) {
        $raw = (Get-Content $PidFile -Raw -ErrorAction SilentlyContinue).Trim()
        $pidNum = 0
        if ([int]::TryParse($raw, [ref]$pidNum) -and $pidNum -gt 0) {
            $proc = Get-Process -Id $pidNum -ErrorAction SilentlyContinue
            if ($proc) {
                $cim = Get-CimInstance Win32_Process -Filter "ProcessId = $pidNum" -ErrorAction SilentlyContinue
                if ($cim -and $cim.CommandLine -match "overnight_gpu_rl_arch\.py") {
                    return $true, $pidNum
                }
                # PID reused by unrelated process — fall through to scan
            }
        }
    }
    $jobs = @(Get-JobProcesses)
    if ($jobs.Count -gt 0) {
        # Prefer .venv_gpu python if multiple
        $preferred = $jobs | Where-Object { $_.CommandLine -match "\.venv_gpu" } | Select-Object -First 1
        if (-not $preferred) { $preferred = $jobs[0] }
        return $true, [int]$preferred.ProcessId
    }
    return $false, 0
}

function Write-PidFile([int]$ProcessId) {
    if ($ProcessId -gt 0) {
        Set-Content -Path $PidFile -Value $ProcessId -Encoding ASCII -NoNewline
    }
}

function Start-GpuJob {
    $py = Resolve-PythonExe
    $ts = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
    $stamp = Get-Date -Format "yyyyMMdd_HHmmss"
    $outLog = Join-Path $Artifacts "overnight_gpu_watchdog_restart_${stamp}.out.log"
    $errLog = Join-Path $Artifacts "overnight_gpu_watchdog_restart_${stamp}.err.log"
    $msg = "[$ts] WATCHDOG restart: $py $($JobArgs -join ' ') out=$outLog"
    Add-Content -Path $LaunchLog -Value $msg -Encoding UTF8
    Write-Heartbeat "DEAD — restarting via $py"

    $p = Start-Process -FilePath $py `
        -ArgumentList $JobArgs `
        -WorkingDirectory $RepoRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $outLog `
        -RedirectStandardError $errLog `
        -PassThru

    Start-Sleep -Seconds 3
    if ($p -and -not $p.HasExited) {
        Write-PidFile $p.Id
        Write-Heartbeat "Restarted OK pid=$($p.Id)"
        return $p.Id
    }
    $alive, $jid = Test-JobAlive
    if ($alive) {
        Write-PidFile $jid
        Write-Heartbeat "Restarted (found) pid=$jid"
        return $jid
    }
    Write-Heartbeat "Restart FAILED — no process"
    return 0
}

# --- main ---
New-Item -ItemType Directory -Force -Path $Artifacts | Out-Null
Write-Heartbeat "Watchdog START pid=$PID repo=$RepoRoot interval=${IntervalSec}s maxHours=$MaxHours"

$alive, $jid = Test-JobAlive
if ($alive) {
    Write-PidFile $jid
    Write-Heartbeat "Initial job OK pid=$jid"
} else {
    $jid = Start-GpuJob
}

while ($true) {
    if (Test-Path $DoneFlag) {
        Write-Heartbeat "DONE flag present — exiting"
        break
    }
    $elapsed = (Get-Date) - $StartedAt
    if ($elapsed.TotalHours -ge $MaxHours) {
        Write-Heartbeat "Reached ${MaxHours}h wall clock — exiting"
        break
    }

    Start-Sleep -Seconds $IntervalSec

    if (Test-Path $DoneFlag) {
        Write-Heartbeat "DONE flag present — exiting"
        break
    }
    $elapsed = (Get-Date) - $StartedAt
    if ($elapsed.TotalHours -ge $MaxHours) {
        Write-Heartbeat "Reached ${MaxHours}h wall clock — exiting"
        break
    }

    $alive, $jid = Test-JobAlive
    if ($alive) {
        Write-PidFile $jid
        Write-Heartbeat "OK job_pid=$jid elapsed_h=$([math]::Round($elapsed.TotalHours, 2))"
    } else {
        Start-GpuJob | Out-Null
    }
}

Write-Heartbeat "Watchdog STOP"

