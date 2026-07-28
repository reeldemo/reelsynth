# Watchdog for v13 D1: if search dies (crash or after reboot with auto-start),
# relaunch resume-only. Never uses --force-fresh.
# Install for login auto-resume:
#   powershell -File scripts\install_v13_d1_autostart.ps1
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Out = Join-Path $Root "brand\artifacts\meta_approach_compare_v13_rblend"
# Sequential only (one seed at a time) - parallel dual-seed was abandoned due to GPU memory leaks.
$Launch = Join-Path $Root "scripts\launch_v13_multiseed_search.ps1"
$Log = Join-Path $Out "watchdog.log"
New-Item -ItemType Directory -Force -Path $Out | Out-Null

function Log([string]$m) {
    $line = "{0} {1}" -f (Get-Date -Format o), $m
    Add-Content -Path $Log -Value $line
    Write-Host $line
}

function AllComplete {
    $seeds = @(1902771841, 2026072701, 2026072702)
    $aps = @("hybrid_lstm", "random", "cmaes", "tpe", "aging_evo", "reinforce")
    foreach ($s in $seeds) {
        foreach ($ap in $aps) {
            $sum = Join-Path $Out "$s\$ap\summary.json"
            if (-not (Test-Path $sum)) { return $false }
            try {
                $j = Get-Content $sum -Raw | ConvertFrom-Json
                $done = [int]($j.iters_done); if (-not $done) { $done = [int]$j.iters }
                if ($done -lt 5000) { return $false }
            } catch { return $false }
        }
    }
    return $true
}

Log "watchdog start (resume-only; reboot-safe)"
while ($true) {
    if (AllComplete) {
        Log "all_complete - watchdog exit"
        break
    }

    # Never allow a wipe agent on this tree
    Get-CimInstance Win32_Process -Filter "Name='python.exe'" -ErrorAction SilentlyContinue |
        Where-Object {
            $_.CommandLine -like "*meta_approach_compare_v13_rblend*" -and
            ($_.CommandLine -like "*force-fresh*" -or $_.CommandLine -like "*no-resume*")
        } |
        ForEach-Object {
            Log "KILL wipe agent PID=$($_.ProcessId)"
            Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
        }

    $any = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            $_.CommandLine -and
            $_.CommandLine -like "*bench_meta_approaches*" -and
            $_.CommandLine -like "*meta_approach_compare_v13_rblend*" -and
            $_.CommandLine -notlike "*force-fresh*"
        }
    $launcherAlive = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            $_.CommandLine -and (
                $_.CommandLine -like "*launch_v13_multiseed_search*" -or
                $_.CommandLine -like "*launch_v13_multiseed_parallel*"
            )
        }

    if (-not $any -and -not $launcherAlive) {
        Log "bench dead; restarting RESUME-ONLY launcher"
        Start-Process -FilePath "powershell.exe" `
            -ArgumentList "-NoProfile","-ExecutionPolicy","Bypass","-File",$Launch `
            -WorkingDirectory $Root `
            -RedirectStandardOutput (Join-Path $Out "watchdog_launch_stdout.log") `
            -RedirectStandardError (Join-Path $Out "watchdog_launch_stderr.log") `
            -WindowStyle Hidden
        Start-Sleep -Seconds 30
    } else {
        Log "ok alive"
    }
    Start-Sleep -Seconds 120
}
