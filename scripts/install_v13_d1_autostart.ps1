# Install Startup-folder shortcuts so D1 resumes after reboot (checkpoint-safe).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Startup = [Environment]::GetFolderPath("Startup")
$Launch = Join-Path $Root "scripts\launch_v13_multiseed_search.ps1"
$Watch = Join-Path $Root "scripts\watchdog_v13_d1.ps1"

function New-PsShortcut([string]$name, [string]$script) {
    $lnkPath = Join-Path $Startup $name
    $w = New-Object -ComObject WScript.Shell
    $s = $w.CreateShortcut($lnkPath)
    $s.TargetPath = "powershell.exe"
    $s.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`""
    $s.WorkingDirectory = $Root
    $s.WindowStyle = 7
    $s.Save()
    Write-Host "Installed $lnkPath"
}

New-PsShortcut "ReelSynth_v13_D1_Launch.lnk" $Launch
New-PsShortcut "ReelSynth_v13_D1_Watchdog.lnk" $Watch
Write-Host ""
Write-Host "After reboot these start automatically and RESUME from checkpoints."
Write-Host "To uninstall: delete the .lnk files from $Startup"
Write-Host "Status anytime: powershell -File scripts\status_v13_d1.ps1"
