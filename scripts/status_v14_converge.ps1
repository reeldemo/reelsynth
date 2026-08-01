# Print v14 FitCell-converge multi-seed search progress + latest fit stats.
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$BaseOut = Join-Path $Root "brand\artifacts\meta_approach_compare_v14_converge"
$Seeds = @(1902771841, 2026072701, 2026072702)
$Approaches = @("hybrid_lstm")
$Iters = 750

Write-Host "=== v14 converge status ($BaseOut) ==="
$alive = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
    Where-Object {
        $_.CommandLine -and
        $_.CommandLine -like "*meta_approach_compare_v14_converge*" -and
        $_.CommandLine -like "*bench_meta*"
    }
if ($alive) {
    $pids = ($alive | ForEach-Object { "PID=$($_.ProcessId)" }) -join ", "
    Write-Host "RUNNING: $pids"
} else {
    Write-Host "NOT RUNNING - start: .\scripts\launch_v14_converge_search.ps1"
}

$status = Join-Path $BaseOut "STATUS.json"
if (-not (Test-Path $status)) {
    $status = Join-Path $Root "brand\artifacts\meta_approach_STATUS.json"
}
if (Test-Path $status) {
    try {
        $s = Get-Content $status -Raw | ConvertFrom-Json
        Write-Host ("STATUS phase={0} current={1} iter={2} updated={3}" -f $s.phase, $s.current_approach, $s.current_iter, $s.updated_at)
    } catch {}
}

foreach ($Seed in $Seeds) {
    $seedDir = Join-Path $BaseOut "$Seed"
    Write-Host ""
    Write-Host "seed $Seed"
    foreach ($ap in $Approaches) {
        $ad = Join-Path $seedDir $ap
        $sum = Join-Path $ad "summary.json"
        $ckpt = Join-Path $ad "checkpoint.json"
        $hist = Join-Path $ad "history.jsonl"
        $fitInfo = ""
        if (Test-Path $hist) {
            try {
                $last = Get-Content $hist -Tail 1 | ConvertFrom-Json
                $fitInfo = (" fit_steps={0} ok={1} free_mib={2}" -f $last.fit_steps_used, $last.fit_converged, $last.cuda_free_mib)
            } catch {}
        }
        if (Test-Path $sum) {
            try {
                $j = Get-Content $sum -Raw | ConvertFrom-Json
                $done = [int]($j.iters_done)
                if (-not $done) { $done = [int]$j.iters }
                $r = $j.champ_raw
                if (-not $r) { $r = $j.champ_r }
                if ($done -ge $Iters) {
                    Write-Host ("  {0,-12} DONE  R_blend={1:N6}{2}" -f $ap, [double]$r, $fitInfo)
                } else {
                    Write-Host ("  {0,-12} summary {1}/{2} R={3}{4}" -f $ap, $done, $Iters, $r, $fitInfo)
                }
                continue
            } catch {}
        }
        if (Test-Path $ckpt) {
            try {
                $j = Get-Content $ckpt -Raw | ConvertFrom-Json
                $done = [int]$j.iters_done
                $r = $j.champ_raw
                if (-not $r) { $r = $j.champ_r }
                $mtime = (Get-Item $ckpt).LastWriteTime.ToString("HH:mm:ss")
                Write-Host ("  {0,-12} ckpt  {1}/{2} champ={3:N6} (saved {4}){5}" -f $ap, $done, $Iters, [double]$r, $mtime, $fitInfo)
                continue
            } catch {}
        }
        Write-Host ("  {0,-12} pending" -f $ap)
    }
}
