[CmdletBinding()]
param(
    [switch]$SkipRust
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Invoke-NativeChecked {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Executable,

        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,

        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE."
    }
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root

try {
    $pnpm = (Get-Command pnpm.cmd -ErrorAction Stop).Source

    Write-Host "Installing locked frontend dependencies..."
    Invoke-NativeChecked $pnpm @("install", "--frozen-lockfile") "Locked dependency installation"

    Write-Host "Running frontend verification gates..."
    Invoke-NativeChecked $pnpm @("check") "Frontend checks"
    Invoke-NativeChecked $pnpm @("test:coverage") "Frontend coverage"
    Invoke-NativeChecked $pnpm @("audit", "--audit-level", "high", "--prod") "Frontend dependency audit"
    Invoke-NativeChecked $pnpm @("license:frontend") "Frontend license policy"
    Invoke-NativeChecked $pnpm @("license:rust") "Rust license policy"
    Invoke-NativeChecked $pnpm @("sbom:frontend") "Frontend SBOM generation"
    Invoke-NativeChecked $pnpm @("sbom:rust") "Rust SBOM generation"

    if (-not $SkipRust) {
        $cargo = Get-Command cargo -ErrorAction SilentlyContinue
        if ($null -eq $cargo) {
            $userCargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
            if (Test-Path -LiteralPath $userCargo) {
                $cargoPath = $userCargo
            } else {
                throw "Cargo was not found. Install the stable Rust toolchain or run with -SkipRust for frontend-only verification."
            }
        } else {
            $cargoPath = $cargo.Source
        }

        Write-Host "Running Rust verification gates..."
        Invoke-NativeChecked $cargoPath @("fmt", "--manifest-path", "src-tauri/Cargo.toml", "--all", "--", "--check") "Rust formatting"
        Invoke-NativeChecked $cargoPath @("check", "--manifest-path", "src-tauri/Cargo.toml", "--workspace", "--all-targets", "--all-features", "--locked") "Rust check"
        Invoke-NativeChecked $cargoPath @("clippy", "--manifest-path", "src-tauri/Cargo.toml", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings") "Rust clippy"
        Invoke-NativeChecked $cargoPath @("test", "--manifest-path", "src-tauri/Cargo.toml", "--workspace", "--all-targets", "--all-features", "--locked") "Rust tests"
        Invoke-NativeChecked $cargoPath @("audit", "--file", "src-tauri/Cargo.lock") "Rust dependency audit"
    }

    Write-Host "Verification completed successfully."
} finally {
    Pop-Location
}
