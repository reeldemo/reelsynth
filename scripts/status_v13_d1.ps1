# Print v13 D1 multi-seed search progress (checkpoint-aware).
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$BaseOut = Join-Path $Root "brand\artifacts\meta_approach_compare_v13_rblend"
$Seeds = @(1902771841, 2026072701, 2026072702)
$Approaches = @("hybrid_lstm", "random", "cmaes", "tpe", "aging_evo", "reinforce")
$Iters = 5000

Write-Host "=== v13 D1 status ($BaseOut) ==="
$alive = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
    Where-Object { $_.CommandLine -and $_.CommandLine -like "*meta_approach_compare_v13_rblend*" -and $_.CommandLine -like "*bench_meta*" }
if ($alive) {
    Write-Host ("RUNNING: " + (($alive | ForEach-Object { "PID=$($_.ProcessId)" }) -join ", "))
} else {
    Write-Host "NOT RUNNING — after reboot: .\scripts\launch_v13_multiseed_search.ps1"
}

foreach ($Seed in $Seeds) {
    $seedDir = Join-Path $BaseOut "$Seed"
    Write-Host ""
    Write-Host "seed $Seed"
    foreach ($ap in $Approaches) {
        $ad = Join-Path $seedDir $ap
        $sum = Join-Path $ad "summary.json"
        $ckpt = Join-Path $ad "checkpoint.json"
        if (Test-Path $sum) {
            try {
                $j = Get-Content $sum -Raw | ConvertFrom-Json
                $done = [int]($j.iters_done); if (-not $done) { $done = [int]$j.iters }
                $r = $j.champ_raw; if (-not $r) { $r = $j.champ_r }
                if ($done -ge $Iters) {
                    Write-Host ("  {0,-12} DONE  R_blend={1:N6}" -f $ap, [double]$r)
                } else {
                    Write-Host ("  {0,-12} summary {1}/{2} R={3}" -f $ap, $done, $Iters, $r)
                }
                continue
            } catch {}
        }
        if (Test-Path $ckpt) {
            try {
                $j = Get-Content $ckpt -Raw | ConvertFrom-Json
                $done = [int]$j.iters_done
                $r = $j.champ_raw; if (-not $r) { $r = $j.champ_r }
                $mtime = (Get-Item $ckpt).LastWriteTime.ToString("HH:mm:ss")
                Write-Host ("  {0,-12} ckpt  {1}/{2} champ={3:N6} (saved {4})" -f $ap, $done, $Iters, [double]$r, $mtime)
                continue
            } catch {}
        }
        Write-Host ("  {0,-12} pending" -f $ap)
    }
}
