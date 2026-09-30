import { useEffect, useRef, useState } from 'react';
import {
  Map as MapLibreMap,
  AttributionControl,
  NavigationControl,
  ScaleControl,
  Marker,
  setWorkerUrl,
  type GeoJSONSource,
} from 'maplibre-gl';
import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url';
import 'maplibre-gl/dist/maplibre-gl.css';
import { Home, LocateFixed, Plus, Minus } from 'lucide-react';
import { CENTER, circle, collection, geographic, line, local, polygon } from './terrain';
import type { Point, Settings, Snapshot, Zone } from './types';
import { addBasemap, MAP_BOUNDS, region } from './basemap';
import { logFailure } from './logging';
setWorkerUrl(workerUrl);

type Props = {
  snapshot: Snapshot;
  settings: Settings;
  zones: Zone[];
  drawing: boolean;
  draft: Point[];
  onPoint: (p: Point) => void;
  onIdentify: (id: number) => void;
  onHome: (center: [number, number], zoom: number) => void;
  onViewport?: (center: [number, number], zoom: number) => void;
};
/** Owns MapLibre layers, radar rendering and target interaction overlays. */
export function RadarMap(props: Props) {
  const host = useRef<HTMLDivElement>(null);
  const mapRef = useRef<MapLibreMap | null>(null);
  const latest = useRef(props);
  latest.current = props;
  const clickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const observations = useRef(new globalThis.Map<number, number>());
  const pulseFrames = useRef(new globalThis.Map<number, number>());
  const [ready, setReady] = useState(false);
  const [pinnedIds, setPinnedIds] = useState<number[]>([]);
  const [, setViewportRevision] = useState(0);
  const [error, setError] = useState('');
  const [bearing, setBearing] = useState(0);

  useEffect(() => {
    if (!host.current) return;
    let map: MapLibreMap;
    try {
      map = new MapLibreMap({
        container: host.current,
        center: CENTER,
        zoom: region.defaultZoom,
        minZoom: region.minZoom,
        maxZoom: region.maxZoom,
        maxBounds: MAP_BOUNDS,
        canvasContextAttributes: { preserveDrawingBuffer: true },
        attributionControl: false,
        style: {
          version: 8,
          sources: {},
          layers: [{ id: 'base', type: 'background', paint: { 'background-color': '#2b383c' } }],
        },
      });
    } catch (e) {
      logFailure('map_failed', e);
      setError(String(e));
      return;
    }
    mapRef.current = map;
    map.addControl(new AttributionControl({ compact: false }), 'bottom-right');
    map.on('error', (event) => {
      logFailure('map_failed', event.error);
      setError(`Ошибка локальной карты: ${event.error.message}`);
    });
    map.addControl(new ScaleControl({ maxWidth: 120, unit: 'metric' }), 'bottom-left');
    map.addControl(new NavigationControl({ showZoom: false, showCompass: false }), 'bottom-right');
    map.doubleClickZoom.disable();
    map.on('rotate', () => setBearing(map.getBearing()));
    map.on('move', () => setViewportRevision((revision) => revision + 1));
    map.on('moveend', () => latest.current.onViewport?.(map.getCenter().toArray(), map.getZoom()));
    map.on('load', () => {
      addBasemap(map);
      map.addSource('zones', { type: 'geojson', data: collection([]) });
      map.addLayer({
        id: 'zone-fill',
        type: 'fill',
        source: 'zones',
        paint: {
          'fill-color': ['match', ['get', 'kind'], 'ignore', '#50a8d9', '#df195d'],
          'fill-opacity': ['match', ['get', 'kind'], 'ignore', 0.25, 0.09],
        },
      });
      map.addLayer({
        id: 'zone-outline',
        type: 'line',
        source: 'zones',
        paint: {
          'line-color': ['match', ['get', 'kind'], 'ignore', '#79c5f1', '#ed3470'],
          'line-width': 2,
        },
      });
      map.addSource('rings', {
        type: 'geojson',
        data: collection([1000, 3000, 7000].map((r) => line(circle(r)))),
      });
      map.addLayer({
        id: 'rings',
        type: 'line',
        source: 'rings',
        paint: { 'line-color': '#e5efea', 'line-width': 1, 'line-opacity': 0.7 },
      });
      map.addSource('trails', { type: 'geojson', data: collection([]) });
      map.addLayer({
        id: 'trails',
        type: 'line',
        source: 'trails',
        paint: { 'line-color': '#ffffff', 'line-width': 3, 'line-opacity': 0.95 },
      });
      map.addSource('sweep', { type: 'geojson', data: collection([]) });
      map.addSource('direction', { type: 'geojson', data: collection([]) });
      map.addLayer({
        id: 'direction',
        type: 'line',
        source: 'direction',
        paint: { 'line-color': '#d0e4ff', 'line-width': 2 },
      });
      map.addLayer({
        id: 'sweep-afterglow',
        type: 'fill',
        source: 'sweep',
        filter: ['==', ['geometry-type'], 'Polygon'],
        paint: { 'fill-color': '#69ffb0', 'fill-opacity': ['get', 'opacity'] },
      });
      map.addLayer({
        id: 'sweep-glow',
        type: 'line',
        source: 'sweep',
        filter: ['==', ['geometry-type'], 'LineString'],
        paint: { 'line-color': '#54ffa2', 'line-width': 9, 'line-opacity': 0.18, 'line-blur': 4 },
      });
      map.addLayer({
        id: 'sweep-beam',
        type: 'line',
        source: 'sweep',
        filter: ['==', ['geometry-type'], 'LineString'],
        paint: { 'line-color': '#a0ffca', 'line-width': 2, 'line-opacity': 0.95 },
      });
      map.addSource('targets', { type: 'geojson', data: collection([]), promoteId: 'id' });
      map.addLayer({
        id: 'target-halo',
        type: 'circle',
        source: 'targets',
        paint: {
          'circle-radius': ['+', 13, ['*', ['coalesce', ['feature-state', 'pulse'], 0], 7]],
          'circle-color': '#2c88ff',
          'circle-opacity': 0.15,
        },
      });
      map.addLayer({
        id: 'targets',
        type: 'circle',
        source: 'targets',
        paint: {
          'circle-radius': ['+', 5.5, ['*', ['coalesce', ['feature-state', 'pulse'], 0], 4]],
          'circle-color': ['case', ['get', 'identified'], '#ef4b4b', '#1272fa'],
          'circle-stroke-color': '#f5faff',
          'circle-stroke-width': 1.8,
        },
      });
      map.addSource('draft', { type: 'geojson', data: collection([]) });
      map.addLayer({
        id: 'draft',
        type: 'line',
        source: 'draft',
        paint: { 'line-color': '#ffde80', 'line-width': 3, 'line-dasharray': [2, 2] },
      });
      const radar = document.createElement('div');
      radar.className = 'radar-origin';
      radar.innerHTML = '<span></span><b>РЛС–01</b>';
      new Marker({ element: radar }).setLngLat(CENTER).addTo(map);
      [1000, 3000, 7000].forEach((r) => {
        const el = document.createElement('div');
        el.className = 'ring-label';
        el.textContent = `${r / 1000} км`;
        new Marker({ element: el }).setLngLat(geographic({ x: r, y: 0 })).addTo(map);
      });
      setReady(true);
    });
    map.on('click', (e) => {
      if (latest.current.drawing) latest.current.onPoint(local(e.lngLat.lng, e.lngLat.lat));
    });
    map.on('dblclick', 'targets', (e) => {
      e.preventDefault();
      if (clickTimer.current) clearTimeout(clickTimer.current);
      clickTimer.current = null;
      if (!latest.current.drawing && e.features?.[0])
        latest.current.onIdentify(Number(e.features[0].properties.id));
    });
    map.on('click', 'targets', (e) => {
      if (latest.current.drawing || !e.features?.[0]) return;
      e.originalEvent.stopPropagation();
      const id = Number(e.features[0].properties.id);
      if (clickTimer.current) clearTimeout(clickTimer.current);
      clickTimer.current = setTimeout(() => {
        setPinnedIds((ids) =>
          ids.includes(id) ? ids.filter((existing) => existing !== id) : [...ids, id],
        );
        clickTimer.current = null;
      }, 250);
    });
    map.on('mousemove', 'targets', () => {
      map.getCanvas().style.cursor = 'pointer';
    });
    map.on('mouseleave', 'targets', () => {
      map.getCanvas().style.cursor = '';
    });
    const resize = new ResizeObserver(() => map.resize());
    resize.observe(host.current);
    return () => {
      if (clickTimer.current) clearTimeout(clickTimer.current);
      pulseFrames.current.forEach((frame) => cancelAnimationFrame(frame));
      pulseFrames.current.clear();
      resize.disconnect();
      map.remove();
      mapRef.current = null;
    };
  }, []);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    // Update a named GeoJSON layer without recreating the MapLibre source.
    const set = (name: string, data: ReturnType<typeof collection>) => {
      void (map.getSource(name) as GeoJSONSource).setData(data);
    };
    set(
      'targets',
      collection(
        props.snapshot.targets.map((t) => ({
          type: 'Feature',
          id: t.id,
          properties: { id: t.id, identified: t.identified },
          geometry: { type: 'Point', coordinates: geographic(t.position) },
        })),
      ),
    );
    const liveIds = new Set(props.snapshot.targets.map((target) => target.id));
    for (const target of props.snapshot.targets) {
      const previous = observations.current.get(target.id);
      observations.current.set(target.id, target.observedAtMs);
      if (previous === target.observedAtMs) continue;
      const oldFrame = pulseFrames.current.get(target.id);
      if (oldFrame) cancelAnimationFrame(oldFrame);
      const startedAt = performance.now();
      // Drive a short feature-state pulse for each authoritative observation.
      const animate = (now: number) => {
        const progress = Math.min((now - startedAt) / 420, 1);
        const pulse = Math.sin(progress * Math.PI);
        try {
          map.setFeatureState({ source: 'targets', id: target.id }, { pulse });
        } catch {
          return;
        }
        if (progress < 1) {
          pulseFrames.current.set(target.id, requestAnimationFrame(animate));
        } else {
          pulseFrames.current.delete(target.id);
        }
      };
      pulseFrames.current.set(target.id, requestAnimationFrame(animate));
    }
    for (const id of observations.current.keys()) {
      if (!liveIds.has(id)) {
        observations.current.delete(id);
        const frame = pulseFrames.current.get(id);
        if (frame) cancelAnimationFrame(frame);
        pulseFrames.current.delete(id);
      }
    }
    set(
      'trails',
      collection(
        props.settings.trails
          ? props.snapshot.targets.filter((t) => t.trail.length >= 2).map((t) => line(t.trail))
          : [],
      ),
    );
    // Estimate course from two observed hits, never from hidden live simulation positions.
    set(
      'direction',
      collection(
        props.settings.direction
          ? props.snapshot.targets.flatMap((t) => {
              const previous = t.trail.at(-2);
              if (!previous) return [];
              const dx = t.position.x - previous.x,
                dy = t.position.y - previous.y;
              const length = Math.hypot(dx, dy);
              if (length < 0.01) return [];
              const ux = dx / length,
                uy = dy / length;
              const tip = { x: t.position.x + ux * 150, y: t.position.y + uy * 150 };
              return [
                line([t.position, tip]),
                line([
                  { x: tip.x - ux * 40 - uy * 25, y: tip.y - uy * 40 + ux * 25 },
                  tip,
                  { x: tip.x - ux * 40 + uy * 25, y: tip.y - uy * 40 - ux * 25 },
                ]),
              ];
            })
          : [],
      ),
    );
    // Same authoritative bearing as Rust detection; no independent animation clock.
    const angle = props.snapshot.sweepBearing;
    // Return the seven-kilometre beam endpoint for a clockwise bearing.
    const radial = (a: number): Point => ({ x: Math.sin(a) * 7000, y: Math.cos(a) * 7000 });
    const origin: Point = { x: 0, y: 0 };
    set(
      'sweep',
      collection([
        ...Array.from({ length: 16 }, (_, i) =>
          polygon([origin, radial(angle - i * 0.018), radial(angle - (i + 1) * 0.018)], {
            opacity: 0.16 * (1 - i / 16),
          }),
        ),
        line([origin, radial(angle)]),
      ]),
    );
  }, [ready, props.snapshot, props.settings.trails, props.settings.direction]);

  useEffect(() => {
    const liveIds = new Set(props.snapshot.targets.map((target) => target.id));
    setPinnedIds((ids) => ids.filter((id) => liveIds.has(id)));
  }, [props.snapshot.targets]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    void (map.getSource('zones') as GeoJSONSource).setData(
      collection(
        props.zones
          .filter((z) => z.points.length >= 3)
          .map((z) => polygon(z.points, { kind: z.kind })),
      ),
    );
  }, [ready, props.zones]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !ready) return;
    void (map.getSource('draft') as GeoJSONSource).setData(
      collection(
        props.draft.length >= 2
          ? [line(props.draft.length >= 3 ? [...props.draft, props.draft[0]] : props.draft)]
          : [],
      ),
    );
  }, [ready, props.draft]);

  const pinnedTargets = pinnedIds.flatMap((id) => {
    const target = props.snapshot.targets.find((candidate) => candidate.id === id);
    const map = mapRef.current;
    if (!target || !map) return [];
    const point = map.project(geographic(target.position));
    return [{ target, x: point.x, y: point.y }];
  });
  return (
    <div className="map-shell">
      <div ref={host} className="map-canvas" />
      <div className="map-corner-label">
        <span className="live-dot" /> РЛС–01 <span className="divider">/</span> Сектор наблюдения
      </div>
      <div className="map-meta">
        {region.name}
        <br />
        {CENTER[1].toFixed(4)}° N &nbsp; {CENTER[0].toFixed(4)}° E<br />
        <span>Круговой обзор · оборот 4 с · автономная карта</span>
      </div>
      {props.drawing && (
        <div className="drawing-hint">Нажимайте на карту, чтобы добавить вершины зоны</div>
      )}
      {props.snapshot.status === 'paused' && (
        <div className="pause-badge">Ⅱ &nbsp; Тренировка приостановлена</div>
      )}
      <div className="map-tools">
        <button title="Север сверху" onClick={() => mapRef.current?.resetNorth()}>
          <span style={{ transform: `rotate(${-bearing}deg)` }}>N ↑</span>
        </button>
        <button title="Увеличить" onClick={() => mapRef.current?.zoomIn()}>
          <Plus size={19} />
        </button>
        <button title="Уменьшить" onClick={() => mapRef.current?.zoomOut()}>
          <Minus size={19} />
        </button>
        <button
          title="Вернуться домой"
          onClick={() =>
            mapRef.current?.flyTo({
              center: props.settings.home,
              zoom: props.settings.zoom,
              bearing: 0,
            })
          }
        >
          <Home size={19} />
        </button>
        <button
          title="Сохранить текущую домашнюю позицию"
          onClick={() => {
            const m = mapRef.current;
            if (m) props.onHome(m.getCenter().toArray() as [number, number], m.getZoom());
          }}
        >
          <LocateFixed size={19} />
        </button>
      </div>
      {pinnedTargets.map(({ target, x, y }) => (
        <div
          key={target.id}
          className="target-tooltip"
          style={{
            left: Math.max(8, Math.min(x + 16, (host.current?.clientWidth ?? 500) - 210)),
            top: Math.max(12, Math.min(y - 100, (host.current?.clientHeight ?? 500) - 145)),
          }}
        >
          <b>
            Цель №{target.id} {target.identified && '✓'}
          </b>
          <strong>
            {(target.speedMps * 3.6).toFixed(1)} <small>км/ч</small>
          </strong>
          <span>
            {target.latitude.toFixed(6)}° N<br />
            {target.longitude.toFixed(6)}° E
          </span>
          <em>
            {target.identified
              ? 'Уничтожение — при следующем проходе'
              : 'Двойной клик — запланировать уничтожение'}
          </em>
        </div>
      ))}
      {error && <div className="map-error">Не удалось запустить карту: {error}</div>}
    </div>
  );
}
