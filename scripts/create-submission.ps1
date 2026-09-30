param(
    [string]$OutputDirectory = ''
)

$ErrorActionPreference = 'Stop'
$projectDir = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$submissionRoot = [System.IO.Path]::GetFullPath((Join-Path $projectDir 'submission'))
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $submissionRoot 'RADAR'
}
$destination = [System.IO.Path]::GetFullPath($OutputDirectory)
$submissionPrefix = $submissionRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $destination.StartsWith($submissionPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Output must be a child of $submissionRoot"
}

if (Test-Path -LiteralPath $destination) {
    Remove-Item -LiteralPath $destination -Recurse -Force
}
New-Item -ItemType Directory -Path $destination -Force | Out-Null

# Include source and documentation from the current working tree, including
# uncommitted files, but never ignored build products or local databases.
$files = & git -c core.quotePath=false -C $projectDir ls-files --cached --others --exclude-standard
if ($LASTEXITCODE -ne 0) {
    throw 'Git could not enumerate the project files.'
}
$copied = [System.Collections.Generic.List[string]]::new()
$allowedDirectories = @('.github', 'assets', 'crates', 'docs', 'public', 'scripts', 'src', 'src-tauri', 'tests')
$allowedRootFiles = @(
    '.gitattributes',
    '.gitignore',
    '.prettierrc.json',
    'Cargo.lock',
    'Cargo.toml',
    'index.html',
    'map-region.json',
    'package-lock.json',
    'package.json',
    'README.md',
    'tsconfig.json',
    'vite.config.ts'
)
foreach ($gitPath in $files) {
    $normalized = $gitPath.Replace('\', '/')
    $segments = $normalized.Split('/')
    $allowed = if ($segments.Count -eq 1) {
        $normalized -in $allowedRootFiles
    } else {
        $segments[0] -in $allowedDirectories
    }
    if (-not $allowed) {
        continue
    }
    $source = Join-Path $projectDir $gitPath
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        continue
    }
    $target = Join-Path $destination $gitPath
    New-Item -ItemType Directory -Path (Split-Path $target -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $source -Destination $target
    $copied.Add($normalized)
}

$manifest = @(
    'RADAR test assignment submission'
    "Generated: $([DateTime]::UtcNow.ToString('u'))"
    "Files: $($copied.Count)"
    ''
) + ($copied | Sort-Object)
Set-Content -LiteralPath (Join-Path $destination 'SUBMISSION_CONTENTS.txt') -Value $manifest -Encoding utf8
Write-Output "Prepared $($copied.Count) files in $destination"

