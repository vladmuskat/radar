import { useState, type FormEvent } from 'react';
import type { Settings } from '../types';
import region from '../../map-region.json';

/** Edits or captures the map home position and zoom. */
export function HomeSettings({
  settings,
  current,
  onSave,
  disabled,
}: {
  settings: Settings;
  current: () => Pick<Settings, 'home' | 'zoom'>;
  onSave: (patch: Partial<Settings>) => Promise<void>;
  disabled: boolean;
}) {
  const [longitude, setLongitude] = useState(String(settings.home[0]));
  const [latitude, setLatitude] = useState(String(settings.home[1]));
  const [zoom, setZoom] = useState(String(settings.zoom));
  const save = (e: FormEvent) => {
    e.preventDefault();
    void onSave({ home: [Number(longitude), Number(latitude)], zoom: Number(zoom) });
  };
  return (
    <section>
      <h3>Настройка карты · домашняя позиция</h3>
      <p className="muted">Кнопка «Домой» возвращает к сохранённым координатам и масштабу.</p>
      <form onSubmit={save}>
        <fieldset disabled={disabled}>
          <div className="form-grid">
            <label>
              Долгота
              <input
                type="number"
                required
                step="any"
                min={region.bbox[0]}
                max={region.bbox[2]}
                value={longitude}
                onChange={(e) => setLongitude(e.target.value)}
              />
            </label>
            <label>
              Широта
              <input
                type="number"
                required
                step="any"
                min={region.bbox[1]}
                max={region.bbox[3]}
                value={latitude}
                onChange={(e) => setLatitude(e.target.value)}
              />
            </label>
            <label>
              Масштаб
              <input
                type="number"
                required
                step="any"
                min={region.minZoom}
                max={region.maxZoom}
                value={zoom}
                onChange={(e) => setZoom(e.target.value)}
              />
            </label>
          </div>
          <button className="secondary" type="submit">
            Сохранить домашнюю позицию
          </button>
          <button
            className="text-button"
            type="button"
            onClick={() => {
              const view = current();
              setLongitude(String(view.home[0]));
              setLatitude(String(view.home[1]));
              setZoom(String(view.zoom));
              void onSave(view);
            }}
          >
            Использовать текущий вид карты
          </button>
        </fieldset>
      </form>
    </section>
  );
}
