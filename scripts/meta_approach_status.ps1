# Durable meta-approach status poll (survives Cursor crashes; reads checkpoints).
# Usage:
#   powershell -File scripts/meta_approach_status.ps1
#   powershell -File scripts/meta_approach_status.ps1 -Watch 30
#   powershell -File scripts/meta_approach_status.ps1 -Json
param(
  [double]$Watch = 0,
  [switch]$Json
)
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $Root "scripts\meta_approach_status.py"))) {
  $Root = "C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
}
$Py = Join-Path $Root ".venv_gpu\Scripts\python.exe"
if (-not (Test-Path $Py)) { $Py = "python" }
$args = @((Join-Path $Root "scripts\meta_approach_status.py"))
if ($Json) { $args += "--json" }
if ($Watch -gt 0) { $args += @("--watch", "$Watch") }
& $Py @args
