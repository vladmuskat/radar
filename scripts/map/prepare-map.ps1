param(
    [string]$SourcePbf = (Join-Path $PSScriptRoot '../../spb-industrial.osm.pbf'),
    [string]$Distribution = 'Debian'
)
$ErrorActionPreference = 'Stop'
$projectDir = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$sourcePath = (Resolve-Path -LiteralPath $SourcePbf).Path
$buildDir = Join-Path $projectDir 'artifacts/map-build'
New-Item -ItemType Directory -Force -Path $buildDir | Out-Null
# Translate a resolved Windows path for the selected WSL distribution.
function LinuxPath([string]$path) {
    $result = & wsl -d $Distribution -- wslpath -a ($path.Replace('\', '/'))
    if ($LASTEXITCODE -ne 0) { throw "Cannot translate path: $path" }
    return $result.Trim()
}
$filtered = LinuxPath (Join-Path $buildDir 'filtered.osm.pbf')
$exported = Join-Path $buildDir 'export.geojson'
& wsl -d $Distribution -- osmium tags-filter (LinuxPath $sourcePath) 'wr/building' 'w/highway' 'w/railway' 'wr/waterway' 'wr/natural' 'wr/landuse' 'wr/water' 'wr/leisure=park,garden' -o $filtered --overwrite
if ($LASTEXITCODE -ne 0) { throw 'OSM tag filtering failed' }
& wsl -d $Distribution -- osmium export $filtered --config (LinuxPath (Join-Path $PSScriptRoot 'osmium-export.json')) --add-unique-id type_id --geometry-types linestring,polygon --show-errors -o (LinuxPath $exported) --overwrite
if ($LASTEXITCODE -ne 0) { throw 'OSM geometry export failed' }
& uv run --with 'shapely==2.1.2' python (Join-Path $PSScriptRoot 'prepare_map.py') --input $exported --source $sourcePath --output (Join-Path $buildDir 'geojson')
if ($LASTEXITCODE -ne 0) { throw 'Map preparation failed' }
& node (Join-Path $PSScriptRoot 'build-map-tiles.mjs')
if ($LASTEXITCODE -ne 0) { throw 'Vector tile generation failed' }
