$sharedHelper = "C:\Users\Julian\Documents\Programming\reeldemo.io\scripts\git-with-token.ps1"
if (-not (Test-Path -LiteralPath $sharedHelper)) {
    throw "Shared helper not found: $sharedHelper"
}

& $sharedHelper @args
exit $LASTEXITCODE
