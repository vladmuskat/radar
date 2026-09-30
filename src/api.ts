import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import type { Command, Config, Settings, Snapshot } from './types';
import { CENTER } from './terrain';
import region from '../map-region.json';
import { logFailure } from './logging';
import { normalizeSettings } from './settings-validation';

export const desktop = isTauri();
export const defaults: Settings = {
  direction: false,
  trails: true,
  sound: false,
  volume: 30,
  archive: true,
  home: CENTER,
  zoom: region.defaultZoom,
};

/** Retain user preferences, but do not fly back to the old synthetic map region. */
export function restoreSettings(saved: unknown): Settings {
  return normalizeSettings(saved, defaults, region.bbox, region.minZoom, region.maxZoom);
}
export const emptyConfig: Config = {
  durationSeconds: 120,
  maxTargets: 12,
  seed: 2401,
  exam: false,
  zones: [],
};
export const emptySnapshot: Snapshot = {
  sessionId: '',
  sequence: 0,
  status: 'ready',
  simulationTimeMs: 0,
  sweepBearing: 0,
  remainingTimeMs: 120000,
  targets: [],
  notifications: [],
  config: emptyConfig,
  statistics: {
    marked: 0,
    correct: 0,
    errors: 0,
    missed: 0,
    averageReactionMs: 0,
    reactionsMs: [],
  },
};

/** UI-only development preview. The simulation is only available in the desktop app. */
export async function request<T>(command: Command): Promise<T> {
  if (!desktop) {
    if (command.type === 'bootstrap')
      return { user: null, config: emptyConfig, settings: defaults } as T;
    throw new Error(
      'Запустите desktop-приложение командой npm run desktop. В браузере доступен только предпросмотр интерфейса.',
    );
  }
  const requestId = crypto.randomUUID();
  try {
    return await invoke<T>('request', { command, requestId });
  } catch (error) {
    logFailure('ipc_failed', error, requestId);
    throw error;
  }
}

/** Opens the desktop snapshot channel and returns its initial acknowledged state. */
export async function subscribe(onSnapshot: (snapshot: Snapshot) => void): Promise<Snapshot> {
  const channel = new Channel<Snapshot>();
  channel.onmessage = onSnapshot;
  try {
    return await invoke<Snapshot>('subscribe', { channel });
  } catch (error) {
    logFailure('ipc_failed', error);
    throw error;
  }
}

/** Formats a simulation duration as minutes and seconds. */
export function time(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  return `${Math.floor(seconds / 60)
    .toString()
    .padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
}
