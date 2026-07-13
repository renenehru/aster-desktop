[CmdletBinding()]
param(
    [string]$ReleaseBinary = "src-tauri\target\release\aster-desktop.exe",
    [string]$Installer = "src-tauri\target\release\bundle\nsis\Aster_0.1.0_x64-setup.exe"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dist = Join-Path $root "dist"
$artifacts = @(
    (Join-Path $root $ReleaseBinary),
    (Join-Path $root $Installer)
)

foreach ($artifact in $artifacts) {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) {
        throw "Required package artifact is missing: $artifact"
    }
}
if (-not (Test-Path -LiteralPath $dist -PathType Container)) {
    throw "The production frontend bundle is missing. Run the production build first."
}

$sourceMaps = @(Get-ChildItem -LiteralPath $dist -Recurse -File -Filter "*.map")
if ($sourceMaps.Count -ne 0) {
    throw "Production source maps were found in the frontend bundle."
}

$patterns = [ordered]@{
    BuilderProfilePath = [regex]::Escape($env:USERPROFILE)
    PrivateKey = "BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY"
    AuthorizationHeader = "Authorization: Bearer "
    TestCredential = "test_key_not_a_secret"
    SourceMapDirective = "sourceMappingURL"
    DevelopmentEndpoint = "(?:http|ws)://(?:127\.0\.0\.1|localhost):1420"
}

$findings = [System.Collections.Generic.List[string]]::new()
foreach ($artifact in $artifacts) {
    $bytes = [System.IO.File]::ReadAllBytes($artifact)
    $decoded = @(
        [System.Text.Encoding]::UTF8.GetString($bytes),
        [System.Text.Encoding]::Unicode.GetString($bytes),
        [System.Text.Encoding]::BigEndianUnicode.GetString($bytes)
    )

    foreach ($pattern in $patterns.GetEnumerator()) {
        foreach ($text in $decoded) {
            if ([regex]::IsMatch($text, $pattern.Value)) {
                $findings.Add("$($pattern.Key) in $([System.IO.Path]::GetFileName($artifact))")
                break
            }
        }
    }
}

$bundleFiles = Get-ChildItem -LiteralPath $dist -Recurse -File
foreach ($file in $bundleFiles) {
    $text = [System.IO.File]::ReadAllText($file.FullName)
    foreach ($pattern in $patterns.GetEnumerator()) {
        if ([regex]::IsMatch($text, $pattern.Value)) {
            $findings.Add("$($pattern.Key) in dist/$($file.Name)")
        }
    }
}

if ($findings.Count -ne 0) {
    $summary = $findings | Sort-Object -Unique
    throw "Package audit failed:`n$($summary -join [Environment]::NewLine)"
}

Write-Host "Package audit passed for the release binary, NSIS installer, and production frontend bundle."
