// Offline build step only. Runtime reads the generated MVT files, not GeoJSON.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { GeoJSONVT } from '@maplibre/geojson-vt';
import { fromGeojsonVt } from '@maplibre/vt-pbf';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const source = resolve(process.argv[2] ?? resolve(root, 'artifacts/map-build/geojson'));
const output = resolve(root, 'public/maps/spb');
const manifest = JSON.parse(await readFile(resolve(source, 'manifest.json'), 'utf8'));
const [west, south, east, north] = manifest.region.bbox;
const indices = {};
for (const layer of Object.keys(manifest.layers)) {
  const data = JSON.parse(await readFile(resolve(source, `${layer}.geojson`), 'utf8'));
  // Original IDs are strings; rendering does not need them in the numeric MVT ID field.
  for (const feature of data.features) delete feature.id;
  indices[layer] = new GeoJSONVT(data, { maxZoom: 14, extent: 4096, buffer: 64, tolerance: 1 });
}
// Convert longitude/latitude to XYZ tile coordinates for the requested zoom.
const tileX = (longitude, zoom) => Math.floor(((longitude + 180) / 360) * 2 ** zoom);
const tileY = (latitude, zoom) => {
  const angle = (latitude * Math.PI) / 180;
  return Math.floor(((1 - Math.asinh(Math.tan(angle)) / Math.PI) / 2) * 2 ** zoom);
};
const tiles = [];
for (let z = 11; z <= 14; z++) {
  for (let x = tileX(west, z); x <= tileX(east, z); x++) {
    await mkdir(resolve(output, `tiles/${z}/${x}`), { recursive: true });
    for (let y = tileY(north, z); y <= tileY(south, z); y++) {
      const layers = {};
      for (const [name, index] of Object.entries(indices)) {
        const tile = index.getTile(z, x, y);
        if (tile?.features.length) layers[name] = tile;
      }
      const bytes = fromGeojsonVt(layers, { version: 2, extent: 4096 });
      const path = `tiles/${z}/${x}/${y}.pbf`;
      await writeFile(resolve(output, path), bytes);
      tiles.push({
        path,
        bytes: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex'),
      });
    }
  }
}
manifest.format = 'Mapbox Vector Tile v2';
manifest.minzoom = 11;
manifest.maxzoom = 14;
manifest.tiles = tiles;
manifest.tileBytes = tiles.reduce((total, tile) => total + tile.bytes, 0);
await writeFile(resolve(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`${tiles.length} tiles, ${(manifest.tileBytes / 1024 / 1024).toFixed(2)} MiB`);
