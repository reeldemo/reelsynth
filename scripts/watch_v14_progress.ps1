# Live v14 hybrid converge progress (refreshes every 2s). Ctrl+C to stop.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\watch_v14_progress.ps1
param(
    [int]$IntervalSec = 2
)
$ErrorActionPreference = "Continue"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$BaseOut = Join-Path $Root "brand\artifacts\meta_approach_compare_v14_converge"
$Seeds = @(1902771841, 2026072701, 2026072702)
$Iters = 750
$Ap = "hybrid_lstm"

function Read-Done([string]$ad) {
    $ckpt = Join-Path $ad "checkpoint.json"
    $sum = Join-Path $ad "summary.json"
    $hist = Join-Path $ad "history.jsonl"
    $done = 0
    $champ = $null
    if (Test-Path $hist) {
        try {
            $last = Get-Content $hist -Tail 1 | ConvertFrom-Json
            $done = [Math]::Max($done, [int]($last.iter))
            if ($null -ne $last.champ_raw) { $champ = [double]$last.champ_raw }
        } catch {}
    }
    if (Test-Path $ckpt) {
        try {
            $j = Get-Content $ckpt -Raw | ConvertFrom-Json
            $done = [Math]::Max($done, [int]($j.iters_done))
            if ($null -eq $champ) {
                if ($j.champ_raw) { $champ = [double]$j.champ_raw }
                elseif ($j.champ_r) { $champ = [double]$j.champ_r }
            }
        } catch {}
    }
    if (Test-Path $sum) {
        try {
            $j = Get-Content $sum -Raw | ConvertFrom-Json
            $d = [int]($j.iters_done); if (-not $d) { $d = [int]$j.iters }
            $done = [Math]::Max($done, $d)
            if ($null -eq $champ) {
                if ($j.champ_raw) { $champ = [double]$j.champ_raw }
                elseif ($j.champ_r) { $champ = [double]$j.champ_r }
            }
        } catch {}
    }
    return @{ Done = $done; Champ = $champ }
}

while ($true) {
    $alive = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue |
        Where-Object {
            $_.CommandLine -and
            $_.CommandLine -like "*meta_approach_compare_v14_converge*" -and
            $_.CommandLine -like "*bench_meta*"
        }
    $run = if ($alive) { "RUNNING" } else { "STOPPED" }

    $totalTarget = $Seeds.Count * $Iters
    $totalDone = 0
    $lines = @()
    foreach ($Seed in $Seeds) {
        $ad = Join-Path (Join-Path $BaseOut "$Seed") $Ap
        $st = Read-Done $ad
        $done = [int]$st.Done
        if ($done -gt $Iters) { $done = $Iters }
        $totalDone += $done
        $pct = 100.0 * $done / $Iters
        $barLen = 20
        $filled = [int][Math]::Round($barLen * $done / $Iters)
        if ($filled -gt $barLen) { $filled = $barLen }
        $bar = ("#" * $filled) + ("-" * ($barLen - $filled))
        $champStr = if ($null -ne $st.Champ) { ("R={0:N6}" -f $st.Champ) } else { "R=—" }
        $lines += ("  {0} [{1}] {2,3}/{3} {4,5:N1}%  {5}" -f $Seed, $bar, $done, $Iters, $pct, $champStr)
    }
    $overall = 100.0 * $totalDone / $totalTarget
    $ts = Get-Date -Format "HH:mm:ss"
    Clear-Host
    Write-Host ("v14 hybrid-only  {0}  {1}  overall {2}/{3} ({4:N1}%)" -f $ts, $run, $totalDone, $totalTarget, $overall)
    Write-Host ("out: {0}" -f $BaseOut)
    Write-Host ""
    $lines | ForEach-Object { Write-Host $_ }
    Write-Host ""
    Write-Host "Ctrl+C to stop  |  refresh ${IntervalSec}s"
    Start-Sleep -Seconds $IntervalSec
}
