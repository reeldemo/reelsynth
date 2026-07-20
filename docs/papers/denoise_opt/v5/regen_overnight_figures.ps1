# One-command plot redo when overnight gate advances (run from anywhere)
# Truncates to the paper-reported freeze iter (6922) so figures match Results claims.
$ErrorActionPreference = "Stop"
$Reel = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
$Py = Join-Path $Reel ".venv_gpu\Scripts\python.exe"
if (-not (Test-Path $Py)) { $Py = "python" }
$Latest = Get-Content (Join-Path $Reel "brand\artifacts\overnight_gpu_rl_arch_latest.json") -Raw | ConvertFrom-Json
$Hist = Join-Path $Latest.run_dir "history.jsonl"
$Base = [string]$Latest.baseline_dual_cosine
# Learning curves: x-axis capped at 5000 (5k clean gate), even if overnight continues past 8k+.
$FreezeIter = 5000
& $Py (Join-Path $Reel "scripts\plot_overnight_history.py") $Hist --baseline $Base --max-iter $FreezeIter
Write-Host "OK plots regenerated max_iter=$FreezeIter (live latest iter=$($Latest.iter) champ=$($Latest.champion_residual))"
