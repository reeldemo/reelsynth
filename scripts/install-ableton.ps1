#Requires -Version 5.1
<#
.SYNOPSIS
  Build (optional) and install ReelSynth for Ableton Live on Windows.

.DESCRIPTION
  - Builds release VST3 + external editor (unless -SkipBuild)
  - Installs VST3 bundle to Common Files\VST3 (needs Admin) or a user folder
  - Installs editor to %LOCALAPPDATA%\ReelSynth\bin
  - Writes config.json with auto_editor=true so Live launches the full UI

  Ableton on Linux is not supported. macOS: use scripts/install-ableton.sh

.EXAMPLE
  # From repo root, elevated PowerShell recommended for system VST3:
  .\scripts\install-ableton.ps1
#>
param(
    [switch]$SkipBuild,
    [switch]$UserVst3,
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

$ErrorActionPreference = "Stop"
Set-Location $RepoRoot

Write-Host "ReelSynth Ableton installer (Windows)" -ForegroundColor Cyan
Write-Host "Repo: $RepoRoot"

if (-not $SkipBuild) {
    Write-Host "Building release plugin + editor..."
    cargo build -p reelsynth-plugin --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$dll = Join-Path $RepoRoot "target\release\reelsynth_plugin.dll"
$editorSrc = Join-Path $RepoRoot "target\release\reelsynth-plugin-editor.exe"
if (-not (Test-Path $dll)) { throw "Missing $dll - build first" }
if (-not (Test-Path $editorSrc)) { throw "Missing $editorSrc - build first" }

# --- Editor (no admin) ---
$binDir = Join-Path $env:LOCALAPPDATA "ReelSynth\bin"
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
$editorDst = Join-Path $binDir "reelsynth-plugin-editor.exe"
Copy-Item $editorSrc $editorDst -Force
Write-Host "Editor -> $editorDst"

# --- Config (auto-open editor when VST loads) ---
$cfgDir = Join-Path $env:LOCALAPPDATA "ReelSynth"
New-Item -ItemType Directory -Force -Path $cfgDir | Out-Null
$cfgPath = Join-Path $cfgDir "config.json"
$cfg = @{
    schema      = "reelsynth-ableton-config-v1"
    auto_editor = $true
    editor_path = $editorDst.Replace('\', '/')
} | ConvertTo-Json
Set-Content -Path $cfgPath -Value $cfg -Encoding UTF8
Write-Host "Config -> $cfgPath (auto_editor=true)"

# --- VST3 bundle ---
if ($UserVst3) {
    $vstRoot = Join-Path $env:USERPROFILE "Documents\VST3"
} else {
    $vstRoot = Join-Path ${env:CommonProgramFiles} "VST3"
}

$bundle = Join-Path $vstRoot "ReelSynth.vst3\Contents\x86_64-win"
try {
    New-Item -ItemType Directory -Force -Path $bundle | Out-Null
    Copy-Item $dll (Join-Path $bundle "ReelSynth.vst3") -Force
    Write-Host "VST3  -> $(Join-Path $vstRoot 'ReelSynth.vst3')"
} catch {
    Write-Warning "Could not write system VST3 folder (need Admin?). Retry as Admin, or: .\scripts\install-ableton.ps1 -UserVst3"
    Write-Warning $_.Exception.Message
    if (-not $UserVst3) {
        Write-Host "Falling back to user Documents\VST3..."
        $vstRoot = Join-Path $env:USERPROFILE "Documents\VST3"
        $bundle = Join-Path $vstRoot "ReelSynth.vst3\Contents\x86_64-win"
        New-Item -ItemType Directory -Force -Path $bundle | Out-Null
        Copy-Item $dll (Join-Path $bundle "ReelSynth.vst3") -Force
        Write-Host "VST3  -> $(Join-Path $vstRoot 'ReelSynth.vst3')"
        Write-Host "In Live: Preferences → Plug-ins → add folder: $vstRoot → Rescan" -ForegroundColor Yellow
    }
}

Write-Host ""
Write-Host "Done. In Ableton Live:" -ForegroundColor Green
Write-Host "  1. Preferences -> Plug-ins -> enable VST3 system folders (and user folder if used)"
Write-Host "  2. Rescan"
Write-Host "  3. Load ReelSynth on a MIDI track - the full editor should open automatically"
Write-Host "  Manual editor: $editorDst"
