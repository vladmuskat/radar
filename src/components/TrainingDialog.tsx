import type { Dispatch, FormEvent, SetStateAction } from 'react';
import { Flag, Radar, Play, Crosshair } from 'lucide-react';
import type { Config } from '../types';
import { Dialog } from './ui';

/** Collects and locks the parameters used to create a training session. */
export function TrainingDialog({
  config,
  setConfig,
  busy,
  onSubmit,
  onClose,
}: {
  config: Config;
  setConfig: Dispatch<SetStateAction<Config>>;
  busy: boolean;
  onSubmit: (e: FormEvent) => void;
  onClose: () => void;
}) {
  return (
    <Dialog
      title="Новая тренировка"
      caption="Настройте сценарий наблюдения"
      onClose={() => {
        if (!busy) onClose();
      }}
    >
      <form onSubmit={onSubmit}>
        <fieldset disabled={busy}>
          <div className="mode-select">
            <button
              type="button"
              className={!config.exam ? 'chosen' : ''}
              onClick={() => setConfig({ ...config, exam: false })}
            >
              <Radar size={24} />
              <b>Тренировка</b>
              <span>Статистика во время сеанса</span>
            </button>
            <button
              type="button"
              className={config.exam ? 'chosen' : ''}
              onClick={() => setConfig({ ...config, exam: true })}
            >
              <Flag size={24} />
              <b>Экзамен</b>
              <span>Оценка после завершения</span>
            </button>
          </div>
          <div className="form-grid">
            <label>
              Длительность, секунд
              <input
                type="number"
                min={30}
                max={3600}
                required
                value={config.durationSeconds}
                onChange={(e) => setConfig({ ...config, durationSeconds: Number(e.target.value) })}
              />
            </label>
            <label>
              Целей одновременно
              <input
                type="number"
                min={1}
                max={20}
                required
                value={config.maxTargets}
                onChange={(e) => setConfig({ ...config, maxTargets: Number(e.target.value) })}
              />
            </label>
          </div>
          <label>
            Номер сценария (seed)
            <input
              type="number"
              min={0}
              max={4294967295}
              required
              value={config.seed}
              onChange={(e) => setConfig({ ...config, seed: Number(e.target.value) })}
            />
          </label>
          <div className="info-box">
            <Crosshair size={20} />
            <p>
              БВС: 25–35 м/с, прямая траектория.
              <br />
              Птицы: 2–10 м/с, криволинейный полёт.
              <br />
              Область наблюдения — 7 км.
            </p>
          </div>
          <button className="primary full" disabled={busy}>
            <Play size={17} />
            Начать сеанс
          </button>
        </fieldset>
      </form>
    </Dialog>
  );
}
