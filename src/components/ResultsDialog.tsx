import { Check } from 'lucide-react';
import type { Snapshot } from '../types';
import { time } from '../api';
import { Dialog, Metric } from './ui';

/** Presents final scoring and a recoverable persistence retry action. */
export function ResultsDialog({
  snapshot,
  onClose,
  onRetry,
  busy,
}: {
  snapshot: Snapshot;
  onClose: () => void;
  onRetry: () => void;
  busy: boolean;
}) {
  return (
    <Dialog
      title={snapshot.config.exam ? 'Результаты экзамена' : 'Тренировка завершена'}
      caption={
        snapshot.saveError ? 'Не удалось сохранить результат' : 'Результат сохранён в вашем профиле'
      }
      onClose={onClose}
    >
      <div className="results-icon">
        <Check size={30} />
      </div>
      <div className="result-grid">
        <Metric label="Время сеанса" value={time(snapshot.simulationTimeMs)} />
        <Metric label="Отмечено целей" value={snapshot.statistics.marked} />
        <Metric label="Правильно" value={snapshot.statistics.correct} className="green" />
        <Metric label="Ошибок" value={snapshot.statistics.errors} className="red" />
        <Metric label="Пропущено БВС" value={snapshot.statistics.missed} />
        <Metric
          label="Средняя реакция"
          value={`${(snapshot.statistics.averageReactionMs / 1000).toFixed(1)} с`}
        />
      </div>
      {snapshot.saveError && (
        <button className="primary full" disabled={busy} onClick={onRetry}>
          Повторить сохранение результата
        </button>
      )}
      <button className="primary full" onClick={onClose}>
        Вернуться к карте
      </button>
    </Dialog>
  );
}
