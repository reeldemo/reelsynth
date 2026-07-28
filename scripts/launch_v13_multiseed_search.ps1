# Idempotent v13 D1 launcher - safe to re-run after reboot / crash.
# Resumes from checkpoint.json per approach (never --force-fresh).
# Usage after reboot:
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\launch_v13_multiseed_search.ps1
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$Seeds = @(1902771841, 2026072701, 2026072702)
$Iters = 5000
$CkptEvery = 25
# hybrid + random first for learning curves; reinforce last
$Approaches = @("hybrid_lstm", "random", "cmaes", "tpe", "aging_evo", "reinforce")
$ApproachesCsv = $Approaches -join ","
$BaseOut = Join-Path $Root "brand\artifacts\meta_approach_compare_v13_rblend"
New-Item -ItemType Directory -Force -Path $BaseOut | Out-Null

$MasterLog = Join-Path $BaseOut "launcher.log"
function Log([string]$msg) {
    $line = "[{0}] {1}" -f (Get-Date -Format "yyyy-MM-ddTHH:mm:ss"), $msg
    Add-Content -Path $MasterLog -Value $line
    Write-Host $line
}

function ApproachDone([string]$seedDir, [string]$ap) {
    $sum = Join-Path (Join-Path $seedDir $ap) "summary.json"
    if (-not (Test-Path $sum)) { return $false }
    try {
        $j = Get-Content $sum -Raw | ConvertFrom-Json
        $done = [int]($j.iters_done)
        if (-not $done) { $done = [int]($j.iters) }
        return ($done -ge $Iters)
    } catch {
        return $false
    }
}

function SeedDone([string]$seedDir) {
    foreach ($ap in $Approaches) {
        if (-not (ApproachDone $seedDir $ap)) { return $false }
    }
    return $true
}

function CkptIters([string]$seedDir, [string]$ap) {
    $ckpt = Join-Path (Join-Path $seedDir $ap) "checkpoint.json"
    if (-not (Test-Path $ckpt)) { return 0 }
    try {
        $j = Get-Content $ckpt -Raw | ConvertFrom-Json
        return [int]($j.iters_done)
    } catch {
        return 0
    }
}

$Py = Join-Path $Root ".venv_gpu\Scripts\python.exe"
if (-not (Test-Path $Py)) { throw "missing GPU venv python: $Py" }

# Single-flight lock: refuse if another v13 launcher / same-out bench is already running
$existing = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
    Where-Object {
        $_.CommandLine -and
        $_.CommandLine -like "*bench_meta_approaches_5k*" -and
        $_.CommandLine -like "*meta_approach_compare_v13_rblend*"
    }
if ($existing) {
    $pids = ($existing | ForEach-Object { $_.ProcessId }) -join ","
    Log "ALREADY_RUNNING pids=$pids - exit 0 (resume-safe; do not start a second copy)"
    exit 0
}

Log "RESUME_OR_START v13 multiseed iters=$Iters ckpt_every=$CkptEvery seeds=$($Seeds -join ',') approaches=$ApproachesCsv"

foreach ($Seed in $Seeds) {
    $OutDir = Join-Path $BaseOut "$Seed"
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

    if (SeedDone $OutDir) {
        Log "SEED $Seed SKIP all approaches complete"
        continue
    }

    $progress = @()
    foreach ($ap in $Approaches) {
        if (ApproachDone $OutDir $ap) {
            $progress += "${ap}=DONE"
        } else {
            $n = CkptIters $OutDir $ap
            $progress += "${ap}=$n/$Iters"
        }
    }
    Log "SEED $Seed RESUME $($progress -join ' ')"

    $SeedLog = Join-Path $OutDir "run.log"
    $pyArgs = @(
        "-u",
        "scripts\bench_meta_approaches_5k.py",
        "--iters", "$Iters",
        "--seed", "$Seed",
        "--out-dir", "$OutDir",
        "--device", "cuda",
        "--ckpt-every", "$CkptEvery",
        "--approaches", $ApproachesCsv
    )
    # No --force-fresh: always resume from checkpoint.json
    & $Py @pyArgs 2>&1 | Tee-Object -FilePath $SeedLog -Append
    $code = $LASTEXITCODE
    Log "SEED $Seed exit=$code"
    if ($code -ne 0) {
        Log "SEED $Seed FAILED - will retry on next launch/watchdog (checkpoints kept)"
        # Do not skip remaining seeds forever: continue so other seeds progress;
        # watchdog/relaunch will retry this seed from ckpt.
    }
}

# Final aggregate if everything complete
$allDone = $true
foreach ($Seed in $Seeds) {
    if (-not (SeedDone (Join-Path $BaseOut "$Seed"))) { $allDone = $false }
}
if ($allDone) {
    Log "ALL SEEDS COMPLETE - running finish helper"
    & $Py "scripts\finish_v13_d1_when_ready.py" 2>&1 | Tee-Object -FilePath (Join-Path $BaseOut "finish.log") -Append
    Log "FINISH exit=$LASTEXITCODE"
} else {
    Log "PARTIAL - re-run this script or let watchdog resume after reboot"
}

exit 0
