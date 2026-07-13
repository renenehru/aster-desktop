[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {
    $pnpm = (Get-Command pnpm.cmd -ErrorAction Stop).Source

    & $pnpm install --frozen-lockfile
    if ($LASTEXITCODE -ne 0) {
        throw "Locked dependency installation failed with exit code $LASTEXITCODE."
    }

    & $pnpm desktop:dev
    if ($LASTEXITCODE -ne 0) {
        throw "Desktop development process failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
