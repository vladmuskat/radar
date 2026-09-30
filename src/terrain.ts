import type { FeatureCollection, Feature, Polygon, LineString } from 'geojson';
import type { Point } from './types';

import region from '../map-region.json';

export const CENTER: [number, number] = [region.center[0], region.center[1]];
/** Converts local east/north metres to map longitude and latitude. */
export function geographic(p: Point): [number, number] {
  return [
    CENTER[0] + ((p.x / (6371000 * Math.cos((CENTER[1] * Math.PI) / 180))) * 180) / Math.PI,
    CENTER[1] + ((p.y / 6371000) * 180) / Math.PI,
  ];
}
/** Converts map longitude and latitude back to radar-local metres. */
export function local(lng: number, lat: number): Point {
  return {
    x: (((lng - CENTER[0]) * Math.PI) / 180) * 6371000 * Math.cos((CENTER[1] * Math.PI) / 180),
    y: (((lat - CENTER[1]) * Math.PI) / 180) * 6371000,
  };
}
/** Builds a GeoJSON polygon from radar-local points. */
export function polygon(
  points: Point[],
  properties: Record<string, unknown> = {},
): Feature<Polygon> {
  return {
    type: 'Feature',
    properties,
    geometry: { type: 'Polygon', coordinates: [[...points, points[0]].map(geographic)] },
  };
}
/** Builds a GeoJSON line from radar-local points. */
export function line(
  points: Point[],
  properties: Record<string, unknown> = {},
): Feature<LineString> {
  return {
    type: 'Feature',
    properties,
    geometry: { type: 'LineString', coordinates: points.map(geographic) },
  };
}
/** Packs GeoJSON features into a collection accepted by MapLibre sources. */
export function collection(features: Feature[]): FeatureCollection {
  return { type: 'FeatureCollection', features };
}
/** Approximates a radar ring with 128 line segments. */
export function circle(radius: number): Point[] {
  return Array.from({ length: 129 }, (_, i) => ({
    x: radius * Math.cos((i / 128) * Math.PI * 2),
    y: radius * Math.sin((i / 128) * Math.PI * 2),
  }));
}
