import { Dialog, Metric, ErrorContext } from './components/ui';
import { TrainingDialog } from './components/TrainingDialog';
import { ResultsDialog } from './components/ResultsDialog';
import { HelpDialog } from './components/HelpDialog';
import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react';
import {
  Activity,
  Bell,
  BookOpen,
  Camera,
  Check,
  ChevronRight,
  Clock3,
  Crosshair,
  LogOut,
  Menu,
  Pause,
  Play,
  Radar,
  RotateCcw,
  Settings2,
  Shield,
  Square,
  UserRound,
  Volume2,
  X,
  Layers,
  ChartNoAxesCombined,
  Plus,
  Trash2,
} from 'lucide-react';
import { toPng } from 'html-to-image';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PendingCapture, Preferences } from './persistence';
import { logFailure } from './logging';
import { trainingViewConfig } from './training-view';
import { HomeSettings } from './components/HomeSettings';
import {
  defaults,
  desktop,
  emptyConfig,
  emptySnapshot,
  request,
  restoreSettings,
  subscribe,
  time,
} from './api';
import { RadarMap } from './RadarMap';
import { geographic } from './terrain';
import type {
  ArchivePage,
  ArchiveOutcome,
  Command,
  Config,
  HistoryPage,
  Point,
  Settings,
  Snapshot,
  User,
  Zone,
} from './types';

type Panel = 'menu' | 'zones' | 'statistics';
type Modal = 'training' | 'settings' | 'profile' | 'help' | 'results' | 'archive' | null;
const statusLabels = {
  ready: 'Готов к работе',
  running: 'Тренировка идёт',
  paused: 'На паузе',
  finished: 'Сеанс завершён',
};
/** Coordinates authentication, training lifecycle, persistence and all desktop views. */
export function App() {
  const [user, setUser] = useState<User | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot>(emptySnapshot);
  const [config, setConfig] = useState<Config>(emptyConfig);
  const [settings, setSettings] = useState<Settings>(defaults);
  const [panel, setPanel] = useState<Panel>('menu');
  const [leftOpen, setLeftOpen] = useState(true);
  const [rightOpen, setRightOpen] = useState(true);
  const [modal, setModal] = useState<Modal>(null);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [requests, setRequests] = useState(0);
  const [transitioning, setTransitioning] = useState(false);
  const busy = requests > 0 || transitioning;
  const [closeRequested, setCloseRequested] = useState(false);
  const [initialized, setInitialized] = useState(false);
  const [now, setNow] = useState(new Date());
  const [history, setHistory] = useState<HistoryPage>({
    items: [],
    total: 0,
    correct: 0,
    offset: 0,
    limit: 100,
  });
  const [users, setUsers] = useState<User[]>([]);
  const [archives, setArchives] = useState<ArchivePage>({
    items: [],
    total: 0,
    offset: 0,
    limit: 100,
  });
  const [archiveImage, setArchiveImage] = useState('');
  const [archiveLimitReached, setArchiveLimitReached] = useState(false);
  const archiveLimitSession = useRef('');
  const [drawing, setDrawing] = useState(false);
  const [draft, setDraft] = useState<Point[]>([]);
  const [zoneName, setZoneName] = useState('Новая зона');
  const [zoneKind, setZoneKind] = useState<Zone['kind']>('detection');
  const [clearThrough, setClearThrough] = useState(0);
  const workspace = useRef<HTMLDivElement>(null);
  const session = useRef('');
  const snapshotRef = useRef(snapshot);
  const settingsRef = useRef(settings);
  const preferences = useRef(
    new Preferences(defaults, (settings) => request({ type: 'saveSettings', settings })),
  );
  const capture = useRef(new PendingCapture());
  const transitionLock = useRef(false);
  const viewport = useRef({ home: defaults.home, zoom: defaults.zoom });
  const noticesSeen = useRef(0);
  const lastCapture = useRef(0);
  const activeConnection = useRef(0);
  const audio = useRef<AudioContext | null>(null);
  const wasFinished = useRef(false);

  // Keep the persistence queue, synchronous refs and rendered state consistent.
  const restorePreferences = (saved: Partial<Settings>) => {
    const next = restoreSettings(saved);
    preferences.current.restore(next);
    settingsRef.current = next;
    setSettings(next);
  };
  // Browsers require audio creation/resume to follow a user gesture.
  const ensureAudio = () => {
    try {
      audio.current ??= new AudioContext();
      void audio.current.resume().catch((e) => {
        logFailure('audio_failed', e);
        setError(`Звук недоступен: ${String(e)}`);
      });
    } catch (e) {
      logFailure('audio_failed', e);
      setError(`Звук недоступен: ${String(e)}`);
    }
  };

  // Reject stale sequences and packets belonging to a previous session.
  const accept = useCallback((next: Snapshot, newSession = false) => {
    if (newSession) {
      archiveLimitSession.current = '';
      setArchiveLimitReached(false);
      lastCapture.current = 0;
      session.current = next.sessionId;
      // Reconnection must not replay sound/screenshots for already observed alerts.
      noticesSeen.current = next.notifications.at(-1)?.id ?? 0;
      setClearThrough(0);
      wasFinished.current = false;
    }
    if (next.sessionId !== session.current) return;
    const previous = snapshotRef.current;
    if (previous.sessionId === next.sessionId && next.sequence < previous.sequence) return;
    if (next.saveError) setError(next.saveError);
    snapshotRef.current = next;
    setSnapshot(next);
  }, []);

  // A generation token prevents callbacks from superseded channels changing UI.
  const connect = useCallback(async () => {
    const generation = ++activeConnection.current;
    const initial = await subscribe((next) => {
      if (activeConnection.current === generation) accept(next);
    });
    accept(initial, true);
    setConfig(initial.config);
  }, [accept]);

  useEffect(() => {
    request<{ user: User | null; config: Config; settings: Partial<Settings> }>({
      type: 'bootstrap',
    })
      .then(async (data) => {
        setUser(data.user);
        setConfig(data.config);
        restorePreferences(data.settings);
        if (data.user) await connect();
      })
      .catch((e) => setError(String(e)))
      .finally(() => setInitialized(true));
    const timer = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(timer);
  }, [connect]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const subscription = listen('radar-close-request', () => {
      if (!disposed) setCloseRequested(true);
    });
    void subscription.catch((e) =>
      setError(`Не удалось подключить обработку закрытия: ${String(e)}`),
    );
    return () => {
      disposed = true;
      void subscription.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  // Block new captures before draining pending bytes/settings. Pause stops new events.
  // Pause first, then persist pending settings/images before a destructive transition.
  const transition = async (operation: () => Promise<void>) => {
    if (transitionLock.current) return;
    transitionLock.current = true;
    setTransitioning(true);
    setError('');
    try {
      if (session.current) {
        const paused = await request<Snapshot>({
          type: 'pauseForTransition',
          sessionId: session.current,
        });
        accept(paused);
      }
      await capture.current.flush();
      await preferences.current.flush();
      await operation();
    } catch (e) {
      setError(String(e));
    } finally {
      transitionLock.current = false;
      setTransitioning(false);
    }
  };

  // Apply uniform busy and error handling to one application command.
  const perform = async <T,>(command: Command): Promise<T | undefined> => {
    setRequests((n) => n + 1);
    setError('');
    try {
      return await request<T>(command);
    } catch (e) {
      setError(String(e));
    } finally {
      setRequests((n) => n - 1);
    }
  };
  // Execute session actions and accept only their confirmed Rust snapshot.
  const action = async (type: 'start' | 'pause' | 'resume' | 'finish' | 'reset') => {
    if (type === 'start' || type === 'resume') ensureAudio();
    const run = async () => {
      const next = await perform<Snapshot>({ type, sessionId: session.current });
      if (next) accept(next, type === 'reset');
    };
    if (type === 'reset') await transition(run);
    else if (!transitionLock.current) await run();
  };

  useEffect(() => {
    if (snapshot.status === 'finished' && !wasFinished.current && !closeRequested) {
      setModal('results');
      wasFinished.current = true;
    }
  }, [snapshot.status, closeRequested]);
  useEffect(() => {
    if (!message) return;
    const t = setTimeout(() => setMessage(''), 4000);
    return () => clearTimeout(t);
  }, [message]);

  useEffect(() => {
    const notice = snapshot.notifications.at(-1);
    if (!user || !notice || notice.id <= noticesSeen.current) return;
    noticesSeen.current = notice.id;
    if (
      settingsRef.current.sound &&
      settingsRef.current.volume > 0 &&
      snapshot.status === 'running' &&
      audio.current?.state === 'running'
    ) {
      const osc = audio.current.createOscillator(),
        gain = audio.current.createGain();
      osc.connect(gain);
      gain.connect(audio.current.destination);
      osc.frequency.value = 780;
      gain.gain.setValueAtTime(settingsRef.current.volume / 500, audio.current.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, audio.current.currentTime + 0.18);
      osc.start();
      osc.stop(audio.current.currentTime + 0.18);
    }
    if (
      !settingsRef.current.archive ||
      archiveLimitSession.current === snapshot.sessionId ||
      transitionLock.current ||
      capture.current.pending ||
      Date.now() - lastCapture.current < 5000 ||
      !workspace.current
    )
      return;
    lastCapture.current = Date.now();
    const sessionId = snapshot.sessionId;
    // Group alerts within five seconds into one screenshot of the operator workspace.
    const element = workspace.current;
    void capture.current
      .start(
        async () => {
          await request({ type: 'reserveArchive', sessionId, eventId: notice.id });
          try {
            return await toPng(element, { pixelRatio: 0.8, skipFonts: true });
          } catch (e) {
            await request({ type: 'cancelArchive', sessionId, eventId: notice.id });
            throw e;
          }
        },
        async (dataUrl) => {
          const outcome = await request<ArchiveOutcome>({
            type: 'archiveImage',
            sessionId,
            eventId: notice.id,
            dataUrl,
          });
          if (outcome === 'limitReached') {
            archiveLimitSession.current = sessionId;
            setArchiveLimitReached(true);
            setMessage('Достигнут лимит: 30 снимков за сеанс. Новые снимки не сохраняются.');
          }
        },
      )
      .catch((e) => {
        logFailure('capture_failed', e);
        setError(
          `Не удалось сохранить снимок: ${String(e)}. Повторите сохранение перед сменой сеанса.`,
        );
      });
  }, [snapshot, user]);

  // Authenticate, restore the profile preferences and open a snapshot channel.
  const login = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const data = new FormData(e.currentTarget);
    const logged = await perform<User>({
      type: 'login',
      login: String(data.get('login')),
      password: String(data.get('password')),
    });
    if (logged) {
      setUser(logged);
      try {
        const boot = await request<{ settings: Partial<Settings> }>({ type: 'bootstrap' });
        restorePreferences(boot.settings);
        await connect();
      } catch (err) {
        setError(String(err));
      }
    }
  };
  // Finish/save an active session before clearing the local profile state.
  const logout = async () => {
    if (
      ['running', 'paused'].includes(snapshot.status) &&
      !window.confirm('Завершить тренировку и выйти? Результат будет сохранён.')
    )
      return;
    await transition(async () => {
      const result = await perform<null>({ type: 'logout' });
      if (result === null) {
        activeConnection.current++;
        session.current = '';
        snapshotRef.current = emptySnapshot;
        setUser(null);
        setSnapshot(emptySnapshot);
        setModal(null);
      }
    });
  };
  // Create a UUID-bound engine and then start its confirmed configuration.
  const start = async (e: FormEvent) => {
    e.preventDefault();
    ensureAudio();
    await transition(async () => {
      const created = await perform<Snapshot>({ type: 'create', config });
      if (!created) return;
      accept(created, true);
      const started = await perform<Snapshot>({ type: 'start', sessionId: created.sessionId });
      if (started) {
        accept(started);
        setConfig(started.config);
        setModal(null);
        setDrawing(false);
        setDraft([]);
      }
    });
  };
  // Forward one operator decision while the session is running and unlocked.
  const identify = (id: number) => {
    if (busy || transitionLock.current || snapshotRef.current.status !== 'running') return;
    void perform<Snapshot>({ type: 'identify', sessionId: session.current, targetId: id }).then(
      (next) => {
        if (next) accept(next);
      },
    );
  };
  // Optimistically render a patch while serializing durable writes.
  const saveSettings = async (patch: Partial<Settings>) => {
    if (transitionLock.current) return;
    if (patch.sound) ensureAudio();
    const pending = preferences.current.patch(patch);
    settingsRef.current = preferences.current.current;
    setSettings(preferences.current.current);
    try {
      await pending;
      setMessage('Настройки сохранены');
    } catch (e) {
      setError(`Настройки не сохранены: ${String(e)}. Доступна повторная попытка.`);
    }
  };
  // Load the requested history page before showing the profile modal.
  const openProfile = async (offset = 0) => {
    const rows = await perform<HistoryPage>({ type: 'history', offset });
    if (rows) {
      setHistory(rows);
      setModal('profile');
    }
  };
  // Administrators additionally need the user list for role management.
  const openSettings = async () => {
    if (user?.role === 'administrator') {
      const rows = await perform<User[]>({ type: 'users' });
      if (rows) setUsers(rows);
    }
    setModal('settings');
  };
  // Load lightweight archive metadata; fetch PNG bytes only on demand.
  const openArchive = async (offset = 0) => {
    const rows = await perform<ArchivePage>({ type: 'archives', offset });
    if (rows) {
      setArchives(rows);
      setArchiveImage('');
      setModal('archive');
    }
  };
  // Validate the polygon in Rust and ignore responses crossing session boundaries.
  const addZone = async () => {
    if (
      busy ||
      transitionLock.current ||
      ['running', 'paused'].includes(snapshotRef.current.status)
    )
      return;
    const editingSession = session.current;
    if (draft.length < 3 || !zoneName.trim()) return;
    const next = {
      ...config,
      zones: [
        ...config.zones,
        { id: crypto.randomUUID(), name: zoneName.trim(), kind: zoneKind, points: draft },
      ],
    };
    if ((await perform<null>({ type: 'validateConfig', config: next })) !== null) return;
    if (
      transitionLock.current ||
      editingSession !== session.current ||
      ['running', 'paused'].includes(snapshotRef.current.status)
    )
      return;
    setConfig(next);
    setDraft([]);
    setDrawing(false);
  };
  const active = snapshot.status === 'running' || snapshot.status === 'paused';
  const displayedConfig = trainingViewConfig(snapshot, config);
  const stats = snapshot.statistics;
  const visibleNotices = snapshot.notifications
    .filter((n) => n.id > clearThrough)
    .slice()
    .reverse();

  return (
    <ErrorContext.Provider value={error}>
      <div className="application" ref={workspace}>
        <header className="topbar">
          <div className="brand">
            <button
              className="top-icon"
              aria-label="Открыть меню"
              onClick={() => setLeftOpen(!leftOpen)}
            >
              <Menu size={22} />
            </button>
            <Radar size={30} />
            <div>
              <strong>РАДАР</strong>
              <span>ТРЕНАЖЁР ОПЕРАТОРА</span>
            </div>
          </div>
          <div className="session-controls">
            <button
              className={`run-button ${snapshot.status === 'running' ? 'pause' : ''}`}
              disabled={!user || busy}
              onClick={() =>
                snapshot.status === 'running'
                  ? void action('pause')
                  : snapshot.status === 'paused'
                    ? void action('resume')
                    : setModal('training')
              }
            >
              {snapshot.status === 'running' ? (
                <Pause size={18} />
              ) : (
                <Play size={18} fill="currentColor" />
              )}
              {snapshot.status === 'running'
                ? 'Пауза'
                : snapshot.status === 'paused'
                  ? 'Продолжить'
                  : 'Начать тренировку'}
            </button>
            <div className="timer">
              <Clock3 size={21} />
              <span>{time(snapshot.simulationTimeMs)}</span>
              <small>/ {time(displayedConfig.durationSeconds * 1000)}</small>
            </div>
            <button
              className="top-icon"
              title="Сбросить сеанс"
              disabled={!user || busy || snapshot.status === 'ready'}
              onClick={() => {
                if (
                  window.confirm(
                    'Сбросить сеанс? Текущий результат сохранится, новая тренировка начнётся с нуля.',
                  )
                )
                  void action('reset');
              }}
            >
              <RotateCcw size={20} />
            </button>
            {active && (
              <button
                className="top-icon"
                title="Завершить тренировку"
                disabled={busy}
                onClick={() => {
                  if (window.confirm('Завершить тренировку и сохранить результат?'))
                    void action('finish');
                }}
              >
                <Square size={17} />
              </button>
            )}
          </div>
          <div className="top-right">
            <div className="wall-clock">
              <strong>
                {now.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' })}
              </strong>
              <span>
                {now.toLocaleDateString('ru-RU', {
                  day: 'numeric',
                  month: 'long',
                  year: 'numeric',
                })}
              </span>
            </div>
            <button
              className="top-icon notification-button"
              title="Уведомления"
              onClick={() => setRightOpen(!rightOpen)}
            >
              <Bell size={22} />
              {visibleNotices.length > 0 && <i />}
            </button>
          </div>
        </header>

        <div className="workspace">
          {leftOpen && user && (
            <aside className="sidebar">
              <div className="sidebar-heading">
                <span>РАБОЧЕЕ ПРОСТРАНСТВО</span>
                <button className="icon-button" title="Справка" onClick={() => setModal('help')}>
                  <BookOpen size={17} />
                </button>
              </div>
              <nav>
                <button
                  className={panel === 'menu' ? 'selected' : ''}
                  onClick={() => setPanel('menu')}
                >
                  <Radar size={19} />
                  Обзор тренировки
                  <ChevronRight size={16} />
                </button>
                <button
                  className={panel === 'zones' ? 'selected' : ''}
                  onClick={() => setPanel('zones')}
                >
                  <Layers size={19} />
                  Зоны наблюдения
                  <ChevronRight size={16} />
                </button>
                <button
                  className={panel === 'statistics' ? 'selected' : ''}
                  onClick={() => setPanel('statistics')}
                >
                  <ChartNoAxesCombined size={19} />
                  Статистика
                  <ChevronRight size={16} />
                </button>
                <button onClick={() => void openArchive()}>
                  <Camera size={19} />
                  Архив событий
                  <ChevronRight size={16} />
                </button>
                <button onClick={() => void openProfile()}>
                  <UserRound size={19} />
                  Профиль
                  <ChevronRight size={16} />
                </button>
                <button onClick={() => void openSettings()}>
                  <Settings2 size={19} />
                  Настройки
                  <ChevronRight size={16} />
                </button>
              </nav>
              <div className="sidebar-content">
                {panel === 'menu' && (
                  <>
                    <div className="section-label">ТЕКУЩИЙ СЕАНС</div>
                    <div className="session-card">
                      <span className={`status-chip ${snapshot.status}`}>
                        <i />
                        {statusLabels[snapshot.status]}
                      </span>
                      <h2>{displayedConfig.exam ? 'Экзамен' : 'Тренировка'}</h2>
                      <p>Обнаружение и распознавание беспилотных воздушных судов</p>
                      <div>
                        <span>Длительность</span>
                        <b>{displayedConfig.durationSeconds / 60} мин</b>
                      </div>
                      <div>
                        <span>Целей одновременно</span>
                        <b>до {displayedConfig.maxTargets}</b>
                      </div>
                      <div>
                        <span>Радиус наблюдения</span>
                        <b>7 км</b>
                      </div>
                    </div>
                    <div className="instruction">
                      <Crosshair size={22} />
                      <b>Найдите БВС</b>
                      <p>
                        Наблюдайте за скоростью и траекторией. Дважды нажмите на цель, чтобы
                        отметить её как БВС.
                      </p>
                      <button className="text-button" onClick={() => setModal('help')}>
                        Как работать с тренажёром <ChevronRight size={14} />
                      </button>
                    </div>
                    <div className="legend">
                      <span>
                        <i className="target-dot" /> Неопознанная цель
                      </span>
                      <span>
                        <i className="target-dot identified" /> Отмеченная цель
                      </span>
                      <span>
                        <i className="zone-line" /> Зона обнаружения
                      </span>
                      <span>
                        <i className="zone-line ignore" /> Зона игнорирования
                      </span>
                    </div>
                  </>
                )}
                {panel === 'statistics' && (
                  <>
                    <div className="section-label">РЕЗУЛЬТАТЫ СЕАНСА</div>
                    <Metric label="Отмечено целей" value={stats.marked} />
                    <Metric
                      label="Правильно"
                      value={displayedConfig.exam && active ? '—' : stats.correct}
                      className="green"
                    />
                    <Metric
                      label="Ошибок"
                      value={displayedConfig.exam && active ? '—' : stats.errors}
                      className="red"
                    />
                    <Metric
                      label="Среднее время реакции"
                      value={`${(stats.averageReactionMs / 1000).toFixed(1)} с`}
                    />
                    <div className="reaction-chart">
                      {stats.reactionsMs.slice(-20).map((v, i) => (
                        <div
                          key={i}
                          title={`${(v / 1000).toFixed(1)} с`}
                          style={{
                            height: `${Math.max(4, (v / Math.max(...stats.reactionsMs, 1)) * 100)}%`,
                          }}
                        />
                      ))}
                    </div>
                    <p className="muted">
                      Время от первого обнаружения до отметки. Паузы не учитываются.
                    </p>
                    <button className="secondary full" onClick={() => void openProfile()}>
                      История тренировок
                    </button>
                  </>
                )}
                {panel === 'zones' && (
                  <>
                    <div className="section-label">ПОЛИГОНЫ НАБЛЮДЕНИЯ</div>
                    <p className="muted">
                      Зоны настраиваются до запуска. Игнорирование имеет приоритет.
                    </p>
                    {config.zones.map((z) => (
                      <div className={`zone-card ${z.kind}`} key={z.id}>
                        <Shield size={17} />
                        <div>
                          <b>{z.name}</b>
                          <small>{z.kind === 'ignore' ? 'Игнорирование' : 'Обнаружение'}</small>
                        </div>
                        <button
                          title="Удалить зону"
                          className="icon-button"
                          disabled={active || busy}
                          onClick={() =>
                            setConfig({
                              ...config,
                              zones: config.zones.filter((x) => x.id !== z.id),
                            })
                          }
                        >
                          <Trash2 size={15} />
                        </button>
                      </div>
                    ))}
                    {!drawing ? (
                      <button
                        className="secondary full"
                        disabled={active || busy || config.zones.length >= 20}
                        onClick={() => {
                          setDrawing(true);
                          setDraft([]);
                        }}
                      >
                        <Plus size={16} />
                        Добавить зону
                      </button>
                    ) : (
                      <div className="zone-editor">
                        <label>
                          Название
                          <input
                            value={zoneName}
                            maxLength={60}
                            onChange={(e) => setZoneName(e.target.value)}
                          />
                        </label>
                        <label>
                          Тип
                          <select
                            value={zoneKind}
                            onChange={(e) => setZoneKind(e.target.value as Zone['kind'])}
                          >
                            <option value="detection">Обнаружение</option>
                            <option value="ignore">Игнорирование</option>
                          </select>
                        </label>
                        <p className="muted">
                          Вершин: {draft.length}. Добавьте минимум 3 точки на карте.
                        </p>
                        <button
                          className="primary full"
                          disabled={active || busy || draft.length < 3}
                          onClick={addZone}
                        >
                          Сохранить полигон
                        </button>
                        <button
                          className="text-button"
                          onClick={() => {
                            setDrawing(false);
                            setDraft([]);
                          }}
                        >
                          Отмена
                        </button>
                      </div>
                    )}
                  </>
                )}
              </div>
              <footer className="sidebar-footer">
                <button className="user-profile" onClick={() => void openProfile()}>
                  <div className="avatar">
                    <UserRound size={21} />
                  </div>
                  <div>
                    <b>{user.login}</b>
                    <span>{user.role === 'administrator' ? 'Администратор' : 'Оператор'}</span>
                  </div>
                </button>
                <button className="icon-button" title="Выйти" onClick={() => void logout()}>
                  <LogOut size={18} />
                </button>
              </footer>
            </aside>
          )}

          <main className="map-area">
            <RadarMap
              snapshot={snapshot}
              settings={settings}
              zones={active ? snapshot.config.zones : config.zones}
              drawing={drawing}
              draft={draft}
              onPoint={(p) => setDraft((prev) => (prev.length < 100 ? [...prev, p] : prev))}
              onIdentify={identify}
              onHome={(home, zoom) => {
                if (user) void saveSettings({ home, zoom });
              }}
              onViewport={(home, zoom) => {
                viewport.current = { home, zoom };
              }}
            />
            <div className="map-status">
              <span>
                <Activity size={14} />
                {snapshot.status === 'running' ? 'Наблюдение активно' : 'Система готова'}
              </span>
              <span>
                Целей на карте: <b>{snapshot.targets.length.toString().padStart(2, '0')}</b>
              </span>
              <span>Масштаб и перемещение — мышью</span>
            </div>
          </main>

          {rightOpen && user && (
            <aside className="notifications">
              <header>
                <div>
                  <h2>Уведомления</h2>
                  <span>События обнаружения</span>
                </div>
                <span className="counter">{visibleNotices.length}</span>
              </header>
              <div className="notification-tab">
                <Crosshair size={17} />
                Цели<span>В РЕАЛЬНОМ ВРЕМЕНИ</span>
              </div>
              <div className="notice-list">
                {visibleNotices.length === 0 ? (
                  <div className="empty">
                    <div>
                      <Bell size={27} />
                    </div>
                    <b>Пока всё спокойно</b>
                    <p>
                      Здесь появятся уведомления
                      <br />о входе целей в охраняемые зоны
                    </p>
                  </div>
                ) : (
                  visibleNotices.map((n) => {
                    const [lng, lat] = geographic(n.position);
                    return (
                      <article className="notice" key={n.id}>
                        <div className="notice-title">
                          <b>Цель №{n.targetId}</b>
                          <time>{time(n.timeMs)}</time>
                        </div>
                        <span className="zone-tag">
                          <Shield size={12} />
                          {n.zoneName}
                        </span>
                        <div className="notice-detail">
                          <strong>
                            {(n.speedMps * 3.6).toFixed(1)} <small>км/ч</small>
                          </strong>
                          <span>
                            {lat.toFixed(5)}° N<br />
                            {lng.toFixed(5)}° E
                          </span>
                        </div>
                      </article>
                    );
                  })
                )}
              </div>
              <footer>
                <button
                  className="secondary full"
                  disabled={!visibleNotices.length}
                  onClick={() => setClearThrough(snapshot.notifications.at(-1)?.id ?? 0)}
                >
                  <Trash2 size={15} />
                  Очистить уведомления
                </button>
                <small>Снимки событий остаются в архиве</small>
              </footer>
            </aside>
          )}
        </div>

        {!user && (
          <div className="login-backdrop">
            <form className="login-card" onSubmit={login}>
              <div className="login-logo">
                <Radar size={48} />
                <div>
                  <strong>РАДАР</strong>
                  <span>ТРЕНАЖЁР ОПЕРАТОРА РЛС</span>
                </div>
              </div>
              <div className="login-heading">
                <h1>Вход в систему</h1>
                <p>Войдите, чтобы начать тренировку</p>
              </div>
              <label>
                Логин
                <input
                  name="login"
                  autoComplete="username"
                  placeholder="Введите логин"
                  required
                  autoFocus
                />
              </label>
              <label>
                Пароль
                <input
                  name="password"
                  type="password"
                  autoComplete="current-password"
                  placeholder="Введите пароль"
                  required
                />
              </label>
              <button className="primary full" disabled={busy || !initialized}>
                {busy ? 'Подключение…' : 'Войти'}
                <ChevronRight size={18} />
              </button>
              <p className="demo-hint">
                Демо-доступ: <b>operator</b>
                <br />
                Администратор: <b>admin</b>
              </p>
              {!desktop && (
                <p className="preview-note">
                  Предпросмотр интерфейса. Для тренировки запустите desktop-приложение.
                </p>
              )}
              <footer>
                <Shield size={14} />
                Автономное рабочее место оператора
              </footer>
            </form>
            <span className="login-version">РАДАР / ПРОТОТИП 0.1.0</span>
          </div>
        )}

        {modal === 'training' && (
          <TrainingDialog
            config={config}
            setConfig={(update) => {
              if (!transitionLock.current && !busy) setConfig(update);
            }}
            busy={busy}
            onSubmit={start}
            onClose={() => setModal(null)}
          />
        )}

        {modal === 'results' && (
          <ResultsDialog
            snapshot={snapshot}
            busy={busy}
            onRetry={() => void action('finish')}
            onClose={() => setModal(null)}
          />
        )}

        {modal === 'settings' && (
          <Dialog
            title="Настройки"
            caption="Отображение и параметры рабочего места"
            onClose={() => setModal(null)}
          >
            <div className="settings-list">
              {(
                [
                  ['trails', 'Показывать траектории', 'Последние 40 обнаружений лучом'],
                  [
                    'direction',
                    'Показывать направление движения',
                    'Оценка курса по двум последним обнаружениям',
                  ],
                  ['sound', 'Звуковые уведомления', 'Сигнал при входе в охраняемую зону'],
                  [
                    'archive',
                    'Снимки тревожных событий',
                    'До 30 снимков за сеанс, интервал от 5 секунд',
                  ],
                ] as const
              ).map(([key, title, desc]) => (
                <label className="toggle-row" key={key}>
                  <span>
                    <b>{title}</b>
                    <small>{desc}</small>
                  </span>
                  <input
                    type="checkbox"
                    checked={settings[key]}
                    disabled={transitioning}
                    onChange={(e) => void saveSettings({ [key]: e.target.checked })}
                  />
                </label>
              ))}
            </div>
            <label className="volume">
              <Volume2 size={18} />
              Громкость
              <input
                type="range"
                min={0}
                max={100}
                value={settings.volume}
                disabled={transitioning}
                onChange={(e) => void saveSettings({ volume: Number(e.target.value) })}
              />
              <span>{settings.volume}%</span>
            </label>
            <HomeSettings
              settings={settings}
              current={() => viewport.current}
              onSave={saveSettings}
              disabled={transitioning}
            />
            {user?.role === 'administrator' && (
              <div className="admin-users">
                <h3>Права пользователей</h3>
                {users.map((u) => (
                  <label key={u.id}>
                    <span>{u.login}</span>
                    <select
                      value={u.role}
                      disabled={u.id === user.id || busy}
                      onChange={async (e) => {
                        const rows = await perform<User[]>({
                          type: 'setRole',
                          userId: u.id,
                          role: e.target.value,
                        });
                        if (rows) setUsers(rows);
                      }}
                    >
                      <option value="operator">Оператор</option>
                      <option value="administrator">Администратор</option>
                    </select>
                  </label>
                ))}
              </div>
            )}
          </Dialog>
        )}

        {modal === 'profile' && (
          <Dialog
            title="Профиль оператора"
            caption={`${user?.name} · ${user?.login} · ${user?.role === 'administrator' ? 'Администратор' : 'Оператор'}`}
            wide
            onClose={() => setModal(null)}
          >
            <div className="profile-summary">
              <Metric label="Сохранено сеансов" value={history.total} />
              <Metric label="БВС распознано" value={history.correct} className="green" />
            </div>
            <h3>История тренировок</h3>
            {history.items.length ? (
              <div className="history-table">
                <table>
                  <thead>
                    <tr>
                      <th>Дата</th>
                      <th>Режим</th>
                      <th>Время</th>
                      <th>Верно</th>
                      <th>Ошибки</th>
                    </tr>
                  </thead>
                  <tbody>
                    {history.items.map((r) => (
                      <tr key={r.snapshot.sessionId}>
                        <td>{new Date(r.date).toLocaleString('ru-RU')}</td>
                        <td>{r.snapshot.config.exam ? 'Экзамен' : 'Тренировка'}</td>
                        <td>{time(r.snapshot.simulationTimeMs)}</td>
                        <td className="green">{r.snapshot.statistics.correct}</td>
                        <td className="red">{r.snapshot.statistics.errors}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
              <p className="muted">Завершите первую тренировку, чтобы увидеть результаты.</p>
            )}
            <div className="pagination">
              <button
                className="secondary"
                disabled={busy || history.offset === 0}
                onClick={() => void openProfile(Math.max(0, history.offset - history.limit))}
              >
                ← Назад
              </button>
              <span>
                {history.total ? history.offset + 1 : 0}–{history.offset + history.items.length} из{' '}
                {history.total}
              </span>
              <button
                className="secondary"
                disabled={busy || history.offset + history.limit >= history.total}
                onClick={() => void openProfile(history.offset + history.limit)}
              >
                Далее →
              </button>
            </div>
            <details>
              <summary>Изменить пароль</summary>
              <form
                onSubmit={async (e) => {
                  e.preventDefault();
                  const form = e.currentTarget,
                    data = new FormData(form);
                  const result = await perform<null>({
                    type: 'changePassword',
                    oldPassword: String(data.get('old')),
                    newPassword: String(data.get('new')),
                  });
                  if (result === null) {
                    form.reset();
                    setMessage('Пароль изменён');
                  }
                }}
              >
                <div className="form-grid">
                  <label>
                    Текущий пароль
                    <input name="old" type="password" required autoComplete="current-password" />
                  </label>
                  <label>
                    Новый пароль
                    <input
                      name="new"
                      type="password"
                      required
                      minLength={8}
                      maxLength={128}
                      autoComplete="new-password"
                    />
                  </label>
                </div>
                <button className="secondary" disabled={busy}>
                  Сохранить пароль
                </button>
              </form>
            </details>
            <button className="secondary" disabled={busy} onClick={() => void logout()}>
              <LogOut size={16} /> Выйти из профиля
            </button>
          </Dialog>
        )}

        {modal === 'archive' && (
          <Dialog
            title="Архив событий"
            caption="Снимки рабочего места при обнаружении целей"
            wide
            onClose={() => setModal(null)}
          >
            {archiveLimitReached && (
              <p role="status">
                Лимит текущего сеанса достигнут: новые снимки не сохраняются (до 30 за сеанс).
              </p>
            )}
            {archiveImage ? (
              <>
                <button className="text-button" onClick={() => setArchiveImage('')}>
                  ← К списку событий
                </button>
                <img
                  className="archive-image"
                  src={archiveImage}
                  alt="Снимок рабочего места при тревожном событии"
                />
              </>
            ) : archives.items.length ? (
              <div className="archive-list">
                {archives.items.map((a) => (
                  <button
                    key={a.id}
                    onClick={async () => {
                      const data = await perform<string>({ type: 'archiveImageData', id: a.id });
                      if (data) setArchiveImage(data);
                    }}
                  >
                    <Camera size={21} />
                    <span>
                      <b>
                        Цель №{a.event.targetId} · {a.event.zoneName}
                      </b>
                      <small>{new Date(a.date).toLocaleString('ru-RU')}</small>
                    </span>
                    <ChevronRight size={18} />
                  </button>
                ))}
              </div>
            ) : (
              <div className="empty">
                <Camera size={32} />
                <b>Архив пока пуст</b>
                <p>Снимки сохраняются автоматически во время тренировки.</p>
              </div>
            )}
            {!archiveImage && (
              <div className="pagination">
                <button
                  className="secondary"
                  disabled={busy || archives.offset === 0}
                  onClick={() => void openArchive(Math.max(0, archives.offset - archives.limit))}
                >
                  ← Назад
                </button>
                <span>
                  {archives.total ? archives.offset + 1 : 0}–
                  {archives.offset + archives.items.length} из {archives.total}
                </span>
                <button
                  className="secondary"
                  disabled={busy || archives.offset + archives.limit >= archives.total}
                  onClick={() => void openArchive(archives.offset + archives.limit)}
                >
                  Далее →
                </button>
              </div>
            )}
          </Dialog>
        )}

        {modal === 'help' && <HelpDialog onClose={() => setModal(null)} />}

        {closeRequested && (
          <Dialog
            title="Закрыть тренажёр?"
            caption="Текущая тренировка будет завершена. Окно закроется только после сохранения результата, настроек и ожидающего снимка."
            onClose={() => {
              if (!transitioning) setCloseRequested(false);
            }}
          >
            <button
              className="primary"
              disabled={busy}
              onClick={() =>
                void transition(async () => {
                  await invoke('close_application');
                })
              }
            >
              Сохранить и закрыть
            </button>
            <button className="secondary" disabled={busy} onClick={() => setCloseRequested(false)}>
              Остаться
            </button>
          </Dialog>
        )}

        {error && (
          <div className="toast error" role="alert">
            <span>{error}</span>
            {(capture.current.pending || preferences.current.pending || snapshot.saveError) && (
              <button
                disabled={busy}
                onClick={() =>
                  void transition(async () => {
                    if (snapshotRef.current.saveError)
                      accept(
                        await request<Snapshot>({ type: 'finish', sessionId: session.current }),
                      );
                    setMessage('Сохранение выполнено');
                  })
                }
              >
                Повторить сохранение
              </button>
            )}
            <button aria-label="Закрыть ошибку" onClick={() => setError('')}>
              <X size={16} />
            </button>
          </div>
        )}
        {message && (
          <div className="toast success" role="status">
            <Check size={17} />
            {message}
          </div>
        )}
      </div>
    </ErrorContext.Provider>
  );
}
