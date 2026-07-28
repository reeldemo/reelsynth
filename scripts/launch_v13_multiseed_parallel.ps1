# DEPRECATED: dual-seed GPU runs leak VRAM on this machine.
# Use sequential resume instead:
#   powershell -File scripts\launch_v13_multiseed_search.ps1
param(
    [int]$MaxParallel = 2
)
Write-Warning "DEPRECATED: use launch_v13_multiseed_search.ps1 (one seed at a time) to avoid GPU memory leaks."
exit 1
