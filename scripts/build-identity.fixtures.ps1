[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$module = Join-Path $PSScriptRoot "Aster.BuildIdentity.psm1"
Import-Module $module -Force

$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$sandbox = [System.IO.Path]::GetFullPath(
    (Join-Path $temporaryRoot ("aster-build-identity-" + [guid]::NewGuid().ToString("N")))
)
$temporaryPrefix = $temporaryRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar
if (-not $sandbox.StartsWith($temporaryPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The build-identity fixture sandbox escaped the system temporary directory."
}
$fixtureJunction = $null

try {
    $dist = Join-Path $sandbox "dist"
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    $artifact = Join-Path $sandbox "artifact.bin"
    $asset = Join-Path $dist "asset.js"
    [System.IO.File]::WriteAllBytes($artifact, [byte[]](1, 2, 3, 4))
    [System.IO.File]::WriteAllText($asset, "safe fixture", [System.Text.UTF8Encoding]::new($false))

    $identity = Get-AsterFileIdentity -RepositoryRoot $sandbox -Path $artifact
    if (
        $identity.path -cne "artifact.bin" -or
        $identity.bytes -ne 4 -or
        $identity.sha256 -notmatch '^[0-9a-f]{64}$'
    ) {
        throw "The file-identity positive fixture failed."
    }

    $firstDigest = Get-AsterDirectoryDigest -RepositoryRoot $sandbox -Path $dist
    [System.IO.File]::AppendAllText($asset, " changed", [System.Text.UTF8Encoding]::new($false))
    $secondDigest = Get-AsterDirectoryDigest -RepositoryRoot $sandbox -Path $dist
    if ($firstDigest -eq $secondDigest) {
        throw "The directory digest did not detect a changed frontend asset."
    }

    $cultureDirectory = Join-Path $sandbox "culture-order"
    New-AsterSafeDirectory -RepositoryRoot $sandbox -Path $cultureDirectory | Out-Null
    $cultureNames = @(
        "I.txt",
        "i.txt",
        ([char]0x0130).ToString() + ".txt",
        ([char]0x0131).ToString() + ".txt",
        ([char]0x00e4).ToString() + ".txt",
        "z.txt"
    )
    foreach ($name in $cultureNames) {
        [System.IO.File]::WriteAllText(
            (Join-Path $cultureDirectory $name),
            "ordinal fixture $name",
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    $originalCulture = [System.Threading.Thread]::CurrentThread.CurrentCulture
    $originalUiCulture = [System.Threading.Thread]::CurrentThread.CurrentUICulture
    try {
        [System.Threading.Thread]::CurrentThread.CurrentCulture = [System.Globalization.CultureInfo]::GetCultureInfo("en-US")
        [System.Threading.Thread]::CurrentThread.CurrentUICulture = [System.Globalization.CultureInfo]::GetCultureInfo("en-US")
        $englishDigest = Get-AsterDirectoryDigest -RepositoryRoot $sandbox -Path $cultureDirectory
        [System.Threading.Thread]::CurrentThread.CurrentCulture = [System.Globalization.CultureInfo]::GetCultureInfo("tr-TR")
        [System.Threading.Thread]::CurrentThread.CurrentUICulture = [System.Globalization.CultureInfo]::GetCultureInfo("tr-TR")
        $turkishDigest = Get-AsterDirectoryDigest -RepositoryRoot $sandbox -Path $cultureDirectory
    }
    finally {
        [System.Threading.Thread]::CurrentThread.CurrentCulture = $originalCulture
        [System.Threading.Thread]::CurrentThread.CurrentUICulture = $originalUiCulture
    }
    if ($englishDigest -cne $turkishDigest) {
        throw "The directory digest depends on the current culture instead of ordinal path order."
    }

    if ($env:OS -eq "Windows_NT") {
        $junctionTarget = Join-Path $sandbox "junction-target"
        New-AsterSafeDirectory -RepositoryRoot $sandbox -Path $junctionTarget | Out-Null
        $targetMarker = Join-Path $junctionTarget "must-survive.txt"
        [System.IO.File]::WriteAllText($targetMarker, "junction target", [System.Text.UTF8Encoding]::new($false))
        $fixtureJunction = Join-Path $dist "linked-assets"
        New-Item -ItemType Junction -Path $fixtureJunction -Target $junctionTarget | Out-Null

        $junctionRejected = $false
        try {
            [void](Get-AsterDirectoryDigest -RepositoryRoot $sandbox -Path $dist)
        }
        catch {
            if ($_.Exception.Message -match "Reparse points and junctions are prohibited") {
                $junctionRejected = $true
            }
            else {
                throw
            }
        }
        if (-not $junctionRejected) {
            throw "The build-identity directory walk accepted a Windows junction."
        }
    }

    $escaped = $false
    try {
        [void](Get-AsterFileIdentity -RepositoryRoot $sandbox -Path $module)
    }
    catch {
        $escaped = $true
    }
    if (-not $escaped) {
        throw "The build-identity path boundary accepted a file outside the repository root."
    }

    Write-Host "Build-identity fixtures passed."
}
finally {
    if ($null -ne $fixtureJunction -and (Test-Path -LiteralPath $fixtureJunction)) {
        $junctionItem = Get-Item -LiteralPath $fixtureJunction -Force
        if (($junctionItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0) {
            throw "Fixture cleanup refused to remove a path that was no longer the expected junction."
        }
        [System.IO.Directory]::Delete($fixtureJunction, $false)
        $fixtureJunction = $null
    }
    $resolvedSandbox = [System.IO.Path]::GetFullPath($sandbox)
    if (
        $resolvedSandbox.StartsWith($temporaryPrefix, [StringComparison]::OrdinalIgnoreCase) -and
        -not [string]::Equals($resolvedSandbox, $temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
        (Test-Path -LiteralPath $resolvedSandbox)
    ) {
        Remove-AsterSafeDirectoryTree -RepositoryRoot $temporaryRoot -Path $resolvedSandbox
    }
}
