# start_overnight_gpu_detached.ps1
# Fully OS-detached overnight GPU RL launch (survives Cursor crash).
# Uses Start-Process Hidden with file redirects — no pipe to calling shell.

param(
    [int]$Iters = 500000,
    [double]$MaxHours = 240,
    [int]$HistoryEvery = 1,
    [string]$Device = "cuda",
    [long]$Seed = 1902771841,
    [int]$PopSize = 12,
    [string]$AlgoTag = "PPO+GA+PBT+NAS+depth+MoE",
    [string]$SeedFitted = "",
    [string]$RepoRoot = ""
)

$ErrorActionPreference = "Stop"

if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path (Join-Path $RepoRoot "scripts\overnight_gpu_rl_arch.py"))) {
    $RepoRoot = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
}

$Artifacts = Join-Path $RepoRoot "brand\artifacts"
$JobScript = Join-Path $RepoRoot "scripts\overnight_gpu_rl_arch.py"
$PidFile = Join-Path $Artifacts "overnight_gpu_rl_arch.pid"
$LaunchLog = Join-Path $Artifacts "overnight_gpu_rl_arch_launch.log"
$VenvPy = Join-Path $RepoRoot ".venv_gpu\Scripts\python.exe"

New-Item -ItemType Directory -Force -Path $Artifacts | Out-Null

if (-not (Test-Path $VenvPy)) {
    throw "Missing .venv_gpu python: $VenvPy"
}

# Timing / ETA estimate for launch log (Europe/Berlin local ≈ UTC+2 in Jul).
$now = Get-Date
$deadline = Get-Date -Year $now.Year -Month $now.Month -Day ($now.Day + 1) -Hour 8 -Minute 0 -Second 0
# If already past midnight toward Sunday 08:00, clamp: if now is Sat afternoon, +1 day is Sunday 08:00.
if ($now.DayOfWeek -eq [DayOfWeek]::Sunday -and $now.Hour -lt 8) {
    $deadline = Get-Date -Year $now.Year -Month $now.Month -Day $now.Day -Hour 8 -Minute 0 -Second 0
}
$hoursLeft = [math]::Round(($deadline - $now).TotalHours, 2)
$rateAssumed = 13.1  # measured ~13.08–13.10 it/s on RTX 3090
$etaHoursAtRate = [math]::Round($Iters / $rateAssumed / 3600.0, 2)
$stamp = Get-Date -Format "yyyyMMdd_HHmmss"
$outLog = Join-Path $Artifacts "overnight_gpu_rl_arch_detached_${stamp}.out.log"
$errLog = Join-Path $Artifacts "overnight_gpu_rl_arch_detached_${stamp}.err.log"

$argList = @(
    $JobScript,
    "--iters", "$Iters",
    "--device", $Device,
    "--max-hours", "$MaxHours",
    "--history-every", "$HistoryEvery",
    "--seed", "$Seed",
    "--pop-size", "$PopSize",
    "--algo-tag", $AlgoTag,
    "--plateau-adapt-every", "1000"
)
if ($SeedFitted) {
    $argList += @("--seed-fitted", $SeedFitted)
}

$estimate = @(
    "DETACHED_LAUNCH estimate_now=$($now.ToString('yyyy-MM-dd HH:mm:ss')) local",
    "deadline_sun_0800=$($deadline.ToString('yyyy-MM-dd HH:mm:ss')) hours_left≈$hoursLeft",
    "target_iters=$Iters max_hours=$MaxHours history_every=$HistoryEvery",
    "assumed_rate=${rateAssumed}/s => full_target_eta_h=$etaHoursAtRate",
    "note=if rate holds, wall clock may hit max-hours before completing all iters; paper target remains $Iters"
) -join " | "

$ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
Add-Content -Path $LaunchLog -Value "[$ts] $estimate" -Encoding UTF8
Add-Content -Path $LaunchLog -Value "[$ts] Start-Process Hidden: $VenvPy $($argList -join ' ') out=$outLog err=$errLog" -Encoding UTF8

$p = Start-Process -FilePath $VenvPy `
    -ArgumentList $argList `
    -WorkingDirectory $RepoRoot `
    -WindowStyle Hidden `
    -RedirectStandardOutput $outLog `
    -RedirectStandardError $errLog `
    -PassThru

if (-not $p) {
    throw "Start-Process returned null"
}

# Wait briefly for possible venv re-exec child, then prefer the worker that owns CUDA work.
Start-Sleep -Seconds 4
$launcherPid = $p.Id
$workerPid = $launcherPid
$children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $launcherPid" -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match "python" -and $_.CommandLine -match "overnight_gpu_rl_arch\.py" }
if ($children) {
    $workerPid = [int]$children[0].ProcessId
}

# Pidfile: write worker PID (watchdog monitors this). Also record both in launch log.
Set-Content -Path $PidFile -Value $workerPid -Encoding ASCII -NoNewline
$meta = @{
    launcher_pid = $launcherPid
    worker_pid   = $workerPid
    iters        = $Iters
    max_hours    = $MaxHours
    history_every = $HistoryEvery
    device       = $Device
    out_log      = $outLog
    err_log      = $errLog
    started_at   = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    detached     = $true
} | ConvertTo-Json -Compress
Add-Content -Path $LaunchLog -Value "[$ts] DETACHED_OK $meta" -Encoding UTF8

Write-Output "DETACHED launcher_pid=$launcherPid worker_pid=$workerPid pidfile=$PidFile"
Write-Output "estimate: $estimate"
exit 0
