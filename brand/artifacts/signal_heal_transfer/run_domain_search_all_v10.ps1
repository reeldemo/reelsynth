# Paper v10.1 transfer-domain hybrid GA-PPO search (R_blend + J).
# Sequential per dataset; 250 iters matches experiments_transfer.tex pilot budget.
$ErrorActionPreference = "Continue"
$Root = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
$Art = Join-Path $Root "brand\artifacts\signal_heal_transfer"
$Py = Join-Path $Root ".venv_gpu\Scripts\python.exe"
if (-not (Test-Path $Py)) { $Py = "python" }
$Log = Join-Path $Art "domain_search_all_v10_launch.log"
$Status = Join-Path $Art "domain_search_STATUS.json"

Set-Location $Root
function Write-Status($phase, $detail) {
  $obj = [ordered]@{
    updated_at = (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ")
    phase      = $phase
    detail     = $detail
    protocol   = "paper_v10.1"
    metric     = "R_blend+J"
  }
  ($obj | ConvertTo-Json -Compress) | Set-Content -Path $Status -Encoding utf8
}

"=== $(Get-Date -Format o) START v10.1 transfer search ===" | Tee-Object -FilePath $Log

# Drop MFPT/PTB caches so n=256 / 500Hz rebuild is guaranteed
Remove-Item (Join-Path $Art "cache\mfpt_bearings.pt") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $Art "cache\mfpt_bearings_meta.json") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $Art "cache\ptbxl_ecg.pt") -Force -ErrorAction SilentlyContinue
Remove-Item (Join-Path $Art "cache\ptbxl_ecg_meta.json") -Force -ErrorAction SilentlyContinue

$datasets = @(
  "cwru_bearings",
  "mfpt_bearings",
  "mitbih_ecg",
  "ptbxl_ecg",
  "synth_cnc_g01",
  "synth_pmu_cycle"
)
$Iters = 250
$Pop = 12

foreach ($ds in $datasets) {
  $msg = "hybrid GA-PPO search $ds iters=$Iters pop=$Pop protocol=v10.1"
  "=== $(Get-Date -Format o) START $msg ===" | Tee-Object -FilePath $Log -Append
  Write-Status "search" $msg
  $force = @("--force-rebuild")
  # Wipe per-domain prior DualCosine-era search artifacts
  $domDir = Join-Path $Art $ds
  if (Test-Path $domDir) {
    Remove-Item (Join-Path $domDir "hybrid_lstm") -Recurse -Force -ErrorAction SilentlyContinue
  }
  & $Py scripts\bench_signal_heal_transfer.py `
    --datasets $ds `
    --iters $Iters `
    --fit-steps 40 `
    --batch 48 `
    --pop-size $Pop `
    --n-periods 256 `
    --merge-existing `
    --device cuda `
    @force 2>&1 | Tee-Object -FilePath $Log -Append
  "=== $(Get-Date -Format o) DONE $ds exit=$LASTEXITCODE ===" | Tee-Object -FilePath $Log -Append
}

# Latency microbench + plots
"=== $(Get-Date -Format o) START latency microbench ===" | Tee-Object -FilePath $Log -Append
Write-Status "latency" "bench_signal_heal_latency"
& $Py scripts\bench_signal_heal_latency.py 2>&1 | Tee-Object -FilePath $Log -Append

Write-Status "all_complete" ("iters=$Iters datasets=" + ($datasets -join ",") + " protocol=paper_v10.1")
"=== $(Get-Date -Format o) ALL DOMAIN SEARCHES COMPLETE ===" | Tee-Object -FilePath $Log -Append
