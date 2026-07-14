Set-StrictMode -Version Latest

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$identityModule = Join-Path $PSScriptRoot "Aster.BuildIdentity.psm1"
Import-Module $identityModule -Force

function Assert-AsterExactPropertySet {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Value,

        [Parameter(Mandatory = $true)]
        [string[]]$Expected,

        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    if ($null -eq $Value) {
        throw "The build identity has a missing $Label object."
    }
    $actualNames = [string[]]@($Value.PSObject.Properties.Name)
    $expectedNames = [string[]]@($Expected)
    [Array]::Sort($actualNames, [StringComparer]::Ordinal)
    [Array]::Sort($expectedNames, [StringComparer]::Ordinal)
    if (($actualNames -join "`n") -cne ($expectedNames -join "`n")) {
        throw "The build identity has an invalid $Label shape."
    }
}

function Assert-AsterFileIdentity {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [object]$Expected,

        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [string]$Label,

        [switch]$CompareRecordedPath
    )

    Assert-AsterExactPropertySet $Expected @("path", "bytes", "sha256") $Label
    if (
        [string]$Expected.path -notmatch '^[A-Za-z0-9._/+-]+$' -or
        [long]$Expected.bytes -lt 0 -or
        [string]$Expected.sha256 -notmatch '^[0-9a-f]{64}$'
    ) {
        throw "The build identity contains an invalid $Label value."
    }
    $actual = Get-AsterFileIdentity -RepositoryRoot $RepositoryRoot -Path $Path
    if (
        ($CompareRecordedPath -and [string]$Expected.path -cne [string]$actual.path) -or
        [long]$Expected.bytes -ne [long]$actual.bytes -or
        [string]$Expected.sha256 -cne [string]$actual.sha256
    ) {
        throw "The $Label does not match the clean-source build identity. Rebuild it; do not rename or reuse a stale artifact."
    }
}

function Get-AsterStrictBuildIdentity {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Json
    )

    $canonicalSchemaPattern = @'
\A\s*\{
\s*"schemaVersion"\s*:\s*(?<schemaVersion>1)\s*,
\s*"sourceRevision"\s*:\s*"(?<sourceRevision>[0-9a-f]{40,64})"\s*,
\s*"generatedUtc"\s*:\s*"(?<generatedUtc>[^"]+)"\s*,
\s*"artifacts"\s*:\s*\{
\s*"releaseBinary"\s*:\s*\{
\s*"path"\s*:\s*"src-tauri/target/release/aster-desktop\.exe"\s*,
\s*"bytes"\s*:\s*(?<releaseBytes>0|[1-9][0-9]*)\s*,
\s*"sha256"\s*:\s*"(?<releaseSha>[0-9a-f]{64})"\s*\}\s*,
\s*"installer"\s*:\s*\{
\s*"path"\s*:\s*"src-tauri/target/release/bundle/nsis/Aster_0\.2\.0_x64-setup\.exe"\s*,
\s*"bytes"\s*:\s*(?<installerBytes>0|[1-9][0-9]*)\s*,
\s*"sha256"\s*:\s*"(?<installerSha>[0-9a-f]{64})"\s*\}\s*\}\s*,
\s*"sboms"\s*:\s*\{
\s*"frontend"\s*:\s*\{
\s*"path"\s*:\s*"work/sbom-frontend\.cdx\.json"\s*,
\s*"bytes"\s*:\s*(?<frontendBytes>0|[1-9][0-9]*)\s*,
\s*"sha256"\s*:\s*"(?<frontendSha>[0-9a-f]{64})"\s*\}\s*,
\s*"rust"\s*:\s*\{
\s*"path"\s*:\s*"work/sbom-rust\.cdx\.json"\s*,
\s*"bytes"\s*:\s*(?<rustBytes>0|[1-9][0-9]*)\s*,
\s*"sha256"\s*:\s*"(?<rustSha>[0-9a-f]{64})"\s*\}\s*\}\s*,
\s*"frontendDist"\s*:\s*\{
\s*"path"\s*:\s*"dist"\s*,
\s*"sha256"\s*:\s*"(?<distSha>[0-9a-f]{64})"\s*\}\s*\}\s*\z
'@
    $options = [System.Text.RegularExpressions.RegexOptions]::IgnorePatternWhitespace -bor
        [System.Text.RegularExpressions.RegexOptions]::CultureInvariant -bor
        [System.Text.RegularExpressions.RegexOptions]::ExplicitCapture
    $schemaMatch = [regex]::Match($Json, $canonicalSchemaPattern, $options)
    if (-not $schemaMatch.Success) {
        throw "The engineering build identity must use the exact duplicate-free schema and JSON token types."
    }

    $maximumSafeInteger = [uint64]9007199254740991
    foreach ($captureName in @("releaseBytes", "installerBytes", "frontendBytes", "rustBytes")) {
        $parsed = [uint64]0
        if (
            -not [uint64]::TryParse(
                $schemaMatch.Groups[$captureName].Value,
                [System.Globalization.NumberStyles]::None,
                [System.Globalization.CultureInfo]::InvariantCulture,
                [ref]$parsed
            ) -or
            $parsed -gt $maximumSafeInteger
        ) {
            throw "The engineering build identity contains a byte count outside the integral JSON safe range."
        }
    }

    $timestamp = [DateTimeOffset]::MinValue
    if (
        -not [DateTimeOffset]::TryParseExact(
            $schemaMatch.Groups["generatedUtc"].Value,
            "yyyy-MM-ddTHH:mm:ssZ",
            [System.Globalization.CultureInfo]::InvariantCulture,
            [System.Globalization.DateTimeStyles]::AssumeUniversal -bor [System.Globalization.DateTimeStyles]::AdjustToUniversal,
            [ref]$timestamp
        )
    ) {
        throw "The engineering build identity generatedUtc value must be a real canonical UTC timestamp."
    }

    try {
        return $Json | ConvertFrom-Json
    }
    catch {
        throw "The engineering build identity is invalid JSON. Rebuild from a clean source revision."
    }
}

function Assert-AsterGitState {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Git,

        [Parameter(Mandatory = $true)]
        [string]$SourceRevision,

        [Parameter(Mandatory = $true)]
        [string]$Stage
    )

    $revision = (& $Git -C $RepositoryRoot rev-parse --verify HEAD 2>$null).Trim()
    if ($LASTEXITCODE -ne 0 -or $revision -cne $SourceRevision) {
        throw "The source revision changed during $Stage. Packaging was aborted."
    }
    $status = @(& $Git -C $RepositoryRoot status --porcelain=v1 --untracked-files=normal)
    if ($LASTEXITCODE -ne 0) {
        throw "The Git working-tree status could not be inspected during $Stage."
    }
    if ($status.Count -ne 0) {
        throw "Engineering packaging requires a completely clean Git working tree during $Stage."
    }
}

function Get-AsterGitInventory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Git,

        [Parameter(Mandatory = $true)]
        [string]$SourceRevision
    )

    $objectFormat = (& $Git -C $RepositoryRoot rev-parse --show-object-format).Trim().ToLowerInvariant()
    if ($LASTEXITCODE -ne 0 -or @("sha1", "sha256") -notcontains $objectFormat) {
        throw "The Git object format could not be identified."
    }
    $lines = @(& $Git -C $RepositoryRoot -c core.quotePath=false ls-tree -r --full-tree $SourceRevision)
    if ($LASTEXITCODE -ne 0) {
        throw "The tracked source inventory could not be read."
    }

    $entriesByPath = [System.Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
    $hashLength = if ($objectFormat -eq "sha1") { 40 } else { 64 }
    foreach ($line in $lines) {
        if ($line -notmatch ('^(?<mode>[0-9]{6}) blob (?<hash>[0-9a-f]{' + $hashLength + '})\t(?<path>.+)$')) {
            throw "The repository contains an unsupported or ambiguous tracked entry."
        }
        $path = [string]$Matches["path"]
        $mode = [string]$Matches["mode"]
        $blob = [string]$Matches["hash"]
        if (
            [System.IO.Path]::IsPathRooted($path) -or
            $path.Contains("\") -or
            $path -match '(^|/)\.\.(/|$)' -or
            $path.IndexOf([char]0) -ne -1 -or
            $path.IndexOf("`r") -ne -1 -or
            $path.IndexOf("`n") -ne -1 -or
            $path.IndexOf("`t") -ne -1
        ) {
            throw "The repository contains a tracked path that cannot be represented safely in the handoff inventory."
        }
        if ($entriesByPath.ContainsKey($path)) {
            throw "The tracked source inventory contains a duplicate path: $path"
        }
        $entriesByPath.Add($path, [pscustomobject][ordered]@{
                path = $path
                mode = $mode
                blob = $blob
            })
    }
    $paths = [string[]]@($entriesByPath.Keys)
    [Array]::Sort($paths, [StringComparer]::Ordinal)
    $entries = foreach ($path in $paths) {
        $entriesByPath[$path]
    }
    return [pscustomobject][ordered]@{
        schemaVersion  = 1
        sourceRevision = $SourceRevision
        objectFormat   = $objectFormat
        entries        = @($entries)
    }
}

function Get-AsterGitBlobHashFromStream {
    param(
        [Parameter(Mandatory = $true)]
        [System.IO.Stream]$Stream,

        [Parameter(Mandatory = $true)]
        [long]$Length,

        [Parameter(Mandatory = $true)]
        [ValidateSet("sha1", "sha256")]
        [string]$ObjectFormat
    )

    $algorithm = if ($ObjectFormat -eq "sha1") {
        [System.Security.Cryptography.SHA1]::Create()
    }
    else {
        [System.Security.Cryptography.SHA256]::Create()
    }
    try {
        $header = [System.Text.Encoding]::ASCII.GetBytes("blob $Length`0")
        [void]$algorithm.TransformBlock($header, 0, $header.Length, $header, 0)
        $buffer = New-Object byte[] 65536
        $total = [long]0
        while (($read = $Stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $total += $read
            [void]$algorithm.TransformBlock($buffer, 0, $read, $buffer, 0)
        }
        if ($total -ne $Length) {
            throw "A source entry changed length while its Git identity was calculated."
        }
        [void]$algorithm.TransformFinalBlock((New-Object byte[] 0), 0, 0)
        return ([System.BitConverter]::ToString($algorithm.Hash) -replace "-", "").ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

function Get-AsterGitBlobHashForFile {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [ValidateSet("sha1", "sha256")]
        [string]$ObjectFormat
    )

    $absolute = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Path
    if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
        throw "The controlled package file is missing: $absolute"
    }
    $item = Get-Item -LiteralPath $absolute -Force
    $stream = [System.IO.FileStream]::new(
        $absolute,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::Read
    )
    try {
        return Get-AsterGitBlobHashFromStream -Stream $stream -Length $item.Length -ObjectFormat $ObjectFormat
    }
    finally {
        $stream.Dispose()
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $absolute)
    }
}

function Assert-AsterControlledCopy {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [object]$Inventory,

        [Parameter(Mandatory = $true)]
        [string]$TrackedPath,

        [Parameter(Mandatory = $true)]
        [string]$CopiedPath
    )

    $matches = @($Inventory.entries | Where-Object { [string]$_.path -ceq $TrackedPath })
    if ($matches.Count -ne 1) {
        throw "The controlled package file is not present exactly once in the tracked source inventory: $TrackedPath"
    }
    $actualBlob = Get-AsterGitBlobHashForFile `
        -RepositoryRoot $RepositoryRoot `
        -Path $CopiedPath `
        -ObjectFormat $Inventory.objectFormat
    if ($actualBlob -cne [string]$matches[0].blob) {
        throw "The copied controlled source file does not match the identified Git revision: $TrackedPath"
    }
}

function Set-AsterZipModesFromInventory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$ArchivePath,

        [Parameter(Mandatory = $true)]
        [object]$Inventory
    )

    $archive = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $ArchivePath
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $modeByPath = [System.Collections.Generic.Dictionary[string, int]]::new([StringComparer]::Ordinal)
    foreach ($tracked in $Inventory.entries) {
        $modeByPath.Add([string]$tracked.path, [Convert]::ToInt32([string]$tracked.mode, 8))
    }
    $directoryMode = [Convert]::ToInt32("040755", 8)
    $zip = [System.IO.Compression.ZipFile]::Open($archive, [System.IO.Compression.ZipArchiveMode]::Update)
    try {
        foreach ($entry in $zip.Entries) {
            $unixMode = if ($entry.FullName.EndsWith("/", [StringComparison]::Ordinal)) {
                $directoryMode
            }
            elseif ($modeByPath.ContainsKey($entry.FullName)) {
                $modeByPath[$entry.FullName]
            }
            else {
                throw "The source archive contains an entry absent from the tracked inventory before mode canonicalization: $($entry.FullName)"
            }
            $dosAttributes = if ($entry.FullName.EndsWith("/", [StringComparison]::Ordinal)) { 0x10 } else { 0 }
            $externalUnsigned = [uint32]((([uint64]$unixMode) -shl 16) -bor [uint64]$dosAttributes)
            $entry.ExternalAttributes = [BitConverter]::ToInt32([BitConverter]::GetBytes($externalUnsigned), 0)
        }
    }
    finally {
        $zip.Dispose()
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $archive)
    }
}

function Assert-AsterZipInventory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$ArchivePath,

        [Parameter(Mandatory = $true)]
        [object]$Inventory
    )

    $archive = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $ArchivePath
    if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
        throw "The tracked source archive is missing."
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $expected = [System.Collections.Generic.Dictionary[string, object]]::new([StringComparer]::Ordinal)
    $expectedDirectories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $Inventory.entries) {
        $trackedPath = [string]$entry.path
        $expected.Add($trackedPath, $entry)
        $segments = @($trackedPath -split '/')
        if ($segments.Count -gt 1) {
            for ($segmentCount = 1; $segmentCount -lt $segments.Count; $segmentCount++) {
                [void]$expectedDirectories.Add((($segments[0..($segmentCount - 1)] -join '/') + '/'))
            }
        }
    }
    $observed = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $observedDirectories = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $canonicalDirectoryMode = [Convert]::ToInt32("040755", 8)
    $zip = [System.IO.Compression.ZipFile]::OpenRead($archive)
    try {
        foreach ($entry in $zip.Entries) {
            $name = $entry.FullName
            $isDirectory = $name.EndsWith("/", [StringComparison]::Ordinal)
            $pathWithoutDirectoryMarker = if ($isDirectory) { $name.Substring(0, $name.Length - 1) } else { $name }
            $segments = @($pathWithoutDirectoryMarker -split '/')
            if (
                [string]::IsNullOrWhiteSpace($name) -or
                [string]::IsNullOrWhiteSpace($pathWithoutDirectoryMarker) -or
                [System.IO.Path]::IsPathRooted($name) -or
                $name.Contains("\") -or
                @($segments | Where-Object { $_ -eq "" -or $_ -eq "." -or $_ -eq ".." }).Count -ne 0
            ) {
                throw "The source archive contains an unexpected, unsafe, or duplicate path: $name"
            }

            $externalAttributes = [uint32]([int64]$entry.ExternalAttributes -band 0xffffffffL)
            $unixMode = [int](($externalAttributes -shr 16) -band 0xffff)
            $unixFileType = $unixMode -band 0xf000
            $dosDirectory = ($externalAttributes -band 0x10) -ne 0
            if ($isDirectory) {
                if (
                    -not $expectedDirectories.Contains($name) -or
                    -not $observedDirectories.Add($name) -or
                    $entry.Length -ne 0 -or
                    $unixFileType -ne 0x4000 -or
                    $unixMode -ne $canonicalDirectoryMode -or
                    -not $dosDirectory
                ) {
                    throw "The source archive contains an unexpected directory or invalid directory attributes: $name"
                }
                continue
            }
            if (
                -not $expected.ContainsKey($name) -or
                -not $observed.Add($name) -or
                $dosDirectory
            ) {
                throw "The source archive contains an unexpected, unsafe, or duplicate path: $name"
            }
            $expectedUnixMode = [Convert]::ToInt32([string]$expected[$name].mode, 8)
            if ($unixMode -ne $expectedUnixMode) {
                throw "The source archive entry mode does not match the tracked Git mode: $name"
            }
            $stream = $entry.Open()
            try {
                $blob = Get-AsterGitBlobHashFromStream `
                    -Stream $stream `
                    -Length $entry.Length `
                    -ObjectFormat $Inventory.objectFormat
            }
            finally {
                $stream.Dispose()
            }
            if ($blob -cne [string]$expected[$name].blob) {
                throw "The source archive entry does not match the identified Git object: $name"
            }
        }
    }
    finally {
        $zip.Dispose()
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $archive)
    }
    if ($observed.Count -ne $expected.Count) {
        $missing = @($expected.Keys | Where-Object { -not $observed.Contains($_) })
        $missingNames = [string[]]$missing
        [Array]::Sort($missingNames, [StringComparer]::Ordinal)
        throw "The source archive is missing tracked entries: $($missingNames -join ', ')"
    }
    if ($observedDirectories.Count -ne $expectedDirectories.Count) {
        $missingDirectories = @($expectedDirectories | Where-Object { -not $observedDirectories.Contains($_) })
        $missingDirectoryNames = [string[]]$missingDirectories
        [Array]::Sort($missingDirectoryNames, [StringComparer]::Ordinal)
        throw "The source archive directory inventory is incomplete: $($missingDirectoryNames -join ', ')"
    }
}

function Get-AsterEvidenceText {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$EvidencePath,

        [Parameter(Mandatory = $true)]
        [string]$SourceRevision,

        [Parameter(Mandatory = $true)]
        [string]$SharedPatternPath
    )

    $report = Read-AsterStrictUtf8Text `
        -RepositoryRoot $RepositoryRoot `
        -Path $EvidencePath `
        -MaximumBytes 262144
    if ($report.IndexOf([char]0) -ne -1 -or [regex]::IsMatch($report, '[\x00-\x08\x0b\x0c\x0e-\x1f]')) {
        throw "The evidence record contains prohibited control characters."
    }

    $evidenceLogPath = Resolve-AsterContainedPath `
        -RepositoryRoot $RepositoryRoot `
        -Path ([System.IO.Path]::ChangeExtension($EvidencePath, ".log"))
    if (-not (Test-Path -LiteralPath $evidenceLogPath -PathType Leaf)) {
        throw "The evidence record requires a retained sibling log."
    }
    $evidenceRoot = [System.IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $evidenceRootPrefix = $evidenceRoot + [System.IO.Path]::DirectorySeparatorChar
    if (-not $evidenceLogPath.StartsWith($evidenceRootPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "The retained evidence log escaped the repository root."
    }
    $evidenceLogRelative = $evidenceLogPath.Substring($evidenceRootPrefix.Length).Replace("\", "/")
    if ($evidenceLogRelative -notmatch '^work/evidence/\d{4}-\d{2}-\d{2}-[0-9a-f]{40,64}-engineering-build\.log$') {
        throw "The retained evidence log must use the canonical ignored work/evidence path."
    }
    $evidenceLog = Read-AsterStrictUtf8Text `
        -RepositoryRoot $RepositoryRoot `
        -Path $evidenceLogPath `
        -MaximumBytes 4194304
    if (
        [string]::IsNullOrWhiteSpace($evidenceLog) -or
        $evidenceLog.IndexOf([char]0) -ne -1 -or
        [regex]::IsMatch($evidenceLog, '[\x00-\x08\x0b\x0c\x0e-\x1f]')
    ) {
        throw "The retained evidence log is empty or contains prohibited control characters."
    }

    $sourceLines = [regex]::Matches($report, '(?im)^\*\*Source revision:\*\*.*\r?$')
    $expectedLine = "**Source revision:** $SourceRevision"
    if ($sourceLines.Count -ne 1 -or $sourceLines[0].Value.TrimEnd("`r") -cne $expectedLine) {
        throw "The evidence record must contain exactly one canonical full Source revision line for the build revision."
    }

    $classificationLines = [regex]::Matches($report, '(?im)^\*\*Overall classification:\*\*.*\r?$')
    $expectedClassification = "**Overall classification:** Unsigned engineering build for local evaluation"
    if (
        $classificationLines.Count -ne 1 -or
        $classificationLines[0].Value.TrimEnd("`r") -cne $expectedClassification
    ) {
        throw "The engineering package evidence must contain exactly one canonical unsigned engineering classification."
    }

    $requiredPatterns = [ordered]@{
        "title" = '(?m)^# Verification Record: .+$'
        "results table" = '(?m)^\| Criterion/gate \| Outcome \| Source revision/state \| Environment \| Started/completed UTC \| Procedure identity \| Exact command or procedure \| Evidence \| Artifact/hash \| Scope/notes \|\s*$'
        "results separator" = '(?m)^\| --- \| --- \| --- \| --- \| --- \| --- \| --- \| --- \| --- \| --- \|\s*$'
        "failure history" = '(?m)^## Failures and retained retries\s*$'
        "residual risk" = '(?m)^## Residual risk and exceptions\s*$'
    }
    foreach ($required in $requiredPatterns.GetEnumerator()) {
        if (-not [regex]::IsMatch($report, $required.Value)) {
            throw "The evidence record is missing its required $($required.Key) structure."
        }
    }

    $fieldPatterns = [ordered]@{
        Artifact = '(?m)^\*\*Artifact:\*\* (?<value>[^\r\n]+)\r?$'
        Environment = '(?m)^\*\*Environment:\*\* (?<value>[^\r\n]+)\r?$'
        StartedUtc = '(?m)^\*\*Started UTC:\*\* (?<value>[^\r\n]+)\r?$'
        CompletedUtc = '(?m)^\*\*Completed UTC:\*\* (?<value>[^\r\n]+)\r?$'
        ProcedureIdentity = '(?m)^\*\*Procedure identity \(self-declared\):\*\* (?<value>[^\r\n]+)\r?$'
    }
    $fieldValues = @{}
    foreach ($field in $fieldPatterns.GetEnumerator()) {
        $matches = [regex]::Matches($report, $field.Value)
        if ($matches.Count -ne 1) {
            throw "The evidence record must contain exactly one complete $($field.Key) field."
        }
        $value = $matches[0].Groups["value"].Value.Trim()
        if ($value.Length -lt 3 -or $value -match '^(?:TBD|TODO|<.+>)$') {
            throw "The evidence record contains an incomplete $($field.Key) field."
        }
        $fieldValues[$field.Key] = $value
    }
    $artifactPattern = '(?i)^(?:Not applicable\b.*|.+\bSHA-256:\s*[0-9a-f]{64}\b.*)$'
    if ($fieldValues.Artifact -notmatch $artifactPattern) {
        throw "The evidence Artifact field must say Not applicable or include the tested artifact's SHA-256."
    }
    $startedUtc = [DateTimeOffset]::MinValue
    $completedUtc = [DateTimeOffset]::MinValue
    $timestampStyles = [System.Globalization.DateTimeStyles]::AssumeUniversal -bor
        [System.Globalization.DateTimeStyles]::AdjustToUniversal
    if (
        -not [DateTimeOffset]::TryParseExact(
            $fieldValues.StartedUtc,
            "yyyy-MM-ddTHH:mm:ssZ",
            [System.Globalization.CultureInfo]::InvariantCulture,
            $timestampStyles,
            [ref]$startedUtc
        ) -or
        -not [DateTimeOffset]::TryParseExact(
            $fieldValues.CompletedUtc,
            "yyyy-MM-ddTHH:mm:ssZ",
            [System.Globalization.CultureInfo]::InvariantCulture,
            $timestampStyles,
            [ref]$completedUtc
        ) -or
        $completedUtc -lt $startedUtc
    ) {
        throw "The evidence record must contain real ordered canonical UTC timestamps."
    }

    $resultPattern = '(?m)^\| `(?<gate>AC-[0-9]{3}|[A-Za-z0-9](?:[A-Za-z0-9 ._:/+-]{0,118}[A-Za-z0-9])?)`\s*\| `(?<outcome>PASS|FAIL|NOT RUN)`\s*\| (?<source>[^|]+) \| (?<environment>[^|]+) \| (?<time>[^|]+) \| (?<identity>[^|]+) \| (?<procedure>[^|]+) \| (?<evidence>[^|]+) \| (?<artifact>[^|]+) \| (?<notes>[^|]+) \|\s*$'
    $resultRows = [regex]::Matches($report, $resultPattern)
    $tableLines = [regex]::Matches($report, '(?m)^[\t ]*\|[^\r\n]*\|\s*$')
    if ($tableLines.Count -ne ($resultRows.Count + 2)) {
        throw "The evidence record contains a malformed or unvalidated result row."
    }
    if ($resultRows.Count -eq 0 -or $resultRows.Count -gt 256) {
        throw "The evidence record must contain between one and 256 complete result rows."
    }
    $expectedRowSource = "$SourceRevision clean"
    $expectedRowTime = "$($fieldValues.StartedUtc) to $($fieldValues.CompletedUtc)"
    $observedGates = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($row in $resultRows) {
        $gate = $row.Groups["gate"].Value
        if (-not $observedGates.Add($gate)) {
            throw "The evidence record contains more than one current result for gate $gate."
        }
        foreach ($column in @("source", "environment", "time", "identity", "procedure", "evidence", "artifact", "notes")) {
            $value = $row.Groups[$column].Value.Trim()
            if ($value.Length -lt 3 -or $value -match '^(?:TBD|TODO|<.+>)$') {
                throw "Every evidence result must contain complete source, environment, time, identity, an exact procedure, evidence, artifact, and scope fields."
            }
        }
        if (
            $row.Groups["source"].Value.Trim() -cne $expectedRowSource -or
            $row.Groups["environment"].Value.Trim() -cne $fieldValues.Environment -or
            $row.Groups["time"].Value.Trim() -cne $expectedRowTime -or
            $row.Groups["identity"].Value.Trim() -cne $fieldValues.ProcedureIdentity
        ) {
            throw "Every evidence result must repeat the canonical source, environment, UTC window, and procedure identity."
        }
        if ($row.Groups["artifact"].Value.Trim() -notmatch $artifactPattern) {
            throw "Every evidence result Artifact/hash cell must say Not applicable or include a SHA-256."
        }
        if (
            $row.Groups["outcome"].Value -ceq "PASS" -and
            $row.Groups["evidence"].Value.Trim() -cne $evidenceLogRelative
        ) {
            throw "A PASS result must identify the canonical retained evidence log."
        }
    }

    $scanReport = $report + "`n" + $evidenceLog
    for ($normalizationPass = 0; $normalizationPass -lt 3; $normalizationPass++) {
        $normalized = [regex]::Replace($scanReport, '(?i)\\u005c', '\')
        $normalized = [regex]::Replace($normalized, '(?i)\\u002f', '/')
        $normalized = $normalized.Replace('\/', '/').Replace('\\', '\')
        if ($normalized -ceq $scanReport) {
            break
        }
        $scanReport = $normalized
    }
    $sensitivePatterns = [ordered]@{
        "Windows user profile path" = '(?i)\b[A-Z]:[\\/]+Users[\\/]+[^\\/\r\n"'']+'
        "macOS user profile path" = '(?i)(?:^|[\s"''(])/Users/[^/\s"'')/]+'
        "Linux home path" = '(?i)(?:^|[\s"''(])/home/[^/\s"'')/]+'
        "authorization or credential header" = '(?im)^[\t ]*(?:(?:>[\t ]*)|(?:[-+*][\t ]+)|(?:[0-9]+[.)][\t ]+))*(?:\*\*|__|`|~~)?[\t ]*["'']?(?:Authorization|Proxy-Authorization|X-Api-Key|Api-Key|Cookie|Set-Cookie)["'']?[\t ]*(?:\*\*|__|`|~~)?[\t ]*:'
    }
    if (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
        $sensitivePatterns["current builder profile path"] = [regex]::Escape($env:USERPROFILE)
    }
    $sharedPatternText = Read-AsterStrictUtf8Text `
        -RepositoryRoot $RepositoryRoot `
        -Path $SharedPatternPath `
        -MaximumBytes 65536
    try {
        $sharedPatterns = $sharedPatternText | ConvertFrom-Json
    }
    catch {
        throw "The shared secret-pattern file is invalid JSON."
    }
    foreach ($rule in @($sharedPatterns)) {
        if ($null -eq $rule.PSObject.Properties["name"] -or $null -eq $rule.PSObject.Properties["pattern"]) {
            throw "The shared secret-pattern file has an invalid rule."
        }
        $flags = if ($null -ne $rule.PSObject.Properties["flags"]) { [string]$rule.flags } else { "" }
        $prefix = if ($flags.Contains("i")) { "(?i)" } else { "" }
        $sensitivePatterns["shared secret pattern: $($rule.name)"] = $prefix + [string]$rule.pattern
    }
    foreach ($pattern in $sensitivePatterns.GetEnumerator()) {
        if ([regex]::IsMatch($scanReport, $pattern.Value)) {
            throw "The evidence record matches a prohibited sensitive-data pattern: $($pattern.Key)."
        }
    }

    return [pscustomobject]@{
        Report = $report
        EvidenceLog = $evidenceLog
        EvidenceLogRelative = $evidenceLogRelative
    }
}

function Assert-AsterPackageAllowlist {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Directory,

        [Parameter(Mandatory = $true)]
        [string[]]$ExpectedNames
    )

    $safeDirectory = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Directory
    if (-not (Test-Path -LiteralPath $safeDirectory -PathType Container)) {
        throw "The package assembly directory is missing."
    }
    $expected = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $ExpectedNames) {
        if (-not $expected.Add($name)) {
            throw "The package allowlist contains a duplicate name: $name"
        }
    }
    $observed = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in @(Get-ChildItem -LiteralPath $safeDirectory -Force)) {
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $entry.FullName)
        if ($entry.PSIsContainer -or -not $expected.Contains($entry.Name) -or -not $observed.Add($entry.Name)) {
            throw "The package contains an unexpected directory, file, or duplicate name: $($entry.Name)"
        }
    }
    if ($observed.Count -ne $expected.Count) {
        $missing = @($expected | Where-Object { -not $observed.Contains($_) })
        throw "The package is missing allowlisted files: $($missing -join ', ')"
    }
}

function Write-AsterChecksums {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Directory,

        [Parameter(Mandatory = $true)]
        [string]$ChecksumPath
    )

    $files = @(Get-AsterSafeDirectoryFiles -RepositoryRoot $RepositoryRoot -Path $Directory | Where-Object {
            [System.IO.Path]::GetFileName($_) -cne "SHA256SUMS.txt"
        })
    $names = [string[]]@($files | ForEach-Object { [System.IO.Path]::GetFileName($_) })
    [Array]::Sort($names, [StringComparer]::Ordinal)
    $lineByName = [System.Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
    foreach ($file in $files) {
        $name = [System.IO.Path]::GetFileName($file)
        $hash = (Get-AsterFileIdentity -RepositoryRoot $RepositoryRoot -Path $file).sha256
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $file)
        $lineByName.Add($name, "$hash  $name")
    }
    $lines = foreach ($name in $names) {
        $lineByName[$name]
    }
    Write-AsterUtf8Text `
        -RepositoryRoot $RepositoryRoot `
        -Path $ChecksumPath `
        -Content (($lines -join "`n") + "`n") | Out-Null
}

function Assert-AsterChecksums {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Directory,

        [Parameter(Mandatory = $true)]
        [string]$ChecksumPath
    )

    $text = Read-AsterStrictUtf8Text -RepositoryRoot $RepositoryRoot -Path $ChecksumPath -MaximumBytes 65536
    $expectedByName = [System.Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
    foreach ($file in @(Get-AsterSafeDirectoryFiles -RepositoryRoot $RepositoryRoot -Path $Directory)) {
        $name = [System.IO.Path]::GetFileName($file)
        if ($name -cne "SHA256SUMS.txt") {
            $expectedByName.Add(
                $name,
                (Get-AsterFileIdentity -RepositoryRoot $RepositoryRoot -Path $file).sha256
            )
            [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $file)
        }
    }
    $observed = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($line in @($text -split "`n")) {
        $canonical = $line.TrimEnd("`r")
        if ($canonical.Length -eq 0) {
            continue
        }
        if ($canonical -notmatch '^(?<hash>[0-9a-f]{64})  (?<name>[^/\\]+)$') {
            throw "The checksum inventory contains a malformed line."
        }
        $name = [string]$Matches["name"]
        if (-not $expectedByName.ContainsKey($name) -or -not $observed.Add($name)) {
            throw "The checksum inventory contains an unexpected or duplicate file: $name"
        }
        if ([string]$Matches["hash"] -cne $expectedByName[$name]) {
            throw "The checksum inventory does not match the final package file: $name"
        }
    }
    if ($observed.Count -ne $expectedByName.Count) {
        throw "The checksum inventory does not cover the exact final package allowlist."
    }
}

function Invoke-AsterEngineeringPackage {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$ScriptsRoot,

        [string]$OutputDirectory = "outputs",

        [Parameter(Mandatory = $true)]
        [string]$EvidenceRecord,

        [Parameter(Mandatory = $true)]
        [ValidatePattern('^[A-Za-z0-9][A-Za-z0-9 ._@:+-]{0,119}$')]
        [string]$VerifierIdentity,

        [scriptblock]$FixtureAfterCopyHook,

        [scriptblock]$FixtureBeforeFinalStateHook,

        [scriptblock]$FixtureBeforePublicationHook
    )

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    [void](Assert-AsterNoReparseAncestors -Path $root)
    $safeScriptsRoot = Resolve-AsterContainedPath -RepositoryRoot $root -Path $ScriptsRoot
    if (-not (Test-Path -LiteralPath $safeScriptsRoot -PathType Container)) {
        throw "The packaging scripts directory is missing."
    }
    $git = (Get-Command git.exe -ErrorAction Stop).Source

    if ([System.IO.Path]::IsPathRooted($EvidenceRecord) -or $EvidenceRecord -match '(^|[\\/])\.\.([\\/]|$)') {
        throw "EvidenceRecord must be a repository-relative path without parent traversal."
    }
    $evidenceRelative = $EvidenceRecord.Replace("\", "/")
    if ($evidenceRelative -notmatch '^work/evidence/\d{4}-\d{2}-\d{2}-([0-9a-f]{40,64})-engineering-build\.md$') {
        throw "EvidenceRecord must match work/evidence/YYYY-MM-DD-<full-source-revision>-engineering-build.md."
    }
    $evidenceRevision = [string]$Matches[1]
    if ([System.IO.Path]::IsPathRooted($OutputDirectory) -or $OutputDirectory -match '(^|[\\/])\.\.([\\/]|$)') {
        throw "OutputDirectory must be a repository-relative path without parent traversal."
    }
    $outputRelative = $OutputDirectory.Replace("\", "/").TrimEnd("/")
    if ([string]::IsNullOrWhiteSpace($outputRelative) -or $outputRelative -eq ".") {
        throw "OutputDirectory must name a contained handoff directory, not the repository root."
    }

    $topLevel = & $git -C $root rev-parse --show-toplevel 2>$null
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($topLevel)) {
        throw "Engineering packaging requires an initialized Git repository."
    }
    $safeTopLevel = [System.IO.Path]::GetFullPath($topLevel.Trim())
    [void](Assert-AsterNoReparseAncestors -Path $safeTopLevel)
    if (-not [string]::Equals($safeTopLevel, $root, [StringComparison]::OrdinalIgnoreCase)) {
        throw "The packaging script must run from the repository that contains this source tree."
    }

    $sourceRevision = (& $git -C $root rev-parse --verify HEAD).Trim().ToLowerInvariant()
    if ($LASTEXITCODE -ne 0 -or $sourceRevision -notmatch '^[0-9a-f]{40,64}$') {
        throw "The source revision could not be identified."
    }
    if ($evidenceRevision -cne $sourceRevision) {
        throw "The evidence-record filename must contain the complete current source revision."
    }
    Assert-AsterGitState -RepositoryRoot $root -Git $git -SourceRevision $sourceRevision -Stage "package preflight"

    $output = Resolve-AsterContainedPath -RepositoryRoot $root -Path (Join-Path $root $outputRelative)
    if (Test-Path -LiteralPath $output) {
        throw "OutputDirectory must not already exist; package publication always targets a fresh directory."
    }
    $outputParent = [System.IO.Directory]::GetParent($output).FullName
    [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $outputParent)
    if (-not (Test-Path -LiteralPath $outputParent -PathType Container)) {
        throw "The OutputDirectory parent must already exist and must not be a reparse point."
    }
    & $git -C $root check-ignore -q -- "$outputRelative/.aster-output-probe"
    if ($LASTEXITCODE -ne 0) {
        throw "OutputDirectory must be covered by the repository ignore policy."
    }
    & $git -C $root check-ignore -q -- "work/package-staging/.aster-staging-probe"
    if ($LASTEXITCODE -ne 0) {
        throw "The package staging directory must be covered by the repository ignore policy."
    }

    $releaseBinary = Join-Path $root "src-tauri\target\release\aster-desktop.exe"
    $installer = Join-Path $root "src-tauri\target\release\bundle\nsis\Aster_0.2.0_x64-setup.exe"
    $frontendSbom = Join-Path $root "work\sbom-frontend.cdx.json"
    $rustSbom = Join-Path $root "work\sbom-rust.cdx.json"
    $buildIdentityPath = Join-Path $root "work\build-identity.json"
    $license = Join-Path $root "LICENSE"
    $notice = Join-Path $root "NOTICE"
    $preview = Join-Path $root "assets\Aster-MVP-v2-preview.png"
    $evidence = Resolve-AsterContainedPath -RepositoryRoot $root -Path (Join-Path $root $evidenceRelative)
    $evidenceLogRelative = [System.IO.Path]::ChangeExtension($evidenceRelative, ".log")
    $evidenceLog = Resolve-AsterContainedPath -RepositoryRoot $root -Path (Join-Path $root $evidenceLogRelative)
    $sharedPatternPath = Join-Path $safeScriptsRoot "secret-patterns.json"
    $packageAudit = Join-Path $safeScriptsRoot "package-audit.ps1"

    foreach ($required in @(
            $releaseBinary,
            $installer,
            $frontendSbom,
            $rustSbom,
            $buildIdentityPath,
            $license,
            $notice,
            $preview,
            $evidence,
            $evidenceLog,
            $sharedPatternPath,
            $packageAudit
        )) {
        $safeRequired = Resolve-AsterContainedPath -RepositoryRoot $root -Path $required
        if (-not (Test-Path -LiteralPath $safeRequired -PathType Leaf)) {
            throw "Required packaging input is missing: $safeRequired"
        }
    }

    $trackedEvidence = @(& $git -C $root ls-files -- $evidenceRelative $evidenceLogRelative)
    if ($LASTEXITCODE -ne 0) {
        throw "The evidence path could not be compared with the tracked source inventory."
    }
    if ($trackedEvidence.Count -ne 0) {
        throw "The packaging evidence input must remain outside the source revision under ignored work/evidence/."
    }
    & $git -C $root check-ignore -q -- $evidenceRelative
    if ($LASTEXITCODE -ne 0) {
        throw "The packaging evidence input must be isolated under the ignored work/evidence/ directory."
    }
    & $git -C $root check-ignore -q -- $evidenceLogRelative
    if ($LASTEXITCODE -ne 0) {
        throw "The retained evidence log must be isolated under the ignored work/evidence/ directory."
    }

    $manifestText = Read-AsterStrictUtf8Text -RepositoryRoot $root -Path $buildIdentityPath -MaximumBytes 1048576
    $buildIdentity = Get-AsterStrictBuildIdentity -Json $manifestText
    Assert-AsterExactPropertySet $buildIdentity @("schemaVersion", "sourceRevision", "generatedUtc", "artifacts", "sboms", "frontendDist") "top-level"
    if (
        [int]$buildIdentity.schemaVersion -ne 1 -or
        [string]$buildIdentity.sourceRevision -cne $sourceRevision -or
        [string]$buildIdentity.generatedUtc -notmatch '^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$'
    ) {
        throw "The engineering build identity does not match the current clean source revision."
    }
    Assert-AsterExactPropertySet $buildIdentity.artifacts @("releaseBinary", "installer") "artifact"
    Assert-AsterExactPropertySet $buildIdentity.sboms @("frontend", "rust") "SBOM"
    Assert-AsterExactPropertySet $buildIdentity.frontendDist @("path", "sha256") "frontend-dist"
    Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.artifacts.releaseBinary -Path $releaseBinary -Label "release binary" -CompareRecordedPath
    Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.artifacts.installer -Path $installer -Label "NSIS installer" -CompareRecordedPath
    Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.sboms.frontend -Path $frontendSbom -Label "frontend SBOM" -CompareRecordedPath
    Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.sboms.rust -Path $rustSbom -Label "Rust SBOM" -CompareRecordedPath
    if (
        [string]$buildIdentity.frontendDist.path -cne "dist" -or
        [string]$buildIdentity.frontendDist.sha256 -notmatch '^[0-9a-f]{64}$' -or
        [string]$buildIdentity.frontendDist.sha256 -cne (Get-AsterDirectoryDigest -RepositoryRoot $root -Path (Join-Path $root "dist"))
    ) {
        throw "The production frontend bundle does not match the clean-source build identity."
    }

    $validatedEvidence = Get-AsterEvidenceText `
        -RepositoryRoot $root `
        -EvidencePath $evidence `
        -SourceRevision $sourceRevision `
        -SharedPatternPath $sharedPatternPath
    $report = [string]$validatedEvidence.Report
    $validatedEvidenceLog = [string]$validatedEvidence.EvidenceLog
    $inventory = Get-AsterGitInventory -RepositoryRoot $root -Git $git -SourceRevision $sourceRevision
    foreach ($controlled in @(
            @{ Source = $license; Tracked = "LICENSE" },
            @{ Source = $notice; Tracked = "NOTICE" },
            @{ Source = $preview; Tracked = "assets/Aster-MVP-v2-preview.png" }
        )) {
        Assert-AsterControlledCopy `
            -RepositoryRoot $root `
            -Inventory $inventory `
            -TrackedPath $controlled.Tracked `
            -CopiedPath $controlled.Source
    }
    $manifestIdentity = [pscustomobject](Get-AsterFileIdentity -RepositoryRoot $root -Path $buildIdentityPath)

    & $packageAudit
    if ($LASTEXITCODE -ne 0) {
        throw "Package input audit failed with exit code $LASTEXITCODE."
    }

    $stagingParent = Join-Path $root "work\package-staging"
    New-AsterSafeDirectory -RepositoryRoot $root -Path $stagingParent | Out-Null
    $staging = Join-Path $stagingParent ([guid]::NewGuid().ToString("N"))
    New-AsterSafeDirectory -RepositoryRoot $root -Path $staging | Out-Null
    $published = $false
    try {
        $paths = [ordered]@{
            portableOutput = Join-Path $staging "Aster-0.2.0-x64-engineering.exe"
            installerOutput = Join-Path $staging "Aster-0.2.0-x64-engineering-setup.exe"
            frontendSbomOutput = Join-Path $staging "sbom-frontend.cdx.json"
            rustSbomOutput = Join-Path $staging "sbom-rust.cdx.json"
            licenseOutput = Join-Path $staging "LICENSE"
            noticeOutput = Join-Path $staging "NOTICE"
            previewOutput = Join-Path $staging "Aster-MVP-v2-preview.png"
            buildIdentityOutput = Join-Path $staging "build-identity.json"
            sourceArchive = Join-Path $staging "Aster-MVP-v2-source.zip"
            sourceInventory = Join-Path $staging "source-inventory.json"
            verification = Join-Path $staging "verification-report.md"
            verificationEvidence = Join-Path $staging "verification-evidence.log"
            checksum = Join-Path $staging "SHA256SUMS.txt"
        }

        Copy-AsterFileSafely -RepositoryRoot $root -Source $releaseBinary -Destination $paths.portableOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $installer -Destination $paths.installerOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $frontendSbom -Destination $paths.frontendSbomOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $rustSbom -Destination $paths.rustSbomOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $license -Destination $paths.licenseOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $notice -Destination $paths.noticeOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $preview -Destination $paths.previewOutput | Out-Null
        Copy-AsterFileSafely -RepositoryRoot $root -Source $buildIdentityPath -Destination $paths.buildIdentityOutput | Out-Null
        Write-AsterUtf8Text `
            -RepositoryRoot $root `
            -Path $paths.verificationEvidence `
            -Content $validatedEvidenceLog | Out-Null

        if ($null -ne $FixtureAfterCopyHook) {
            & $FixtureAfterCopyHook $paths
        }

        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.artifacts.releaseBinary -Path $paths.portableOutput -Label "copied release binary"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.artifacts.installer -Path $paths.installerOutput -Label "copied NSIS installer"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.sboms.frontend -Path $paths.frontendSbomOutput -Label "copied frontend SBOM"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.sboms.rust -Path $paths.rustSbomOutput -Label "copied Rust SBOM"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $manifestIdentity -Path $paths.buildIdentityOutput -Label "copied build identity"
        Assert-AsterControlledCopy -RepositoryRoot $root -Inventory $inventory -TrackedPath "LICENSE" -CopiedPath $paths.licenseOutput
        Assert-AsterControlledCopy -RepositoryRoot $root -Inventory $inventory -TrackedPath "NOTICE" -CopiedPath $paths.noticeOutput
        Assert-AsterControlledCopy -RepositoryRoot $root -Inventory $inventory -TrackedPath "assets/Aster-MVP-v2-preview.png" -CopiedPath $paths.previewOutput

        [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $paths.sourceArchive)
        $archiveOutputArgument = "--output=$($paths.sourceArchive)"
        & $git -C $root archive --format=zip $archiveOutputArgument $sourceRevision
        if ($LASTEXITCODE -ne 0) {
            throw "Tracked source archive generation failed with exit code $LASTEXITCODE."
        }
        [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $paths.sourceArchive)
        Set-AsterZipModesFromInventory `
            -RepositoryRoot $root `
            -ArchivePath $paths.sourceArchive `
            -Inventory $inventory
        Assert-AsterZipInventory -RepositoryRoot $root -ArchivePath $paths.sourceArchive -Inventory $inventory
        $inventoryJson = $inventory | ConvertTo-Json -Depth 6
        Write-AsterUtf8Text `
            -RepositoryRoot $root `
            -Path $paths.sourceInventory `
            -Content $inventoryJson | Out-Null

        $identityTargets = @(
            $paths.portableOutput,
            $paths.installerOutput,
            $paths.sourceArchive,
            $paths.sourceInventory,
            $paths.frontendSbomOutput,
            $paths.rustSbomOutput,
            $paths.licenseOutput,
            $paths.noticeOutput,
            $paths.previewOutput,
            $paths.buildIdentityOutput,
            $paths.verificationEvidence
        )
        $identityRows = foreach ($path in $identityTargets) {
            $safePath = Resolve-AsterContainedPath -RepositoryRoot $root -Path $path
            $item = Get-Item -LiteralPath $safePath -Force
            $hash = (Get-AsterFileIdentity -RepositoryRoot $root -Path $safePath).sha256
            [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $safePath)
            $signature = if ($item.Extension -eq ".exe") {
                (Get-AuthenticodeSignature -LiteralPath $safePath).Status.ToString()
            }
            else {
                "Not applicable"
            }
            $modifiedUtc = $item.LastWriteTimeUtc.ToString("yyyy-MM-ddTHH:mm:ssZ")
            "| ``$($item.Name)`` | $($item.Length) | $modifiedUtc | ``$hash`` | $signature |"
        }
        $generatedUtc = [DateTime]::UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ")
        $identitySection = @"


## Packaged artifact identity

This table was generated by ``scripts/package-engineering.ps1`` after staged
copies and the tracked-source inventory were validated. ``SHA256SUMS.txt``
separately covers every final handoff file except itself.

**Generated UTC:** $generatedUtc

**Self-declared verifier identity:** $VerifierIdentity

**Build identity revision:** $sourceRevision

| File | Bytes | Modified UTC | SHA-256 | Authenticode |
| --- | ---: | --- | --- | --- |
$($identityRows -join [Environment]::NewLine)
"@
        $verificationContent = $report.TrimEnd() + $identitySection + "`n"
        Write-AsterUtf8Text `
            -RepositoryRoot $root `
            -Path $paths.verification `
            -Content $verificationContent | Out-Null
        Write-AsterChecksums -RepositoryRoot $root -Directory $staging -ChecksumPath $paths.checksum

        if ($null -ne $FixtureBeforeFinalStateHook) {
            & $FixtureBeforeFinalStateHook $paths
        }
        $expectedNames = [string[]]@($paths.Values | ForEach-Object { [System.IO.Path]::GetFileName($_) })
        Assert-AsterPackageAllowlist -RepositoryRoot $root -Directory $staging -ExpectedNames $expectedNames
        Assert-AsterChecksums -RepositoryRoot $root -Directory $staging -ChecksumPath $paths.checksum
        Assert-AsterZipInventory -RepositoryRoot $root -ArchivePath $paths.sourceArchive -Inventory $inventory
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.artifacts.releaseBinary -Path $paths.portableOutput -Label "final copied release binary"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.artifacts.installer -Path $paths.installerOutput -Label "final copied NSIS installer"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.sboms.frontend -Path $paths.frontendSbomOutput -Label "final copied frontend SBOM"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $buildIdentity.sboms.rust -Path $paths.rustSbomOutput -Label "final copied Rust SBOM"
        Assert-AsterFileIdentity -RepositoryRoot $root -Expected $manifestIdentity -Path $paths.buildIdentityOutput -Label "final copied build identity"
        Assert-AsterControlledCopy -RepositoryRoot $root -Inventory $inventory -TrackedPath "LICENSE" -CopiedPath $paths.licenseOutput
        Assert-AsterControlledCopy -RepositoryRoot $root -Inventory $inventory -TrackedPath "NOTICE" -CopiedPath $paths.noticeOutput
        Assert-AsterControlledCopy -RepositoryRoot $root -Inventory $inventory -TrackedPath "assets/Aster-MVP-v2-preview.png" -CopiedPath $paths.previewOutput
        if (
            (Read-AsterStrictUtf8Text -RepositoryRoot $root -Path $paths.sourceInventory -MaximumBytes 16777216) -cne $inventoryJson -or
            (Read-AsterStrictUtf8Text -RepositoryRoot $root -Path $paths.verification -MaximumBytes 1048576) -cne $verificationContent -or
            (Read-AsterStrictUtf8Text -RepositoryRoot $root -Path $paths.verificationEvidence -MaximumBytes 4194304) -cne
            $validatedEvidenceLog
        ) {
            throw "A generated package inventory or verification report changed during assembly."
        }
        if (
            [string]$buildIdentity.frontendDist.sha256 -cne
            (Get-AsterDirectoryDigest -RepositoryRoot $root -Path (Join-Path $root "dist"))
        ) {
            throw "The production frontend bundle changed during package assembly."
        }

        Assert-AsterGitState -RepositoryRoot $root -Git $git -SourceRevision $sourceRevision -Stage "final staged-package verification"
        [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $staging)
        [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $output)
        if (Test-Path -LiteralPath $output) {
            throw "OutputDirectory appeared during package assembly; safe publication was aborted."
        }
        if (
            [System.IO.Path]::GetPathRoot($staging) -cne [System.IO.Path]::GetPathRoot($output)
        ) {
            throw "Package staging and publication must remain on the same filesystem volume."
        }
        # Every fallible verification has completed. This same-volume rename is
        # the sole publication action and the last fallible operation that can
        # affect whether the candidate receives the final handoff name.
        if ($null -ne $FixtureBeforePublicationHook) {
            & $FixtureBeforePublicationHook $paths $output
        }
        [System.IO.Directory]::Move($staging, $output)
        $published = $true
        Write-Host "Engineering artifacts and SHA-256 checksums were safely published to $output."
    }
    finally {
        if (-not $published) {
            try {
                $safeStaging = Resolve-AsterContainedPath -RepositoryRoot $root -Path $staging
                if (Test-Path -LiteralPath $safeStaging -PathType Container) {
                    Remove-AsterSafeDirectoryTree -RepositoryRoot $root -Path $safeStaging
                }
            }
            catch {
                Write-Warning "Safe cleanup refused the unpublished staging candidate: $($_.Exception.Message)"
            }
        }
    }
}

Export-ModuleMember -Function @(
    "Assert-AsterExactPropertySet",
    "Assert-AsterFileIdentity",
    "Assert-AsterGitState",
    "Get-AsterGitInventory",
    "Get-AsterGitBlobHashForFile",
    "Assert-AsterControlledCopy",
    "Assert-AsterZipInventory",
    "Get-AsterEvidenceText",
    "Assert-AsterPackageAllowlist",
    "Assert-AsterChecksums",
    "Get-AsterStrictBuildIdentity",
    "Invoke-AsterEngineeringPackage"
)
