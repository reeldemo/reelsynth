# Parallel v13 D1 launcher: up to MaxParallel seeds at once (default 2).
# Each seed runs approaches sequentially. Resume-only (never --force-fresh).
param(
    [int]$MaxParallel = 2
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$Seeds = @(1902771841, 2026072701, 2026072702)
$Iters = 5000
$CkptEvery = 25
$Approaches = @("hybrid_lstm", "random", "cmaes", "tpe", "aging_evo", "reinforce")
$ApproachesCsv = ($Approaches -join ",")
$BaseOut = Join-Path $Root "brand\artifacts\meta_approach_compare_v13_rblend"
New-Item -ItemType Directory -Force -Path $BaseOut | Out-Null

$MasterLog = Join-Path $BaseOut "parallel_launcher.log"
$Py = Join-Path $Root ".venv_gpu\Scripts\python.exe"
if (-not (Test-Path $Py)) { throw "missing GPU venv python: $Py" }

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
        $done = 0
        if ($null -ne $j.iters_done) { $done = [int]$j.iters_done }
        elseif ($null -ne $j.iters) { $done = [int]$j.iters }
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

Log "PARALLEL START max=$MaxParallel seeds=$($Seeds -join ',') approaches=$ApproachesCsv"

$pending = New-Object System.Collections.ArrayList
foreach ($s in $Seeds) {
    $sd = Join-Path $BaseOut "$s"
    if (SeedDone $sd) {
        Log "SEED $s already complete at launch"
    } else {
        [void]$pending.Add($s)
        Log "SEED $s queued"
    }
}

$active = New-Object System.Collections.ArrayList

function Start-SeedWorker([int]$Seed) {
    $OutDir = Join-Path $BaseOut "$Seed"
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    $SeedLog = Join-Path $OutDir "run_parallel.log"
    $ErrLog = Join-Path $OutDir "run_parallel.err.log"
    $Lock = Join-Path $OutDir "RUNNING.lock"

    if (Test-Path $Lock) {
        $old = Get-Content $Lock -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($old -match "pid=(\d+)") {
            $opid = [int]$Matches[1]
            if (Get-Process -Id $opid -ErrorAction SilentlyContinue) {
                Log "SEED $Seed locked by live PID=$opid - skip"
                return $null
            }
        }
        Remove-Item -Force -ErrorAction SilentlyContinue $Lock
    }

    Log "SEED $Seed START worker"
    "pid=pending seed=$Seed started=$(Get-Date -Format o)" | Set-Content -Path $Lock

    $argList = "-u scripts\bench_meta_approaches_5k.py --iters $Iters --seed $Seed --out-dir `"$OutDir`" --device cuda --ckpt-every $CkptEvery --approaches $ApproachesCsv"
    $p = Start-Process -FilePath $Py -ArgumentList $argList -WorkingDirectory $Root `
        -RedirectStandardOutput $SeedLog -RedirectStandardError $ErrLog `
        -PassThru -WindowStyle Hidden

    "pid=$($p.Id) seed=$Seed started=$(Get-Date -Format o)" | Set-Content -Path $Lock
    Log "SEED $Seed python PID=$($p.Id)"
    return [pscustomobject]@{ Seed = $Seed; Process = $p; Lock = $Lock; OutDir = $OutDir }
}

while ($pending.Count -gt 0 -or $active.Count -gt 0) {
    while ($active.Count -lt $MaxParallel -and $pending.Count -gt 0) {
        $next = [int]$pending[0]
        $pending.RemoveAt(0)
        $job = Start-SeedWorker $next
        if ($null -ne $job) {
            [void]$active.Add($job)
        }
    }

    if ($active.Count -eq 0) {
        Log "No active workers; pending=$($pending.Count) - break"
        break
    }

    Start-Sleep -Seconds 20

    for ($i = $active.Count - 1; $i -ge 0; $i--) {
        $job = $active[$i]
        if ($job.Process.HasExited) {
            $code = $job.Process.ExitCode
            Log "SEED $($job.Seed) exit=$code"
            Remove-Item -Force -ErrorAction SilentlyContinue $job.Lock
            $active.RemoveAt($i)
            if ($code -ne 0) {
                Log "SEED $($job.Seed) FAILED - re-run launcher to resume from checkpoint"
            }
        }
    }
}

$allDone = $true
foreach ($s in $Seeds) {
    if (-not (SeedDone (Join-Path $BaseOut "$s"))) { $allDone = $false }
}
if ($allDone) {
    Log "ALL SEEDS COMPLETE - finish helper"
    & $Py "scripts\finish_v13_d1_when_ready.py" 2>&1 | Tee-Object -FilePath (Join-Path $BaseOut "finish.log") -Append
} else {
    Log "PARTIAL - re-run parallel launcher to resume"
}
Log "PARALLEL EXIT"
exit 0
