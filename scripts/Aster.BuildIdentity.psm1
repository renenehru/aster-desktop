Set-StrictMode -Version Latest

function Assert-AsterNoReparseAncestors {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $anchor = [System.IO.Path]::GetPathRoot($fullPath)
    if ([string]::IsNullOrWhiteSpace($anchor)) {
        throw "Aster safety checks require an absolute filesystem path."
    }

    $current = $anchor
    $segments = @($fullPath.Substring($anchor.Length) -split '[\\/]+' | Where-Object { $_.Length -ne 0 })
    $pathsToCheck = [System.Collections.Generic.List[string]]::new()
    $pathsToCheck.Add($anchor)
    foreach ($segment in $segments) {
        $current = Join-Path $current $segment
        $pathsToCheck.Add($current)
    }

    $missingAncestorSeen = $false
    foreach ($candidate in $pathsToCheck) {
        if ($missingAncestorSeen) {
            continue
        }
        $itemErrors = @()
        $item = Get-Item `
            -LiteralPath $candidate `
            -Force `
            -ErrorAction SilentlyContinue `
            -ErrorVariable +itemErrors
        if ($null -eq $item) {
            $unexpectedError = @($itemErrors | Where-Object {
                    $_.FullyQualifiedErrorId -notmatch 'PathNotFound|ItemNotFound'
                })
            if ($unexpectedError.Count -ne 0) {
                throw "A security-sensitive path ancestor could not be inspected: $candidate"
            }
            $missingAncestorSeen = $true
            continue
        }
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Reparse points and junctions are prohibited in security-sensitive paths: $candidate"
        }
    }

    return $fullPath
}

function Resolve-AsterContainedPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $candidate = [System.IO.Path]::GetFullPath($Path)
    $rootPrefix = $root + [System.IO.Path]::DirectorySeparatorChar
    if (
        -not [string]::Equals($candidate, $root, [StringComparison]::OrdinalIgnoreCase) -and
        -not $candidate.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)
    ) {
        throw "Security-sensitive paths must resolve inside the repository."
    }

    [void](Assert-AsterNoReparseAncestors -Path $root)
    [void](Assert-AsterNoReparseAncestors -Path $candidate)
    return $candidate
}

function New-AsterSafeDirectory {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $absolute = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Path
    if (Test-Path -LiteralPath $absolute) {
        if (-not (Test-Path -LiteralPath $absolute -PathType Container)) {
            throw "A required directory path is occupied by a file: $absolute"
        }
        return $absolute
    }

    $missing = [System.Collections.Generic.List[string]]::new()
    $cursor = $absolute
    while (-not (Test-Path -LiteralPath $cursor)) {
        $missing.Add($cursor)
        $parent = [System.IO.Directory]::GetParent($cursor)
        if ($null -eq $parent) {
            throw "A safe existing parent directory could not be found for $absolute."
        }
        $cursor = $parent.FullName
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $cursor)
    }
    if (-not (Test-Path -LiteralPath $cursor -PathType Container)) {
        throw "The parent path is not a directory: $cursor"
    }

    for ($index = $missing.Count - 1; $index -ge 0; $index--) {
        $directory = $missing[$index]
        $parent = [System.IO.Directory]::GetParent($directory).FullName
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $parent)
        New-Item -ItemType Directory -Path $directory -ErrorAction Stop | Out-Null
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $directory)
    }
    return $absolute
}

function Get-AsterSafeDirectoryFiles {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $absolute = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Path
    if (-not (Test-Path -LiteralPath $absolute -PathType Container)) {
        throw "The directory is missing: $absolute"
    }

    $files = [System.Collections.Generic.List[string]]::new()
    $pending = [System.Collections.Generic.Stack[string]]::new()
    $pending.Push($absolute)
    while ($pending.Count -ne 0) {
        $directory = $pending.Pop()
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $directory)
        foreach ($entry in @(Get-ChildItem -LiteralPath $directory -Force)) {
            $entryPath = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $entry.FullName
            if ($entry.PSIsContainer) {
                $pending.Push($entryPath)
            }
            else {
                $files.Add($entryPath)
            }
        }
    }

    $result = $files.ToArray()
    [Array]::Sort($result, [StringComparer]::Ordinal)
    return $result
}

function Remove-AsterSafeDirectoryTree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $absolute = Resolve-AsterContainedPath -RepositoryRoot $root -Path $Path
    if ([string]::Equals($absolute, $root, [StringComparison]::OrdinalIgnoreCase)) {
        throw "The repository root must never be removed."
    }
    if (-not (Test-Path -LiteralPath $absolute)) {
        return
    }
    if (-not (Test-Path -LiteralPath $absolute -PathType Container)) {
        throw "Safe tree removal requires a directory: $absolute"
    }

    function Remove-SafeNode {
        param([string]$Directory)

        [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $Directory)
        foreach ($entry in @(Get-ChildItem -LiteralPath $Directory -Force)) {
            $entryPath = Resolve-AsterContainedPath -RepositoryRoot $root -Path $entry.FullName
            if ($entry.PSIsContainer) {
                Remove-SafeNode -Directory $entryPath
            }
            else {
                [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $entryPath)
                Remove-Item -LiteralPath $entryPath -Force
            }
        }
        [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $Directory)
        Remove-Item -LiteralPath $Directory -Force
    }

    Remove-SafeNode -Directory $absolute
}

function Copy-AsterFileSafely {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Source,

        [Parameter(Mandatory = $true)]
        [string]$Destination
    )

    $safeSource = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Source
    $safeDestination = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Destination
    if (-not (Test-Path -LiteralPath $safeSource -PathType Leaf)) {
        throw "The copy source is missing: $safeSource"
    }
    if (Test-Path -LiteralPath $safeDestination) {
        throw "Safe package assembly never overwrites an existing path: $safeDestination"
    }
    $parent = [System.IO.Directory]::GetParent($safeDestination).FullName
    [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $parent)
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        throw "The copy destination parent is missing: $parent"
    }
    Copy-Item -LiteralPath $safeSource -Destination $safeDestination
    [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $safeDestination)
    return $safeDestination
}

function Read-AsterStrictUtf8Text {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path,

        [long]$MaximumBytes = 262144
    )

    $absolute = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Path
    if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
        throw "The UTF-8 input is missing: $absolute"
    }
    if ($MaximumBytes -lt 0 -or $MaximumBytes -gt [int]::MaxValue) {
        throw "The UTF-8 safety limit is outside the supported range."
    }
    $stream = [System.IO.FileStream]::new(
        $absolute,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::Read
    )
    try {
        if ($stream.Length -gt $MaximumBytes) {
            throw "The UTF-8 input exceeds the $MaximumBytes-byte safety limit: $absolute"
        }
        $bytes = New-Object byte[] ([int]$stream.Length)
        $offset = 0
        while ($offset -lt $bytes.Length) {
            $read = $stream.Read($bytes, $offset, $bytes.Length - $offset)
            if ($read -eq 0) {
                throw "The UTF-8 input ended before its validated length: $absolute"
            }
            $offset += $read
        }
    }
    finally {
        $stream.Dispose()
        [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $absolute)
    }
    if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xef -and $bytes[1] -eq 0xbb -and $bytes[2] -eq 0xbf) {
        throw "UTF-8 inputs must not contain a byte-order mark: $absolute"
    }
    try {
        $decoder = [System.Text.UTF8Encoding]::new($false, $true)
        return $decoder.GetString($bytes)
    }
    catch {
        throw "The input is not strict UTF-8: $absolute"
    }
}

function Write-AsterUtf8Text {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path,

        [Parameter(Mandatory = $true)]
        [AllowEmptyString()]
        [string]$Content,

        [switch]$AllowReplace
    )

    $absolute = Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $Path
    $parent = [System.IO.Directory]::GetParent($absolute).FullName
    [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $parent)
    if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
        throw "The UTF-8 output parent is missing: $parent"
    }
    if ((Test-Path -LiteralPath $absolute) -and -not $AllowReplace) {
        throw "Safe output creation never overwrites an existing path: $absolute"
    }
    $mode = if ($AllowReplace) { [System.IO.FileMode]::Create } else { [System.IO.FileMode]::CreateNew }
    $stream = [System.IO.FileStream]::new(
        $absolute,
        $mode,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None
    )
    try {
        $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($Content)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
    [void](Resolve-AsterContainedPath -RepositoryRoot $RepositoryRoot -Path $absolute)
    return $absolute
}

function Get-AsterFileIdentity {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $absolute = Resolve-AsterContainedPath -RepositoryRoot $root -Path $Path
    if (-not (Test-Path -LiteralPath $absolute -PathType Leaf)) {
        throw "Build identity input is missing: $absolute"
    }
    $rootPrefix = $root + [System.IO.Path]::DirectorySeparatorChar
    $relative = $absolute.Substring($rootPrefix.Length).Replace("\", "/")
    $stream = [System.IO.FileStream]::new(
        $absolute,
        [System.IO.FileMode]::Open,
        [System.IO.FileAccess]::Read,
        [System.IO.FileShare]::Read
    )
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $length = [long]$stream.Length
        $digest = $algorithm.ComputeHash($stream)
    }
    finally {
        $algorithm.Dispose()
        $stream.Dispose()
    }
    $hash = ([System.BitConverter]::ToString($digest) -replace "-", "").ToLowerInvariant()
    [void](Resolve-AsterContainedPath -RepositoryRoot $root -Path $absolute)
    return [ordered]@{
        path   = $relative
        bytes  = $length
        sha256 = $hash
    }
}

function Get-AsterDirectoryDigest {
    param(
        [Parameter(Mandatory = $true)]
        [string]$RepositoryRoot,

        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $root = [System.IO.Path]::GetFullPath($RepositoryRoot).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $absolute = Resolve-AsterContainedPath -RepositoryRoot $root -Path $Path
    if (-not (Test-Path -LiteralPath $absolute -PathType Container)) {
        throw "Build identity directory is missing: $absolute"
    }

    $directoryPrefix = $absolute.TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    ) + [System.IO.Path]::DirectorySeparatorChar
    $rowByRelativePath = [System.Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
    foreach ($file in @(Get-AsterSafeDirectoryFiles -RepositoryRoot $root -Path $absolute)) {
        $relative = $file.Substring($directoryPrefix.Length).Replace("\", "/")
        $identity = Get-AsterFileIdentity -RepositoryRoot $root -Path $file
        $rowByRelativePath.Add($relative, "$relative`t$($identity.bytes)`t$($identity.sha256)")
    }
    $relativePaths = [string[]]@($rowByRelativePath.Keys)
    [Array]::Sort($relativePaths, [StringComparer]::Ordinal)
    $rows = foreach ($relative in $relativePaths) {
        $rowByRelativePath[$relative]
    }
    $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($rows -join "`n")
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digest = $algorithm.ComputeHash($bytes)
    }
    finally {
        $algorithm.Dispose()
    }
    return ([System.BitConverter]::ToString($digest) -replace "-", "").ToLowerInvariant()
}

Export-ModuleMember -Function @(
    "Assert-AsterNoReparseAncestors",
    "Resolve-AsterContainedPath",
    "New-AsterSafeDirectory",
    "Get-AsterSafeDirectoryFiles",
    "Remove-AsterSafeDirectoryTree",
    "Copy-AsterFileSafely",
    "Read-AsterStrictUtf8Text",
    "Write-AsterUtf8Text",
    "Get-AsterFileIdentity",
    "Get-AsterDirectoryDigest"
)
