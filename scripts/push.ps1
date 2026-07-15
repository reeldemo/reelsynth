param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$GitArgs
)

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
& "$scriptDir\git-with-token.ps1" git push @GitArgs
exit $LASTEXITCODE
