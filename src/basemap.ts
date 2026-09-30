import type { Map } from 'maplibre-gl';
import region from '../map-region.json';

export { region };
export const MAP_BOUNDS: [[number, number], [number, number]] = [
  [region.bbox[0], region.bbox[1]],
  [region.bbox[2], region.bbox[3]],
];

/** All tiles are packaged by Vite/Tauri. No remote URLs, API keys or glyph servers. */
export function addBasemap(map: Map) {
  const base = new URL(`${import.meta.env.BASE_URL}maps/spb/`, window.location.href).href;
  map.addSource('city', {
    type: 'vector',
    tiles: [`${base}tiles/{z}/{x}/{y}.pbf`],
    bounds: region.bbox as [number, number, number, number],
    minzoom: 11,
    maxzoom: 14,
    attribution:
      '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noopener noreferrer">OpenStreetMap contributors</a> · <a href="https://opendatacommons.org/licenses/odbl/1-0/" target="_blank" rel="noopener noreferrer">ODbL</a>',
  });
  map.addLayer({
    id: 'city-landuse',
    type: 'fill',
    source: 'city',
    'source-layer': 'landuse',
    paint: {
      'fill-color': [
        'match',
        ['get', 'kind'],
        'green',
        '#314b41',
        'industrial',
        '#495052',
        'railway',
        '#42484b',
        'construction',
        '#514c43',
        'commercial',
        '#434b4c',
        'retail',
        '#434b4c',
        '#374448',
      ],
      'fill-opacity': 0.85,
    },
  });
  map.addLayer({
    id: 'city-water',
    type: 'fill',
    source: 'city',
    'source-layer': 'water',
    filter: ['==', ['geometry-type'], 'Polygon'],
    paint: { 'fill-color': '#193d50' },
  });
  map.addLayer({
    id: 'city-waterways',
    type: 'line',
    source: 'city',
    'source-layer': 'water',
    filter: ['==', ['geometry-type'], 'LineString'],
    paint: {
      'line-color': '#285369',
      'line-width': ['interpolate', ['linear'], ['zoom'], 11, 0.8, 16, 3],
    },
  });
  map.addLayer({
    id: 'city-roads',
    type: 'line',
    source: 'city',
    'source-layer': 'roads',
    paint: {
      'line-color': [
        'match',
        ['get', 'kind'],
        ['motorway', 'trunk', 'primary'],
        '#aaa88f',
        '#757e7c',
      ],
      'line-width': ['interpolate', ['linear'], ['zoom'], 11, 0.6, 14, 1.7, 16, 5],
      'line-opacity': 0.75,
    },
  });
  map.addLayer({
    id: 'city-railways',
    type: 'line',
    source: 'city',
    'source-layer': 'railways',
    paint: {
      'line-color': '#b5a58a',
      'line-width': ['interpolate', ['linear'], ['zoom'], 11, 0.5, 16, 2],
      'line-dasharray': [3, 2],
      'line-opacity': 0.6,
    },
  });
  map.addLayer({
    id: 'city-buildings',
    type: 'fill',
    source: 'city',
    'source-layer': 'buildings',
    paint: {
      'fill-color': '#89958f',
      'fill-opacity': ['interpolate', ['linear'], ['zoom'], 11, 0.45, 14, 0.8],
      'fill-outline-color': '#a9b1a3',
    },
  });
}
