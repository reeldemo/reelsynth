# Stage Windows payload and build NSIS setup.exe.
# Usage: .\scripts\package-windows.ps1 [-Version VER] [-TargetDir DIR] [-OutDir DIR]
param(
    [string]$Version = "",
    [string]$TargetDir = "",
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $Root

if (-not $Version) {
    $Version = (Select-String -Path (Join-Path $Root "Cargo.toml") -Pattern '^version = "([^"]+)"' |
        Select-Object -First 1).Matches.Groups[1].Value
}
if (-not $TargetDir) { $TargetDir = Join-Path $Root "target\release" }
if (-not $OutDir) { $OutDir = Join-Path $Root "dist" }

$App = Join-Path $TargetDir "reelsynth-app.exe"
$Export = Join-Path $TargetDir "reelsynth-export.exe"
$Editor = Join-Path $TargetDir "reelsynth-plugin-editor.exe"
$Dll = Join-Path $TargetDir "reelsynth_plugin.dll"

foreach ($f in @($App, $Export, $Editor, $Dll)) {
    if (-not (Test-Path $f)) {
        throw "Missing $f — build with: cargo build --release -p reelsynth-app -p reelsynth --bin reelsynth-export -p reelsynth-plugin"
    }
}

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$Stage = Join-Path $OutDir "nsis-stage-windows-x86_64"
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

Copy-Item $App (Join-Path $Stage "ReelSynth.exe")
Copy-Item $Export (Join-Path $Stage "reelsynth-export.exe")
Copy-Item $Editor (Join-Path $Stage "reelsynth-plugin-editor.exe")
Copy-Item $Dll (Join-Path $Stage "reelsynth_plugin.dll")
Copy-Item (Join-Path $Root "LICENSE") $Stage -ErrorAction SilentlyContinue
Copy-Item (Join-Path $Root "README.md") $Stage -ErrorAction SilentlyContinue

$OutExe = Join-Path $OutDir "reelsynth-$Version-windows-x86_64-setup.exe"
$Nsi = Join-Path $Root "installer\windows\reelsynth.nsi"

$makensis = Get-Command makensis -ErrorAction SilentlyContinue
if (-not $makensis) {
    $candidates = @(
        "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
        "${env:ProgramFiles}\NSIS\makensis.exe"
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $makensis = Get-Item $c; break }
    }
}
if (-not $makensis) {
    throw "makensis not found. Install NSIS (e.g. choco install nsis) on the Windows builder."
}

$makensisPath = if ($makensis.Source) { $makensis.Source } else { $makensis.FullName }
$stageAbs = (Resolve-Path $Stage).Path
$outAbs = $OutExe

& $makensisPath `
    "/DREELSYNTH_VERSION=$Version" `
    "/DREELSYNTH_STAGE=$stageAbs" `
    "/DREELSYNTH_OUT=$outAbs" `
    $Nsi

if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit $LASTEXITCODE" }
Write-Host "Created $OutExe"
