[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
    throw "Visual Studio Installer could not be located. Install the Visual C++ x64 build tools."
}

$installationPath = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath
if ([string]::IsNullOrWhiteSpace($installationPath)) {
    throw "A Visual Studio installation with the Visual C++ x64 tools was not found."
}

$developerPrompt = Join-Path $installationPath "Common7\Tools\VsDevCmd.bat"
$cargoRegistry = Join-Path $env:USERPROFILE ".cargo\registry\src"
$cargoCheckouts = Join-Path $env:USERPROFILE ".cargo\git\checkouts"
$unitSeparator = [char]0x1f
$remapFlags = @(
    "--remap-path-prefix=$root=aster-source",
    "--remap-path-prefix=$cargoRegistry=cargo-registry",
    "--remap-path-prefix=$cargoCheckouts=cargo-checkouts",
    "--remap-path-prefix=$env:USERPROFILE=user-home"
)
$env:CARGO_ENCODED_RUSTFLAGS = $remapFlags -join $unitSeparator

# AWS-LC compiles C sources whose diagnostic strings otherwise retain absolute
# source locations. Keep those local build paths out of the packaged binary as
# well as the Rust paths above. The crate-scoped variables avoid changing flags
# for unrelated native dependencies.
$nativeRemapFlags = @(
    "/experimental:deterministic",
    "`"/pathmap:$cargoRegistry=cargo-registry`"",
    "`"/pathmap:$cargoCheckouts=cargo-checkouts`"",
    "`"/pathmap:$root=aster-source`"",
    "`"/pathmap:$env:USERPROFILE=user-home`""
)
$nativeRemapValue = $nativeRemapFlags -join " "
# cc-rs otherwise splits environment flags on every space, including spaces
# inside a quoted Windows path.
$env:CC_SHELL_ESCAPED_FLAGS = "1"
$env:AWS_LC_SYS_CFLAGS = $nativeRemapValue
$env:AWS_LC_SYS_CXXFLAGS = $nativeRemapValue

$pnpm = (Get-Command pnpm.cmd -ErrorAction Stop).Source
$command = "call `"$developerPrompt`" -arch=x64 -host_arch=x64 && `"$pnpm`" desktop:build"
Push-Location $root
try {
    & cmd.exe /d /s /c $command
    if ($LASTEXITCODE -ne 0) {
        throw "The engineering build failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
