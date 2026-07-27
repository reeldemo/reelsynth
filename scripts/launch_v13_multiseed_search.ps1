# Durable v13 D1 launcher: matched-5k outer-loop re-search under R_blend, 3 seeds.
# Approach order prioritizes hybrid + random (learning curves) before slow reinforce.
# Resume-safe (bench_meta_approaches_5k checkpoints). Logs per seed.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

$Seeds = @(1902771841, 2026072701, 2026072702)
$Iters = 5000
# hybrid + random first for paper learning curves; reinforce last (~26h alone historically)
$Approaches = "hybrid_lstm,random,cmaes,tpe,aging_evo,reinforce"
$BaseOut = Join-Path $Root "brand\artifacts\meta_approach_compare_v13_rblend"
New-Item -ItemType Directory -Force -Path $BaseOut | Out-Null

$MasterLog = Join-Path $BaseOut "launcher.log"
function Log([string]$msg) {
    $line = "[{0}] {1}" -f (Get-Date -Format "yyyy-MM-ddTHH:mm:ss"), $msg
    Add-Content -Path $MasterLog -Value $line
    Write-Host $line
}

Log "START v13 multiseed search iters=$Iters seeds=$($Seeds -join ',') approaches=$Approaches"

$Py = Join-Path $Root ".venv_gpu\Scripts\python.exe"
if (-not (Test-Path $Py)) { throw "missing GPU venv python: $Py" }

foreach ($Seed in $Seeds) {
    $OutDir = Join-Path $BaseOut "$Seed"
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
    $SeedLog = Join-Path $OutDir "run.log"
    Log "SEED $Seed begin out=$OutDir"
    $pyArgs = @(
        "scripts\bench_meta_approaches_5k.py",
        "--iters", "$Iters",
        "--seed", "$Seed",
        "--out-dir", "$OutDir",
        "--device", "cuda",
        "--approaches", $Approaches
    )
    & $Py @pyArgs 2>&1 | Tee-Object -FilePath $SeedLog -Append
    $code = $LASTEXITCODE
    Log "SEED $Seed exit=$code"
    if ($code -ne 0) {
        Log "SEED $Seed FAILED - continuing to next seed"
    }
}

Log "ALL SEEDS DONE"
exit 0
