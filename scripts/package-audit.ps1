[CmdletBinding()]
param(
    [string]$ReleaseBinary = "src-tauri\target\release\aster-desktop.exe",
    [string]$Installer = "src-tauri\target\release\bundle\nsis\Aster_0.2.0_x64-setup.exe"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$identityModule = Join-Path $PSScriptRoot "Aster.BuildIdentity.psm1"
Import-Module $identityModule -Force
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
[void](Assert-AsterNoReparseAncestors -Path $root)
$dist = Join-Path $root "dist"
$artifacts = @(
    (Join-Path $root $ReleaseBinary),
    (Join-Path $root $Installer)
)

foreach ($artifact in $artifacts) {
    [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $artifact)
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
        throw "Required package artifact is missing: $artifact"
    }
}
[void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $dist)
if (-not (Test-Path -LiteralPath $dist -PathType Container)) {
    throw "The production frontend bundle is missing. Run the production build first."
}

$bundleFiles = @(Get-AsterSafeDirectoryFiles -RepositoryRoot $root -Path $dist)
$sourceMaps = @($bundleFiles | Where-Object { [System.IO.Path]::GetExtension($_) -ieq ".map" })
if ($sourceMaps.Count -ne 0) {
    throw "Production source maps were found in the frontend bundle."
}

$builderProfilePattern = if ([string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
    '(?!)'
}
else {
    [regex]::Escape($env:USERPROFILE)
}
$patterns = [ordered]@{
    BuilderProfilePath = $builderProfilePattern
    AuthorizationHeader = "Authorization: Bearer "
    TestCredential = "test_key_not_a_secret"
    SourceMapDirective = "sourceMappingURL"
    DevelopmentEndpoint = "(?:http|ws)://(?:127\.0\.0\.1|localhost):1420"
}
$sharedPatternPath = Join-Path $PSScriptRoot "secret-patterns.json"
$sharedPatternText = Read-AsterStrictUtf8Text -RepositoryRoot $root -Path $sharedPatternPath -MaximumBytes 65536
$sharedPatterns = $sharedPatternText | ConvertFrom-Json
foreach ($rule in $sharedPatterns) {
    $flags = if ($null -ne $rule.PSObject.Properties["flags"]) { [string]$rule.flags } else { "" }
    $prefix = if ($flags.Contains("i")) { "(?i)" } else { "" }
    $patterns["Credential/$($rule.name)"] = $prefix + $rule.pattern
}

$findings = [System.Collections.Generic.List[string]]::new()
foreach ($artifact in $artifacts) {
    [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $artifact)
    $bytes = [System.IO.File]::ReadAllBytes($artifact)
    [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $artifact)
    $decoded = [System.Collections.Generic.List[string]]::new()
    $decoded.Add([System.Text.Encoding]::UTF8.GetString($bytes))
    foreach ($encoding in @([System.Text.Encoding]::Unicode, [System.Text.Encoding]::BigEndianUnicode)) {
        foreach ($offset in @(0, 1)) {
            if ($bytes.Length -gt $offset) {
                $decoded.Add($encoding.GetString($bytes, $offset, $bytes.Length - $offset))
            }
        }
    }

    foreach ($pattern in $patterns.GetEnumerator()) {
        foreach ($text in $decoded) {
            if ([regex]::IsMatch($text, $pattern.Value)) {
                $findings.Add("$($pattern.Key) in $([System.IO.Path]::GetFileName($artifact))")
                break
            }
        }
    }
}

foreach ($file in $bundleFiles) {
    $text = Read-AsterStrictUtf8Text -RepositoryRoot $root -Path $file -MaximumBytes 52428800
    foreach ($pattern in $patterns.GetEnumerator()) {
        if ([regex]::IsMatch($text, $pattern.Value)) {
            $findings.Add("$($pattern.Key) in dist/$([System.IO.Path]::GetFileName($file))")
        }
    }
}

if ($findings.Count -ne 0) {
    $summary = $findings | Sort-Object -Unique
    throw "Package audit failed:`n$($summary -join [Environment]::NewLine)"
}

Write-Host "Package audit passed for the release binary, NSIS installer, and production frontend bundle."
