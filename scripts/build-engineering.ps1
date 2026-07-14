[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$identityModule = Join-Path $PSScriptRoot "Aster.BuildIdentity.psm1"
Import-Module $identityModule -Force
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
[void](Assert-AsterNoReparseAncestors -Path $root)
$git = (Get-Command git.exe -ErrorAction Stop).Source
$topLevel = (& $git -C $root rev-parse --show-toplevel).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($topLevel)) {
    throw "Engineering builds require an initialized Git repository."
}
$safeTopLevel = [System.IO.Path]::GetFullPath($topLevel)
[void](Assert-AsterNoReparseAncestors -Path $safeTopLevel)
if (-not [string]::Equals($safeTopLevel, $root, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The engineering build must run from the repository that contains this source tree."
}
$workingTreeStatus = @(& $git -C $root status --porcelain=v1 --untracked-files=normal)
if ($LASTEXITCODE -ne 0 -or $workingTreeStatus.Count -ne 0) {
    throw "Engineering builds require a completely clean Git working tree."
}
$sourceRevision = (& $git -C $root rev-parse --verify HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceRevision -notmatch '^[0-9a-fA-F]{40,64}$') {
    throw "The engineering-build source revision could not be identified."
}
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

    $postBuildRevision = (& $git -C $root rev-parse --verify HEAD).Trim()
    $postBuildStatus = @(& $git -C $root status --porcelain=v1 --untracked-files=normal)
    if (
        $LASTEXITCODE -ne 0 -or
        $postBuildRevision -ne $sourceRevision -or
        $postBuildStatus.Count -ne 0
    ) {
        throw "The tracked source changed during the engineering build; no build identity was issued."
    }

    $releaseBinary = Join-Path $root "src-tauri\target\release\aster-desktop.exe"
    $installer = Join-Path $root "src-tauri\target\release\bundle\nsis\Aster_0.2.0_x64-setup.exe"
    $frontendDist = Join-Path $root "dist"
    $frontendSbom = Join-Path $root "work\sbom-frontend.cdx.json"
    $rustSbom = Join-Path $root "work\sbom-rust.cdx.json"
    $manifest = [ordered]@{
        schemaVersion = 1
        sourceRevision = $sourceRevision.ToLowerInvariant()
        generatedUtc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
        artifacts = [ordered]@{
            releaseBinary = Get-AsterFileIdentity -RepositoryRoot $root -Path $releaseBinary
            installer = Get-AsterFileIdentity -RepositoryRoot $root -Path $installer
        }
        sboms = [ordered]@{
            frontend = Get-AsterFileIdentity -RepositoryRoot $root -Path $frontendSbom
            rust = Get-AsterFileIdentity -RepositoryRoot $root -Path $rustSbom
        }
        frontendDist = [ordered]@{
            path = "dist"
            sha256 = Get-AsterDirectoryDigest -RepositoryRoot $root -Path $frontendDist
        }
    }
    $work = Join-Path $root "work"
    New-AsterSafeDirectory -RepositoryRoot $root -Path $work | Out-Null
    $manifestPath = Join-Path $work "build-identity.json"
    Write-AsterUtf8Text `
        -RepositoryRoot $root `
        -Path $manifestPath `
        -Content ($manifest | ConvertTo-Json -Depth 6) `
        -AllowReplace | Out-Null
    Write-Host "Engineering build identity written to $manifestPath for source $sourceRevision."
} finally {
    Pop-Location
}
