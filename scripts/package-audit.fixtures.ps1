[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$identityModule = Join-Path $PSScriptRoot "Aster.BuildIdentity.psm1"
Import-Module $identityModule -Force
$temporaryRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$sandbox = Join-Path $temporaryRoot ("aster-package-audit-" + [guid]::NewGuid().ToString("N"))
$temporaryPrefix = $temporaryRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar
if (-not $sandbox.StartsWith($temporaryPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The package-audit fixture sandbox escaped the system temporary directory."
}

function Assert-AuditRejects {
    param(
        [Parameter(Mandatory = $true)]
        [string]$AuditScript,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    $rejected = $false
    try {
        & $AuditScript
    }
    catch {
        if ($_.Exception.Message -match "Package audit failed") {
            $rejected = $true
        }
        else {
            throw "$Label failed for an unexpected reason: $($_.Exception.Message)"
        }
    }
    if (-not $rejected) {
        throw "$Label was not detected by the binary package audit."
    }
}

try {
    New-Item -ItemType Directory -Path $sandbox | Out-Null
    foreach ($directory in @("scripts", "dist", "src-tauri\target\release\bundle\nsis")) {
        New-Item -ItemType Directory -Path (Join-Path $sandbox $directory) -Force | Out-Null
    }
    foreach ($scriptName in @("Aster.BuildIdentity.psm1", "package-audit.ps1", "secret-patterns.json")) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $scriptName) -Destination (Join-Path $sandbox "scripts\$scriptName")
    }
    [System.IO.File]::WriteAllText(
        (Join-Path $sandbox "dist\app.js"),
        "console.log('safe fixture');",
        [System.Text.UTF8Encoding]::new($false)
    )
    $releaseBinary = Join-Path $sandbox "src-tauri\target\release\aster-desktop.exe"
    $installer = Join-Path $sandbox "src-tauri\target\release\bundle\nsis\Aster_0.2.0_x64-setup.exe"
    $auditScript = Join-Path $sandbox "scripts\package-audit.ps1"
    [System.IO.File]::WriteAllBytes($installer, [System.Text.Encoding]::ASCII.GetBytes("safe installer fixture"))
    $sensitiveText = "Author" + "ization: Bearer fixture-value"

    foreach ($case in @(
            [pscustomobject]@{ Name = "UTF-16LE offset 0"; Encoding = [System.Text.Encoding]::Unicode; Offset = 0 },
            [pscustomobject]@{ Name = "UTF-16LE offset 1"; Encoding = [System.Text.Encoding]::Unicode; Offset = 1 },
            [pscustomobject]@{ Name = "UTF-16BE offset 0"; Encoding = [System.Text.Encoding]::BigEndianUnicode; Offset = 0 },
            [pscustomobject]@{ Name = "UTF-16BE offset 1"; Encoding = [System.Text.Encoding]::BigEndianUnicode; Offset = 1 }
        )) {
        $payload = $case.Encoding.GetBytes($sensitiveText)
        $bytes = New-Object byte[] ($payload.Length + $case.Offset)
        if ($case.Offset -eq 1) {
            $bytes[0] = 0x7f
        }
        [Array]::Copy($payload, 0, $bytes, $case.Offset, $payload.Length)
        [System.IO.File]::WriteAllBytes($releaseBinary, $bytes)
        Assert-AuditRejects -AuditScript $auditScript -Label $case.Name
    }

    [System.IO.File]::WriteAllBytes($releaseBinary, [System.Text.Encoding]::ASCII.GetBytes("safe release fixture"))
    & $auditScript
    Write-Host "Binary package-audit alignment fixtures passed."
}
finally {
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
