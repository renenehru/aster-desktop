[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$identityModule = Join-Path $PSScriptRoot "Aster.BuildIdentity.psm1"
Import-Module $identityModule -Force
$git = (Get-Command git.exe -ErrorAction Stop).Source
$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$sandbox = Join-Path $temporaryRoot ("aster-package-provenance-" + [guid]::NewGuid().ToString("N"))
$temporaryPrefix = $temporaryRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar
if (-not $sandbox.StartsWith($temporaryPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The package-provenance fixture sandbox escaped the system temporary directory."
}
$fixtureJunctions = [System.Collections.Generic.List[string]]::new()

$originalModulePath = $env:PSModulePath
$fallbackModule = $null
try {
    $env:PSModulePath = Join-Path $temporaryRoot ("aster-empty-module-path-" + [guid]::NewGuid().ToString("N"))
    $fallbackModule = Import-Module (Join-Path $PSScriptRoot "Aster.Packaging.psm1") -Force -PassThru
    $repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
    $observation = & $fallbackModule {
        param($Root, $Artifact)
        Get-AsterAuthenticodeObservation -RepositoryRoot $Root -Path $Artifact
    } $repositoryRoot $PSCommandPath
    if ($observation -cne "NOT RUN (verifier unavailable)") {
        throw "The unavailable Authenticode verifier fixture did not return the canonical NOT RUN observation."
    }
}
finally {
    $env:PSModulePath = $originalModulePath
    if ($null -ne $fallbackModule) {
        Remove-Module $fallbackModule -Force
    }
    Import-Module $identityModule -Force
}

function Write-FixtureText {
    param([string]$Path, [string]$Content)
    $parent = [System.IO.Directory]::GetParent($Path).FullName
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    [System.IO.File]::WriteAllText($Path, $Content, [System.Text.UTF8Encoding]::new($false))
}

function Invoke-FixtureGit {
    param([string]$Repository, [string[]]$Arguments)
    & $git -C $Repository @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Fixture Git command failed: git $($Arguments -join ' ')"
    }
}

function Get-FixtureEvidence {
    param([string]$Revision)
    $evidenceLogRelative = "work/evidence/2026-07-13-$Revision-engineering-build.log"
    return @"
# Verification Record: fixture engineering build

**Source revision:** $Revision

**Artifact:** Not applicable - fixture-only package inputs

**Environment:** Windows fixture sandbox; no production artifact

**Started UTC:** 2026-07-13T00:00:00Z

**Completed UTC:** 2026-07-13T00:01:00Z

**Procedure identity (self-declared):** package-provenance-fixture

**Overall classification:** Unsigned engineering build for local evaluation

| Criterion/gate | Outcome | Source revision/state | Environment | Started/completed UTC | Procedure identity | Exact command or procedure | Evidence | Artifact/hash | Scope/notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| ``AC-031`` | ``PASS`` | $Revision clean | Windows fixture sandbox; no production artifact | 2026-07-13T00:00:00Z to 2026-07-13T00:01:00Z | package-provenance-fixture | ``powershell -File scripts/package-provenance.fixtures.ps1`` | $evidenceLogRelative | Not applicable - synthetic inputs | Packaging-policy scope only |

## Failures and retained retries

None in this fixture record.

## Residual risk and exceptions

This record is synthetic and is never release evidence.
"@
}

function New-FixtureRepository {
    param([string]$Name)

    $repository = Join-Path $sandbox $Name
    New-Item -ItemType Directory -Path $repository | Out-Null
    foreach ($directory in @(
            "assets",
            "scripts",
            "dist",
            "work\evidence",
            "src-tauri\target\release\bundle\nsis"
        )) {
        New-Item -ItemType Directory -Path (Join-Path $repository $directory) -Force | Out-Null
    }
    Write-FixtureText (Join-Path $repository ".gitignore") @"
/dist/
/work/
/outputs/
/src-tauri/target/
"@
    Write-FixtureText (Join-Path $repository "LICENSE") "fixture license`n"
    Write-FixtureText (Join-Path $repository "NOTICE") "fixture notice`n"
    [System.IO.File]::WriteAllBytes((Join-Path $repository "assets\Aster-MVP-v2-preview.png"), [byte[]](1, 2, 3, 4, 5))
    foreach ($scriptName in @(
            "Aster.BuildIdentity.psm1",
            "Aster.Packaging.psm1",
            "package-engineering.ps1",
            "package-audit.ps1",
            "secret-patterns.json"
        )) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $scriptName) -Destination (Join-Path $repository "scripts\$scriptName")
    }
    Write-FixtureText (Join-Path $repository "dist\app.js") "console.log('safe fixture');`n"

    & $git init -q $repository
    if ($LASTEXITCODE -ne 0) {
        throw "The fixture Git repository could not be initialized."
    }
    Invoke-FixtureGit $repository @("config", "user.name", "Aster Fixture")
    Invoke-FixtureGit $repository @("config", "user.email", "fixture@example.invalid")
    Invoke-FixtureGit $repository @("config", "core.autocrlf", "false")
    Invoke-FixtureGit $repository @("add", ".gitignore", "LICENSE", "NOTICE", "assets", "scripts")
    Invoke-FixtureGit $repository @("commit", "-q", "-m", "fixture source")
    $revision = (& $git -C $repository rev-parse HEAD).Trim().ToLowerInvariant()

    $releaseBinary = Join-Path $repository "src-tauri\target\release\aster-desktop.exe"
    $installer = Join-Path $repository "src-tauri\target\release\bundle\nsis\Aster_0.2.0_x64-setup.exe"
    $frontendSbom = Join-Path $repository "work\sbom-frontend.cdx.json"
    $rustSbom = Join-Path $repository "work\sbom-rust.cdx.json"
    Write-FixtureText $releaseBinary "fixture release binary"
    Write-FixtureText $installer "fixture installer"
    Write-FixtureText $frontendSbom '{"bomFormat":"CycloneDX","fixture":"frontend"}'
    Write-FixtureText $rustSbom '{"bomFormat":"CycloneDX","fixture":"rust"}'
    $manifest = [ordered]@{
        schemaVersion = 1
        sourceRevision = $revision
        generatedUtc = "2026-07-13T00:00:00Z"
        artifacts = [ordered]@{
            releaseBinary = Get-AsterFileIdentity -RepositoryRoot $repository -Path $releaseBinary
            installer = Get-AsterFileIdentity -RepositoryRoot $repository -Path $installer
        }
        sboms = [ordered]@{
            frontend = Get-AsterFileIdentity -RepositoryRoot $repository -Path $frontendSbom
            rust = Get-AsterFileIdentity -RepositoryRoot $repository -Path $rustSbom
        }
        frontendDist = [ordered]@{
            path = "dist"
            sha256 = Get-AsterDirectoryDigest -RepositoryRoot $repository -Path (Join-Path $repository "dist")
        }
    }
    Write-FixtureText (Join-Path $repository "work\build-identity.json") ($manifest | ConvertTo-Json -Depth 6)
    $evidenceRelative = "work/evidence/2026-07-13-$revision-engineering-build.md"
    $evidenceLogRelative = [System.IO.Path]::ChangeExtension($evidenceRelative, ".log")
    Write-FixtureText (Join-Path $repository $evidenceRelative) (Get-FixtureEvidence $revision)
    Write-FixtureText (Join-Path $repository $evidenceLogRelative) @"
Fixture command: powershell -File scripts/package-provenance.fixtures.ps1
Fixture result: PASS
Scope: synthetic packaging-policy inputs only
"@
    return [pscustomobject]@{
        Root = $repository
        Revision = $revision
        EvidenceRelative = $evidenceRelative
        EvidencePath = Join-Path $repository $evidenceRelative
        EvidenceLogRelative = $evidenceLogRelative
        EvidenceLogPath = Join-Path $repository $evidenceLogRelative
        ManifestPath = Join-Path $repository "work\build-identity.json"
        ReleaseBinary = $releaseBinary
        Installer = $installer
        FrontendSbom = $frontendSbom
        RustSbom = $rustSbom
        PackageScript = Join-Path $repository "scripts\package-engineering.ps1"
        PackageModule = Join-Path $repository "scripts\Aster.Packaging.psm1"
    }
}

function Invoke-PackageFixture {
    param([object]$Fixture, [string]$OutputDirectory = "outputs")
    & $Fixture.PackageScript `
        -OutputDirectory $OutputDirectory `
        -EvidenceRecord $Fixture.EvidenceRelative `
        -VerifierIdentity "fixture-reviewer"
}

function Assert-FixtureFailure {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock]$Action,
        [Parameter(Mandatory = $true)]
        [string]$ExpectedMessage,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    $failedAsExpected = $false
    try {
        & $Action
    }
    catch {
        if ($_.Exception.Message -match $ExpectedMessage) {
            $failedAsExpected = $true
        }
        else {
            throw "$Label failed for an unexpected reason: $($_.Exception.Message)"
        }
    }
    if (-not $failedAsExpected) {
        throw "$Label did not reject the abuse fixture."
    }
}

try {
    New-Item -ItemType Directory -Path $sandbox | Out-Null

    $positive = New-FixtureRepository "positive"
    Invoke-PackageFixture $positive
    $positiveOutput = Join-Path $positive.Root "outputs"
    foreach ($required in @("source-inventory.json", "Aster-MVP-v2-source.zip", "SHA256SUMS.txt", "verification-report.md", "verification-evidence.log")) {
        if (-not (Test-Path -LiteralPath (Join-Path $positiveOutput $required) -PathType Leaf)) {
            throw "The positive package fixture did not publish $required."
        }
    }
    $positiveReport = Read-AsterStrictUtf8Text `
        -RepositoryRoot $positive.Root `
        -Path (Join-Path $positiveOutput "verification-report.md") `
        -MaximumBytes 1048576
    $positiveSourceLines = [regex]::Matches($positiveReport, '(?im)^\*\*Source revision:\*\*.*$').Count
    $positiveVerifier = $positiveReport -match '(?m)^\*\*Self-declared verifier identity:\*\* fixture-reviewer\r?$'
    $positiveClassification = $positiveReport -match '(?m)^\*\*Overall classification:\*\* Unsigned engineering build for local evaluation\r?$'
    $positiveAuthenticodeHeader = $positiveReport -match '(?m)^\| File \| Bytes \| Modified UTC \| SHA-256 \| Authenticode observation \|\r?$'
    $positiveAuthenticodeRows = [regex]::Matches(
        $positiveReport,
        '(?m)\| (?:NotSigned|UnknownError|NOT RUN \(verifier unavailable\)) \|\r?$'
    ).Count
    $positiveAuthenticodeScope = $positiveReport -match 'it is not signature\s+evidence and never satisfies the production signing gate'
    $positiveExecutableRows = @(
        [regex]::Matches($positiveReport, '(?m)^\| `[^`]+\.exe` \|.*\r?$') |
            ForEach-Object { $_.Value.TrimEnd("`r") }
    ) -join " || "
    if (
        $positiveSourceLines -ne 1 -or
        -not $positiveVerifier -or
        -not $positiveClassification -or
        -not $positiveAuthenticodeHeader -or
        $positiveAuthenticodeRows -ne 2 -or
        -not $positiveAuthenticodeScope
    ) {
        throw "The positive package report failed its bounded identity assertions (source=$positiveSourceLines; verifier=$positiveVerifier; classification=$positiveClassification; AuthenticodeHeader=$positiveAuthenticodeHeader; AuthenticodeRows=$positiveAuthenticodeRows; AuthenticodeScope=$positiveAuthenticodeScope; executableRows=$positiveExecutableRows)."
    }
    Import-Module $positive.PackageModule -Force
    $positiveInventoryText = Read-AsterStrictUtf8Text `
        -RepositoryRoot $positive.Root `
        -Path (Join-Path $positiveOutput "source-inventory.json") `
        -MaximumBytes 16777216
    $positiveInventory = $positiveInventoryText | ConvertFrom-Json
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $positiveArchive = Join-Path $positiveOutput "Aster-MVP-v2-source.zip"
    $archiveFixtureDirectory = Join-Path $positive.Root "work\archive-fixtures"
    New-Item -ItemType Directory -Path $archiveFixtureDirectory | Out-Null
    $unexpectedArchive = Join-Path $archiveFixtureDirectory "unexpected-file.zip"
    $unsafeDirectoryArchive = Join-Path $archiveFixtureDirectory "unsafe-directory.zip"
    $modeArchive = Join-Path $archiveFixtureDirectory "mode-tamper.zip"
    $missingParentArchive = Join-Path $archiveFixtureDirectory "missing-parent.zip"
    $directoryModeArchive = Join-Path $archiveFixtureDirectory "directory-mode-tamper.zip"
    Copy-Item -LiteralPath $positiveArchive -Destination $unexpectedArchive
    Copy-Item -LiteralPath $positiveArchive -Destination $unsafeDirectoryArchive
    Copy-Item -LiteralPath $positiveArchive -Destination $modeArchive
    Copy-Item -LiteralPath $positiveArchive -Destination $missingParentArchive
    Copy-Item -LiteralPath $positiveArchive -Destination $directoryModeArchive

    $zip = [System.IO.Compression.ZipFile]::Open($unexpectedArchive, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        $unexpectedEntry = $zip.CreateEntry("ignored-private.txt")
        $entryStream = $unexpectedEntry.Open()
        try {
            $entryBytes = [System.Text.UTF8Encoding]::new($false).GetBytes("must reject")
            $entryStream.Write($entryBytes, 0, $entryBytes.Length)
        }
        finally {
            $entryStream.Dispose()
        }
    }
    finally {
        $zip.Dispose()
    }
    Assert-FixtureFailure {
        Assert-AsterZipInventory `
            -RepositoryRoot $positive.Root `
            -ArchivePath $unexpectedArchive `
            -Inventory $positiveInventory
    } "unexpected, unsafe, or duplicate path" "source-archive inventory fixture"

    $zip = [System.IO.Compression.ZipFile]::Open($unsafeDirectoryArchive, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        [void]$zip.CreateEntry("../unsafe-directory/")
    }
    finally {
        $zip.Dispose()
    }
    Assert-FixtureFailure {
        Assert-AsterZipInventory `
            -RepositoryRoot $positive.Root `
            -ArchivePath $unsafeDirectoryArchive `
            -Inventory $positiveInventory
    } "unexpected, unsafe, or duplicate path" "unsafe archive-directory fixture"

    $zip = [System.IO.Compression.ZipFile]::Open($modeArchive, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        $licenseEntry = $zip.GetEntry("LICENSE")
        if ($null -eq $licenseEntry) {
            throw "The fixture source archive is missing LICENSE."
        }
        $symlinkMode = [uint32](([uint64][Convert]::ToInt32("120777", 8)) -shl 16)
        $licenseEntry.ExternalAttributes = [BitConverter]::ToInt32([BitConverter]::GetBytes($symlinkMode), 0)
    }
    finally {
        $zip.Dispose()
    }
    Assert-FixtureFailure {
        Assert-AsterZipInventory `
            -RepositoryRoot $positive.Root `
            -ArchivePath $modeArchive `
            -Inventory $positiveInventory
    } "mode does not match the tracked Git mode" "regular-to-symlink archive fixture"

    $zip = [System.IO.Compression.ZipFile]::Open($missingParentArchive, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        $parentEntry = $zip.GetEntry("scripts/")
        if ($null -eq $parentEntry) {
            throw "The fixture source archive is missing the scripts directory entry."
        }
        $parentEntry.Delete()
    }
    finally {
        $zip.Dispose()
    }
    Assert-FixtureFailure {
        Assert-AsterZipInventory `
            -RepositoryRoot $positive.Root `
            -ArchivePath $missingParentArchive `
            -Inventory $positiveInventory
    } "directory inventory is incomplete" "missing ZIP parent-directory fixture"

    $zip = [System.IO.Compression.ZipFile]::Open($directoryModeArchive, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        $directoryEntry = $zip.GetEntry("scripts/")
        if ($null -eq $directoryEntry) {
            throw "The fixture source archive is missing the scripts directory entry."
        }
        $regularMode = [uint32](([uint64][Convert]::ToInt32("100644", 8)) -shl 16)
        $directoryEntry.ExternalAttributes = [BitConverter]::ToInt32([BitConverter]::GetBytes($regularMode), 0)
    }
    finally {
        $zip.Dispose()
    }
    Assert-FixtureFailure {
        Assert-AsterZipInventory `
            -RepositoryRoot $positive.Root `
            -ArchivePath $directoryModeArchive `
            -Inventory $positiveInventory
    } "invalid directory attributes" "invalid ZIP directory-mode fixture"

    $dirty = New-FixtureRepository "dirty"
    [System.IO.File]::AppendAllText((Join-Path $dirty.Root "LICENSE"), "dirty", [System.Text.UTF8Encoding]::new($false))
    Assert-FixtureFailure { Invoke-PackageFixture $dirty } "clean Git working tree" "dirty-tree fixture"

    $wrongHead = New-FixtureRepository "wrong-head"
    [System.IO.File]::AppendAllText((Join-Path $wrongHead.Root "NOTICE"), "new commit", [System.Text.UTF8Encoding]::new($false))
    Invoke-FixtureGit $wrongHead.Root @("add", "NOTICE")
    Invoke-FixtureGit $wrongHead.Root @("commit", "-q", "-m", "different source")
    Assert-FixtureFailure { Invoke-PackageFixture $wrongHead } "complete current source revision" "wrong-HEAD fixture"

    $staleArtifact = New-FixtureRepository "stale-artifact"
    [System.IO.File]::AppendAllText($staleArtifact.ReleaseBinary, "tampered", [System.Text.UTF8Encoding]::new($false))
    Assert-FixtureFailure { Invoke-PackageFixture $staleArtifact } "does not match the clean-source build identity" "stale-artifact fixture"

    $tamperedSbom = New-FixtureRepository "tampered-sbom"
    [System.IO.File]::AppendAllText($tamperedSbom.FrontendSbom, "tampered", [System.Text.UTF8Encoding]::new($false))
    Assert-FixtureFailure { Invoke-PackageFixture $tamperedSbom } "does not match the clean-source build identity" "tampered-SBOM fixture"

    $malformedManifest = New-FixtureRepository "malformed-manifest"
    Write-FixtureText $malformedManifest.ManifestPath "{not valid json"
    Assert-FixtureFailure { Invoke-PackageFixture $malformedManifest } "exact duplicate-free schema" "malformed-manifest fixture"

    $strictManifest = New-FixtureRepository "strict-manifest"
    $canonicalManifest = [System.IO.File]::ReadAllText($strictManifest.ManifestPath, [System.Text.Encoding]::UTF8)
    $manifestVariants = @(
        [pscustomobject]@{
            Label = "wrong JSON token type"
            Expected = "exact duplicate-free schema"
            Text = $canonicalManifest -replace '"schemaVersion"\s*:\s*1', '"schemaVersion": "1"'
        },
        [pscustomobject]@{
            Label = "string byte count"
            Expected = "exact duplicate-free schema"
            Text = $canonicalManifest -replace '("bytes"\s*:\s*)[0-9]+', '$1"4"'
        },
        [pscustomobject]@{
            Label = "fractional byte count"
            Expected = "exact duplicate-free schema"
            Text = $canonicalManifest -replace '("bytes"\s*:\s*)[0-9]+', '${1}1.5'
        },
        [pscustomobject]@{
            Label = "unsafe-range JSON integer"
            Expected = "outside the integral JSON safe range"
            Text = $canonicalManifest -replace '("bytes"\s*:\s*)[0-9]+', '${1}9007199254740992'
        },
        [pscustomobject]@{
            Label = "duplicate JSON key"
            Expected = "exact duplicate-free schema"
            Text = $canonicalManifest -replace '"schemaVersion"\s*:\s*1\s*,', '"schemaVersion": 1, "schemaVersion": 1,'
        },
        [pscustomobject]@{
            Label = "impossible manifest date"
            Expected = "real canonical UTC timestamp"
            Text = $canonicalManifest.Replace("2026-07-13T00:00:00Z", "2026-02-30T00:00:00Z")
        },
        [pscustomobject]@{
            Label = "extra JSON property"
            Expected = "exact duplicate-free schema"
            Text = [regex]::Replace($canonicalManifest, '^\s*\{', '{ "unexpected": true,', 1)
        }
    )
    foreach ($variant in $manifestVariants) {
        Write-FixtureText $strictManifest.ManifestPath $variant.Text
        Assert-FixtureFailure { Invoke-PackageFixture $strictManifest } $variant.Expected "$($variant.Label) fixture"
    }
    Write-FixtureText $strictManifest.ManifestPath $canonicalManifest

    $duplicateSource = New-FixtureRepository "duplicate-source"
    [System.IO.File]::AppendAllText(
        $duplicateSource.EvidencePath,
        "`n**Source revision:** $($duplicateSource.Revision)`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    Assert-FixtureFailure { Invoke-PackageFixture $duplicateSource } "exactly one canonical full Source revision line" "duplicate-source fixture"

    $invalidUtf8 = New-FixtureRepository "invalid-utf8"
    [System.IO.File]::WriteAllBytes($invalidUtf8.EvidencePath, [byte[]](0xc3, 0x28))
    Assert-FixtureFailure { Invoke-PackageFixture $invalidUtf8 } "not strict UTF-8" "strict-UTF-8 fixture"

    $oversizedEvidence = New-FixtureRepository "oversized-evidence"
    [System.IO.File]::WriteAllBytes($oversizedEvidence.EvidencePath, (New-Object byte[] 262145))
    Assert-FixtureFailure { Invoke-PackageFixture $oversizedEvidence } "exceeds the 262144-byte" "evidence-size fixture"

    $secretEvidence = New-FixtureRepository "secret-evidence"
    $secretValue = "sk" + "-" + ("a" * 24)
    [System.IO.File]::AppendAllText($secretEvidence.EvidencePath, "`n$secretValue`n", [System.Text.UTF8Encoding]::new($false))
    Assert-FixtureFailure { Invoke-PackageFixture $secretEvidence } "shared secret pattern" "shared-secret evidence fixture"

    $personalPathEvidence = New-FixtureRepository "personal-path-evidence"
    $personalPath = "C:" + "\Users\Fixture Person\private.txt"
    [System.IO.File]::AppendAllText($personalPathEvidence.EvidencePath, "`n$personalPath`n", [System.Text.UTF8Encoding]::new($false))
    Assert-FixtureFailure { Invoke-PackageFixture $personalPathEvidence } "user profile path" "personal-path evidence fixture"

    $headerEvidence = New-FixtureRepository "header-evidence"
    $header = "Author" + "ization: Bearer redacted"
    [System.IO.File]::AppendAllText($headerEvidence.EvidencePath, "`n$header`n", [System.Text.UTF8Encoding]::new($false))
    Assert-FixtureFailure { Invoke-PackageFixture $headerEvidence } "credential header" "credential-header evidence fixture"

    $evidenceBypasses = New-FixtureRepository "evidence-bypasses"
    $canonicalEvidence = [System.IO.File]::ReadAllText($evidenceBypasses.EvidencePath, [System.Text.Encoding]::UTF8)
    $canonicalResultRow = [regex]::Match($canonicalEvidence, '(?m)^\| `AC-031`[^\r\n]+\r?$').Value.TrimEnd("`r")
    $canonicalClassification = "**Overall classification:** Unsigned engineering build for local evaluation"
    $duplicateResultRow = $canonicalResultRow.Replace("``PASS``", "``FAIL``").Replace(
        "| $($evidenceBypasses.EvidenceLogRelative) |",
        "| Retained failure history below |"
    )
    $whitespaceAliasRow = $duplicateResultRow.Replace("``AC-031``", "``AC-031 ``")
    $forwardUserPath = "C:" + "/Users/Fixture Person/private.txt"
    $jsonEscapedUserPath = "C:" + "\\Users\\Fixture Person\\private.txt"
    $unicodeEscapedUserPath = "C:" + "\u002fUsers\u002fFixture Person\u002fprivate.txt"
    $markdownHeader = "> - **Author" + "ization**: Bearer redacted"
    $listHeader = "1. ``X-Api-Key``: redacted"
    $evidenceVariants = @(
        [pscustomobject]@{
            Label = "forward-slash user path"
            Expected = "user profile path"
            Text = $canonicalEvidence + "`n$forwardUserPath`n"
        },
        [pscustomobject]@{
            Label = "JSON-escaped user path"
            Expected = "user profile path"
            Text = $canonicalEvidence + "`n$jsonEscapedUserPath`n"
        },
        [pscustomobject]@{
            Label = "Unicode slash-escaped user path"
            Expected = "user profile path"
            Text = $canonicalEvidence + "`n$unicodeEscapedUserPath`n"
        },
        [pscustomobject]@{
            Label = "Markdown blockquote header"
            Expected = "credential header"
            Text = $canonicalEvidence + "`n$markdownHeader`n"
        },
        [pscustomobject]@{
            Label = "Markdown list header"
            Expected = "credential header"
            Text = $canonicalEvidence + "`n$listHeader`n"
        },
        [pscustomobject]@{
            Label = "missing procedure identity"
            Expected = "complete ProcedureIdentity field"
            Text = [regex]::Replace(
                $canonicalEvidence,
                '(?m)^\*\*Procedure identity \(self-declared\):\*\*.*\r?\n',
                "",
                1
            )
        },
        [pscustomobject]@{
            Label = "duplicate engineering classification"
            Expected = "exactly one canonical unsigned engineering classification"
            Text = $canonicalEvidence.Replace(
                $canonicalClassification,
                "$canonicalClassification`n`n$canonicalClassification"
            )
        },
        [pscustomobject]@{
            Label = "production classification"
            Expected = "exactly one canonical unsigned engineering classification"
            Text = $canonicalEvidence.Replace(
                $canonicalClassification,
                "**Overall classification:** Production release"
            )
        },
        [pscustomobject]@{
            Label = "impossible evidence timestamp"
            Expected = "real ordered canonical UTC timestamps"
            Text = $canonicalEvidence.Replace("2026-07-13T00:01:00Z", "2026-02-30T00:01:00Z")
        },
        [pscustomobject]@{
            Label = "inexact result procedure"
            Expected = "exact procedure"
            Text = $canonicalEvidence.Replace(
                "``powershell -File scripts/package-provenance.fixtures.ps1``",
                "TBD"
            )
        },
        [pscustomobject]@{
            Label = "unbackticked result row"
            Expected = "malformed or unvalidated result row"
            Text = $canonicalEvidence.Replace(
                "| ``AC-031`` | ``PASS`` |",
                "| AC-999 | PASS |"
            )
        },
        [pscustomobject]@{
            Label = "indented malformed result row"
            Expected = "malformed or unvalidated result row"
            Text = $canonicalEvidence.Replace(
                $canonicalResultRow,
                "  $canonicalResultRow"
            )
        },
        [pscustomobject]@{
            Label = "duplicate conflicting gate result"
            Expected = "more than one current result for gate AC-031"
            Text = $canonicalEvidence.Replace(
                $canonicalResultRow,
                "$canonicalResultRow`n$duplicateResultRow"
            )
        },
        [pscustomobject]@{
            Label = "whitespace-padded gate alias"
            Expected = "malformed or unvalidated result row"
            Text = $canonicalEvidence.Replace(
                $canonicalResultRow,
                "$canonicalResultRow`n$whitespaceAliasRow"
            )
        },
        [pscustomobject]@{
            Label = "PASS without reviewable evidence"
            Expected = "PASS result must identify the canonical retained evidence log"
            Text = $canonicalEvidence.Replace(
                "| $($evidenceBypasses.EvidenceLogRelative) |",
                "| none available after cleanup |"
            )
        },
        [pscustomobject]@{
            Label = "artifact without hash"
            Expected = "Artifact field must say Not applicable or include"
            Text = $canonicalEvidence.Replace(
                "**Artifact:** Not applicable - fixture-only package inputs",
                "**Artifact:** Aster candidate executable"
            )
        }
    )
    foreach ($variant in $evidenceVariants) {
        Write-FixtureText $evidenceBypasses.EvidencePath $variant.Text
        Assert-FixtureFailure { Invoke-PackageFixture $evidenceBypasses } $variant.Expected "$($variant.Label) fixture"
    }
    Write-FixtureText $evidenceBypasses.EvidencePath $canonicalEvidence

    $missingEvidenceLog = New-FixtureRepository "missing-evidence-log"
    Remove-Item -LiteralPath $missingEvidenceLog.EvidenceLogPath
    Assert-FixtureFailure {
        Invoke-PackageFixture $missingEvidenceLog
    } "Required packaging input is missing" "missing retained evidence-log fixture"

    $invalidEvidenceLog = New-FixtureRepository "invalid-evidence-log"
    [System.IO.File]::WriteAllBytes($invalidEvidenceLog.EvidenceLogPath, [byte[]](0xc3, 0x28))
    Assert-FixtureFailure {
        Invoke-PackageFixture $invalidEvidenceLog
    } "not strict UTF-8" "invalid retained evidence-log UTF-8 fixture"

    $oversizedEvidenceLog = New-FixtureRepository "oversized-evidence-log"
    [System.IO.File]::WriteAllBytes($oversizedEvidenceLog.EvidenceLogPath, (New-Object byte[] 4194305))
    Assert-FixtureFailure {
        Invoke-PackageFixture $oversizedEvidenceLog
    } "exceeds the 4194304-byte safety limit" "oversized retained evidence-log fixture"

    $personalPathEvidenceLog = New-FixtureRepository "personal-path-evidence-log"
    $logPersonalPath = "C:" + "/Users/Fixture Person/private.txt"
    [System.IO.File]::AppendAllText(
        $personalPathEvidenceLog.EvidenceLogPath,
        "`n$logPersonalPath`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    Assert-FixtureFailure {
        Invoke-PackageFixture $personalPathEvidenceLog
    } "user profile path" "personal-path retained evidence-log fixture"

    $credentialHeaderEvidenceLog = New-FixtureRepository "credential-header-evidence-log"
    $logHeader = "Author" + "ization: Bearer redacted"
    [System.IO.File]::AppendAllText(
        $credentialHeaderEvidenceLog.EvidenceLogPath,
        "`n$logHeader`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    Assert-FixtureFailure {
        Invoke-PackageFixture $credentialHeaderEvidenceLog
    } "credential header" "credential-header retained evidence-log fixture"

    $sharedSecretEvidenceLog = New-FixtureRepository "shared-secret-evidence-log"
    $fakeProviderToken = "sk-" + (("x" * 24) -join "")
    [System.IO.File]::AppendAllText(
        $sharedSecretEvidenceLog.EvidenceLogPath,
        "`n$fakeProviderToken`n",
        [System.Text.UTF8Encoding]::new($false)
    )
    Assert-FixtureFailure {
        Invoke-PackageFixture $sharedSecretEvidenceLog
    } "shared secret pattern" "shared-secret retained evidence-log fixture"

    $partialFilename = New-FixtureRepository "partial-filename"
    $shortRevision = $partialFilename.Revision.Substring(0, 12)
    $shortRelative = "work/evidence/2026-07-13-$shortRevision-engineering-build.md"
    Write-FixtureText (Join-Path $partialFilename.Root $shortRelative) (Get-FixtureEvidence $partialFilename.Revision)
    $partialFilename.EvidenceRelative = $shortRelative
    Assert-FixtureFailure { Invoke-PackageFixture $partialFilename } "full-source-revision" "full-revision filename fixture"

    if ($env:OS -eq "Windows_NT") {
        $junction = New-FixtureRepository "junction"
        $junctionTarget = Join-Path $sandbox "junction-publish-target"
        New-Item -ItemType Directory -Path $junctionTarget | Out-Null
        Write-FixtureText (Join-Path $junctionTarget "must-survive.txt") "junction target"
        $junctionPath = Join-Path $junction.Root "work\publish-link"
        New-Item -ItemType Junction -Path $junctionPath -Target $junctionTarget | Out-Null
        $fixtureJunctions.Add($junctionPath)
        Assert-FixtureFailure {
            Invoke-PackageFixture $junction "work\publish-link\bundle"
        } "Reparse points and junctions are prohibited" "package-junction fixture"
        if (-not (Test-Path -LiteralPath (Join-Path $junctionTarget "must-survive.txt") -PathType Leaf)) {
            throw "The junction fixture altered its external target."
        }
    }

    $unexpectedOutput = New-FixtureRepository "unexpected-output"
    New-Item -ItemType Directory -Path (Join-Path $unexpectedOutput.Root "outputs") | Out-Null
    Write-FixtureText (Join-Path $unexpectedOutput.Root "outputs\private-sentinel.txt") "must reject"
    Assert-FixtureFailure { Invoke-PackageFixture $unexpectedOutput } "must not already exist" "unexpected-output fixture"

    $postCopy = New-FixtureRepository "post-copy-mismatch"
    Import-Module $postCopy.PackageModule -Force
    $postCopyHook = {
        param($Paths)
        [System.IO.File]::AppendAllText($Paths.portableOutput, "changed after copy", [System.Text.UTF8Encoding]::new($false))
    }
    Assert-FixtureFailure {
        Invoke-AsterEngineeringPackage `
            -RepositoryRoot $postCopy.Root `
            -ScriptsRoot (Join-Path $postCopy.Root "scripts") `
            -EvidenceRecord $postCopy.EvidenceRelative `
            -VerifierIdentity "fixture-reviewer" `
            -FixtureAfterCopyHook $postCopyHook
    } "copied release binary does not match" "post-copy mismatch fixture"

    $postCopyEvidenceLog = New-FixtureRepository "post-copy-evidence-log-mismatch"
    Import-Module $postCopyEvidenceLog.PackageModule -Force
    $postCopyEvidenceLogHook = {
        param($Paths)
        [System.IO.File]::AppendAllText(
            $Paths.verificationEvidence,
            "changed after validated snapshot",
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    Assert-FixtureFailure {
        Invoke-AsterEngineeringPackage `
            -RepositoryRoot $postCopyEvidenceLog.Root `
            -ScriptsRoot (Join-Path $postCopyEvidenceLog.Root "scripts") `
            -EvidenceRecord $postCopyEvidenceLog.EvidenceRelative `
            -VerifierIdentity "fixture-reviewer" `
            -FixtureAfterCopyHook $postCopyEvidenceLogHook
    } "inventory or verification report changed" "post-copy evidence-log mismatch fixture"

    $finalMismatch = New-FixtureRepository "final-state-mismatch"
    Import-Module $finalMismatch.PackageModule -Force
    $finalStateHook = {
        param($Paths)
        [System.IO.File]::AppendAllText((Join-Path $finalMismatch.Root "LICENSE"), "changed at final state", [System.Text.UTF8Encoding]::new($false))
    }.GetNewClosure()
    Assert-FixtureFailure {
        Invoke-AsterEngineeringPackage `
            -RepositoryRoot $finalMismatch.Root `
            -ScriptsRoot (Join-Path $finalMismatch.Root "scripts") `
            -EvidenceRecord $finalMismatch.EvidenceRelative `
            -VerifierIdentity "fixture-reviewer" `
            -FixtureBeforeFinalStateHook $finalStateHook
    } "clean Git working tree during final staged-package verification" "final-state mismatch fixture"

    $finalPackageMismatch = New-FixtureRepository "final-package-mismatch"
    Import-Module $finalPackageMismatch.PackageModule -Force
    $finalPackageHook = {
        param($Paths)
        [System.IO.File]::AppendAllText(
            $Paths.sourceInventory,
            "changed after final checksum creation",
            [System.Text.UTF8Encoding]::new($false)
        )
    }
    Assert-FixtureFailure {
        Invoke-AsterEngineeringPackage `
            -RepositoryRoot $finalPackageMismatch.Root `
            -ScriptsRoot (Join-Path $finalPackageMismatch.Root "scripts") `
            -EvidenceRecord $finalPackageMismatch.EvidenceRelative `
            -VerifierIdentity "fixture-reviewer" `
            -FixtureBeforeFinalStateHook $finalPackageHook
    } "checksum inventory does not match" "final-package mismatch fixture"

    $outputAppears = New-FixtureRepository "output-appears"
    Import-Module $outputAppears.PackageModule -Force
    $outputAppearsHook = {
        param($Paths)
        New-Item -ItemType Directory -Path (Join-Path $outputAppears.Root "outputs") | Out-Null
        [System.IO.File]::WriteAllText(
            (Join-Path $outputAppears.Root "outputs\late-sentinel.txt"),
            "late output",
            [System.Text.UTF8Encoding]::new($false)
        )
    }.GetNewClosure()
    Assert-FixtureFailure {
        Invoke-AsterEngineeringPackage `
            -RepositoryRoot $outputAppears.Root `
            -ScriptsRoot (Join-Path $outputAppears.Root "scripts") `
            -EvidenceRecord $outputAppears.EvidenceRelative `
            -VerifierIdentity "fixture-reviewer" `
            -FixtureBeforeFinalStateHook $outputAppearsHook
    } "appeared during package assembly" "late-output mismatch fixture"

    if ($env:OS -eq "Windows_NT") {
        $publicationRace = New-FixtureRepository "publication-race"
        Import-Module $publicationRace.PackageModule -Force
        $externalTarget = Join-Path $sandbox "publication-race-external-target"
        New-Item -ItemType Directory -Path $externalTarget | Out-Null
        $externalSentinel = Join-Path $externalTarget "must-survive.txt"
        Write-FixtureText $externalSentinel "external target must survive"
        $publicationRaceHook = {
            param($Paths, $OutputPath)
            New-Item -ItemType Junction -Path $OutputPath -Target $externalTarget | Out-Null
            [void]$fixtureJunctions.Add($OutputPath)
        }.GetNewClosure()
        Assert-FixtureFailure {
            Invoke-AsterEngineeringPackage `
                -RepositoryRoot $publicationRace.Root `
                -ScriptsRoot (Join-Path $publicationRace.Root "scripts") `
                -EvidenceRecord $publicationRace.EvidenceRelative `
                -VerifierIdentity "fixture-reviewer" `
                -FixtureBeforePublicationHook $publicationRaceHook
        } "already exists|Cannot create a file" "atomic publication-race fixture"
        $publicationOutput = Join-Path $publicationRace.Root "outputs"
        if (-not (Test-Path -LiteralPath $externalSentinel -PathType Leaf)) {
            throw "The publication race altered the external junction target."
        }
        if (Test-Path -LiteralPath (Join-Path $publicationOutput "Aster-0.2.0-x64-engineering.exe") -PathType Leaf) {
            throw "The unpublished candidate received the final output name during a destination race."
        }

        $lockedPublication = New-FixtureRepository "locked-publication-child"
        Import-Module $lockedPublication.PackageModule -Force
        $lockState = [pscustomobject]@{ Handle = $null }
        $lockedPublicationHook = {
            param($Paths, $OutputPath)
            $lockState.Handle = [System.IO.File]::Open(
                $Paths.portableOutput,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::None
            )
        }.GetNewClosure()
        try {
            Assert-FixtureFailure {
                Invoke-AsterEngineeringPackage `
                    -RepositoryRoot $lockedPublication.Root `
                    -ScriptsRoot (Join-Path $lockedPublication.Root "scripts") `
                    -EvidenceRecord $lockedPublication.EvidenceRelative `
                    -VerifierIdentity "fixture-reviewer" `
                    -FixtureBeforePublicationHook $lockedPublicationHook
            } "Access to the path.*is denied" "locked publication-child fixture"
        }
        finally {
            if ($null -ne $lockState.Handle) {
                $lockState.Handle.Dispose()
            }
        }
        if (Test-Path -LiteralPath (Join-Path $lockedPublication.Root "outputs")) {
            throw "A locked unpublished candidate received the final output name."
        }
    }

    Write-Host "Package-provenance fixtures passed."
}
finally {
    foreach ($junctionPath in $fixtureJunctions) {
        if (Test-Path -LiteralPath $junctionPath) {
            $fullJunction = [System.IO.Path]::GetFullPath($junctionPath)
            if (-not $fullJunction.StartsWith($sandbox + [System.IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
                throw "Fixture cleanup refused a junction outside its temporary sandbox."
            }
            $junctionItem = Get-Item -LiteralPath $fullJunction -Force
            if (($junctionItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0) {
                throw "Fixture cleanup refused a path that was no longer the expected junction."
            }
            [System.IO.Directory]::Delete($fullJunction, $false)
        }
    }
    if (Test-Path -LiteralPath $sandbox) {
        $safeSandbox = [System.IO.Path]::GetFullPath($sandbox)
        if (
            $safeSandbox.StartsWith($temporaryPrefix, [StringComparison]::OrdinalIgnoreCase) -and
            -not [string]::Equals($safeSandbox, $temporaryRoot, [StringComparison]::OrdinalIgnoreCase)
        ) {
            Remove-AsterSafeDirectoryTree -RepositoryRoot $temporaryRoot -Path $safeSandbox
        }
    }
}
