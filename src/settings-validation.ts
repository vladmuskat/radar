import type { Settings } from './types';

/** Runtime boundary; TypeScript annotations alone do not validate persisted/IPC values. */
export function normalizeSettings(
  saved: unknown,
  defaults: Settings,
  bbox: number[],
  minZoom: number,
  maxZoom: number,
): Settings {
  const next = { ...defaults, home: [...defaults.home] as [number, number] };
  if (!saved || typeof saved !== 'object' || Array.isArray(saved)) return next;
  const value = saved as Record<string, unknown>;
  for (const key of ['direction', 'trails', 'sound', 'archive'] as const) {
    if (typeof value[key] === 'boolean') next[key] = value[key];
  }
  if (
    typeof value.volume === 'number' &&
    Number.isInteger(value.volume) &&
    value.volume >= 0 &&
    value.volume <= 100
  )
    next.volume = value.volume;
  if (
    Array.isArray(value.home) &&
    value.home.length === 2 &&
    value.home.every(Number.isFinite) &&
    value.home[0] >= bbox[0] &&
    value.home[0] <= bbox[2] &&
    value.home[1] >= bbox[1] &&
    value.home[1] <= bbox[3]
  ) {
    next.home = [value.home[0], value.home[1]];
    if (
      typeof value.zoom === 'number' &&
      Number.isFinite(value.zoom) &&
      value.zoom >= minZoom &&
      value.zoom <= maxZoom
    )
      next.zoom = value.zoom;
  }
  return next;
}
