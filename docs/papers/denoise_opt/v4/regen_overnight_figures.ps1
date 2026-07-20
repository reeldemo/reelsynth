# One-command plot redo when overnight gate advances (run from anywhere)
$ErrorActionPreference = "Stop"
$Reel = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
$Py = Join-Path $Reel ".venv_gpu\Scripts\python.exe"
$Latest = Get-Content (Join-Path $Reel "brand\artifacts\overnight_gpu_rl_arch_latest.json") -Raw | ConvertFrom-Json
$Hist = Join-Path $Latest.run_dir "history.jsonl"
$Base = [string]$Latest.baseline_dual_cosine
& $Py (Join-Path $Reel "scripts\plot_overnight_history.py") $Hist --baseline $Base
Write-Host "OK plots regenerated from iter=$($Latest.iter) champ=$($Latest.champion_residual)"
