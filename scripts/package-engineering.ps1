[CmdletBinding()]
param(
    [string]$OutputDirectory = "outputs",
    [Parameter(Mandatory = $true)]
    [string]$EvidenceRecord,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9 ._@:+-]{0,119}$')]
    [string]$VerifierIdentity
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$packagingModule = Join-Path $PSScriptRoot "Aster.Packaging.psm1"
Import-Module $packagingModule -Force
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))

Invoke-AsterEngineeringPackage `
    -RepositoryRoot $root `
    -ScriptsRoot $PSScriptRoot `
    -OutputDirectory $OutputDirectory `
    -EvidenceRecord $EvidenceRecord `
    -VerifierIdentity $VerifierIdentity
