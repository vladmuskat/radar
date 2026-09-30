import { invoke, isTauri } from '@tauri-apps/api/core';

type Event =
  | 'unhandled_error'
  | 'unhandled_rejection'
  | 'react_render_failed'
  | 'ipc_failed'
  | 'map_failed'
  | 'capture_failed'
  | 'audio_failed';
const kinds = new Set([
  'Error',
  'TypeError',
  'RangeError',
  'ReferenceError',
  'SyntaxError',
  'DOMException',
]);

/** Keep source locations, not the stack's first line (which contains the error message). */
export function safeFrames(error: unknown): { script: string; line: number; column: number }[] {
  if (!(error instanceof Error)) return [];
  return (error.stack ?? '')
    .split('\n')
    .slice(1)
    .flatMap((line) => {
      const match = line.match(/[\\/]([A-Za-z0-9_.-]{1,100}\.(?:js|tsx?)):(\d{1,9}):(\d{1,9})/);
      return match ? [{ script: match[1], line: Number(match[2]), column: Number(match[3]) }] : [];
    })
    .slice(0, 8);
}

/** No raw messages, command arguments, console interception, tokens or image data. */
export function logFailure(event: Event, error: unknown, requestId?: string): void {
  const kind = error instanceof Error && kinds.has(error.name) ? error.name : 'Unknown';
  if (!isTauri()) {
    console.warn('[radar]', event, kind, requestId ?? '');
    return;
  }
  // Do not use request(): logging failures must never recursively log themselves.
  void invoke('frontend_log', { event, kind, requestId, frames: safeFrames(error) }).catch(
    () => {},
  );
}

/** Installs global browser failure handlers and returns their cleanup callback. */
export function installGlobalLogging(): () => void {
  const onError = (event: ErrorEvent) => logFailure('unhandled_error', event.error);
  const onRejection = (event: PromiseRejectionEvent) =>
    logFailure('unhandled_rejection', event.reason);
  window.addEventListener('error', onError);
  window.addEventListener('unhandledrejection', onRejection);
  return () => {
    window.removeEventListener('error', onError);
    window.removeEventListener('unhandledrejection', onRejection);
  };
}
