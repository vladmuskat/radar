param([ValidateSet('dev', 'build', 'test', 'check')][string]$Mode = 'dev')
$ErrorActionPreference = 'Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
        $radarVswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
        if (-not (Test-Path $radarVswhere)) { throw 'Install Visual Studio C++ build tools and Windows SDK first.' }
        $radarVisualStudio = & $radarVswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if (-not $radarVisualStudio) { throw 'Visual Studio C++ workload was not found.' }
        & (Join-Path $radarVisualStudio 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    }
    switch ($Mode) {
        'dev' { & npm.cmd exec tauri -- dev }
        'build' { & npm.cmd exec tauri -- build --no-bundle }
        'test' { & cargo test --workspace --locked }
        'check' { & cargo clippy --workspace --all-targets --locked -- -D warnings }
    }
    $radarExit = $LASTEXITCODE
} finally { Pop-Location }
exit $radarExit
