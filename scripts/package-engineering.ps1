[CmdletBinding()]
param(
    [string]$OutputDirectory = "outputs",
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRecord
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$git = (Get-Command git.exe -ErrorAction Stop).Source

if ([System.IO.Path]::IsPathRooted($EvidenceRecord) -or $EvidenceRecord -match '(^|[\\/])\.\.([\\/]|$)') {
    throw "EvidenceRecord must be a repository-relative path without parent traversal."
}

$topLevel = & $git -C $root rev-parse --show-toplevel 2>$null
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($topLevel)) {
    throw "Engineering packaging requires an initialized Git repository."
}
$resolvedTopLevel = (Resolve-Path $topLevel.Trim()).Path
if (-not [string]::Equals($resolvedTopLevel, $root, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The packaging script must run from the repository that contains this source tree."
}

$sourceRevision = (& $git -C $root rev-parse --verify HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $sourceRevision -notmatch '^[0-9a-fA-F]{40,64}$') {
    throw "The source revision could not be identified."
}

$workingTreeStatus = @(& $git -C $root status --porcelain=v1 --untracked-files=normal)
if ($LASTEXITCODE -ne 0) {
    throw "The Git working-tree status could not be inspected."
}
if ($workingTreeStatus.Count -ne 0) {
    throw "Engineering packaging requires a clean Git working tree. Commit or remove every staged, unstaged, and untracked source change first."
}

$output = Join-Path $root $OutputDirectory
$releaseBinary = Join-Path $root "src-tauri\target\release\aster-desktop.exe"
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\Aster_0.1.0_x64-setup.exe"
$frontendSbom = Join-Path $root "work\sbom-frontend.cdx.json"
$rustSbom = Join-Path $root "work\sbom-rust.cdx.json"
$license = Join-Path $root "LICENSE"
$notice = Join-Path $root "NOTICE"
$preview = Join-Path $output "Aster-MVP-v1-preview.png"
$evidence = Join-Path $root $EvidenceRecord
$evidenceRelative = $EvidenceRecord.Replace("\", "/")

& $git -C $root ls-files --error-unmatch -- $evidenceRelative 1>$null 2>$null
if ($LASTEXITCODE -ne 0) {
    throw "The evidence record must be tracked in the identified source revision."
}

if (-not (Test-Path -LiteralPath $releaseBinary -PathType Leaf)) {
    throw "The release binary does not exist. Run the Tauri engineering build first."
}
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    throw "The NSIS engineering installer does not exist. Run the Tauri engineering build first."
}
foreach ($required in @($frontendSbom, $rustSbom, $license, $notice, $preview, $evidence)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "Required packaging input is missing: $required"
    }
}

$packageAudit = Join-Path $PSScriptRoot "package-audit.ps1"
& $packageAudit

New-Item -ItemType Directory -Path $output -Force | Out-Null

$portableOutput = Join-Path $output "Aster-0.1.0-x64-engineering.exe"
$installerOutput = Join-Path $output "Aster-0.1.0-x64-engineering-setup.exe"
$frontendSbomOutput = Join-Path $output "sbom-frontend.cdx.json"
$rustSbomOutput = Join-Path $output "sbom-rust.cdx.json"
$licenseOutput = Join-Path $output "LICENSE"
$noticeOutput = Join-Path $output "NOTICE"
$sourceArchive = Join-Path $output "Aster-MVP-v1-source.zip"
$verification = Join-Path $output "verification-report.md"
$checksumPath = Join-Path $output "SHA256SUMS.txt"

foreach ($stale in @(
        $portableOutput,
        $installerOutput,
        $frontendSbomOutput,
        $rustSbomOutput,
        $licenseOutput,
        $noticeOutput,
        $sourceArchive,
        $verification,
        $checksumPath
    )) {
    if (Test-Path -LiteralPath $stale -PathType Leaf) {
        Remove-Item -LiteralPath $stale -Force
    }
}

Copy-Item -LiteralPath $releaseBinary -Destination $portableOutput -Force
Copy-Item -LiteralPath $installer -Destination $installerOutput -Force
Copy-Item -LiteralPath $frontendSbom -Destination $frontendSbomOutput -Force
Copy-Item -LiteralPath $rustSbom -Destination $rustSbomOutput -Force
Copy-Item -LiteralPath $license -Destination $licenseOutput -Force
Copy-Item -LiteralPath $notice -Destination $noticeOutput -Force

& $git -C $root archive --format=zip --output=$sourceArchive HEAD
if ($LASTEXITCODE -ne 0) {
    throw "Tracked source archive generation failed with exit code $LASTEXITCODE."
}

$identityTargets = @(
    $portableOutput,
    $installerOutput,
    $sourceArchive,
    $frontendSbomOutput,
    $rustSbomOutput,
    $licenseOutput,
    $noticeOutput,
    $preview
)
$identityRows = foreach ($path in $identityTargets) {
    $item = Get-Item -LiteralPath $path
    $hash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    $signature = if ($item.Extension -eq ".exe") {
        (Get-AuthenticodeSignature -LiteralPath $path).Status.ToString()
    } else {
        "Not applicable"
    }
    $modifiedUtc = $item.LastWriteTimeUtc.ToString("yyyy-MM-ddTHH:mm:ssZ")
    "| ``$($item.Name)`` | $($item.Length) | $modifiedUtc | ``$hash`` | $signature |"
}

$report = [System.IO.File]::ReadAllText($evidence)
$generatedUtc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
$identitySection = @"

## Packaged artifact identity

This table was generated by ``scripts/package-engineering.ps1`` after the source snapshot and copied artifacts were finalized. ``SHA256SUMS.txt`` separately covers every file in the output directory except itself.

**Generated UTC:** $generatedUtc

**Identity:** local Codex verification session
**Source revision:** ``$sourceRevision``

| File | Bytes | Modified UTC | SHA-256 | Authenticode |
| --- | ---: | --- | --- | --- |
$($identityRows -join [Environment]::NewLine)
"@
[System.IO.File]::WriteAllText(
    $verification,
    $report + $identitySection,
    [System.Text.UTF8Encoding]::new($false)
)

$checksumLines = Get-ChildItem -LiteralPath $output -File |
    Where-Object Name -ne "SHA256SUMS.txt" |
    Sort-Object Name |
    ForEach-Object {
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "$hash  $($_.Name)"
    }
[System.IO.File]::WriteAllLines($checksumPath, $checksumLines, [System.Text.UTF8Encoding]::new($false))

Write-Host "Engineering artifacts and SHA-256 checksums were written to $output."
