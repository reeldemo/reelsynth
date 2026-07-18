$Repo = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
$Art = Join-Path $Repo "brand\artifacts"
$Latest = Join-Path $Art "overnight_gpu_rl_arch_latest.json"
$Done = Join-Path $Art "overnight_gpu_DONE.flag"
while ($true) {
  Start-Sleep -Seconds 300
  $iter = 0
  if (Test-Path $Latest) {
    try { $iter = [int]((Get-Content $Latest -Raw | ConvertFrom-Json).iter) } catch {}
  }
  if ($iter -ge 250000 -or (Test-Path $Done)) {
    $payload = (@{ prompt = "1M COMPLETE — generate final publication plots, run Klaut paper v4 plan/write/revise/export with honest numbers, commit+push both repos."; iter = $iter } | ConvertTo-Json -Compress)
    Write-Output ("AGENT_LOOP_WAKE_denoise1m_DONE " + $payload)
    break
  }
}
