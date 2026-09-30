import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const base = resolve(root, 'public/maps/spb');
const region = JSON.parse(await readFile(resolve(root, 'map-region.json'), 'utf8'));
const manifest = JSON.parse(await readFile(resolve(base, 'manifest.json'), 'utf8'));
assert.deepEqual(manifest.region, region);
assert.equal(manifest.license, 'ODbL-1.0');
assert(manifest.tiles.length > 0);
const paths = new Set();
let total = 0;
for (const tile of manifest.tiles) {
  assert.match(tile.path, /^tiles\/(11|12|13|14)\/\d+\/\d+\.pbf$/);
  assert(!paths.has(tile.path));
  paths.add(tile.path);
  const bytes = await readFile(resolve(base, tile.path));
  assert.equal(bytes.length, tile.bytes);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), tile.sha256);
  total += bytes.length;
}
assert.equal(total, manifest.tileBytes);
// Recompute the expected XYZ coverage independently from the manifest tile list.
const tileX = (lng, z) => Math.floor(((lng + 180) / 360) * 2 ** z);
const tileY = (lat, z) =>
  Math.floor(((1 - Math.asinh(Math.tan((lat * Math.PI) / 180)) / Math.PI) / 2) * 2 ** z);
let expected = 0;
for (let z = manifest.minzoom; z <= manifest.maxzoom; z++) {
  for (let x = tileX(region.bbox[0], z); x <= tileX(region.bbox[2], z); x++) {
    for (let y = tileY(region.bbox[3], z); y <= tileY(region.bbox[1], z); y++) {
      assert(paths.has(`tiles/${z}/${x}/${y}.pbf`), `Missing coverage at ${z}/${x}/${y}`);
      expected++;
    }
  }
}
assert.equal(paths.size, expected);
for (const layer of ['buildings', 'roads', 'railways', 'water', 'landuse']) {
  assert(manifest.layers[layer].features > 0);
}
console.log(`Map verified: ${paths.size} tiles, ${(total / 1024 / 1024).toFixed(2)} MiB`);
