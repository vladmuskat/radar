export type Point = { x: number; y: number };
export type Zone = { id: string; name: string; kind: 'detection' | 'ignore'; points: Point[] };
export type Config = {
  durationSeconds: number;
  maxTargets: number;
  seed: number;
  exam: boolean;
  zones: Zone[];
};
export type Target = {
  id: number;
  position: Point;
  longitude: number;
  latitude: number;
  speedMps: number;
  observedAtMs: number;
  identified: boolean;
  trail: Point[];
};
export type Notice = {
  id: number;
  targetId: number;
  zoneName: string;
  timeMs: number;
  position: Point;
  speedMps: number;
};
export type Statistics = {
  marked: number;
  correct: number;
  errors: number;
  missed: number;
  averageReactionMs: number;
  reactionsMs: number[];
};
export type Snapshot = {
  sessionId: string;
  sequence: number;
  status: 'ready' | 'running' | 'paused' | 'finished';
  simulationTimeMs: number;
  sweepBearing: number;
  remainingTimeMs: number;
  targets: Target[];
  notifications: Notice[];
  statistics: Statistics;
  config: Config;
  saveError?: string | null;
};
export type User = { id: number; login: string; name: string; role: 'administrator' | 'operator' };
export type Settings = {
  direction: boolean;
  trails: boolean;
  sound: boolean;
  volume: number;
  archive: boolean;
  home: [number, number];
  zoom: number;
};
export type HistoryItem = { date: string; snapshot: Snapshot };
export type HistoryPage = {
  items: HistoryItem[];
  total: number;
  correct: number;
  offset: number;
  limit: number;
};
export type Archive = { id: number; date: string; event: Notice };
export type ArchiveOutcome = 'saved' | 'alreadySaved' | 'limitReached';
export type ArchivePage = { items: Archive[]; total: number; offset: number; limit: number };
export type Command =
  | { type: 'bootstrap' | 'logout' | 'users' }
  | { type: 'history' | 'archives'; offset?: number }
  | { type: 'login'; login: string; password: string }
  | { type: 'create' | 'validateConfig'; config: Config }
  | {
      type: 'start' | 'pause' | 'pauseForTransition' | 'resume' | 'finish' | 'reset';
      sessionId: string;
    }
  | { type: 'identify'; sessionId: string; targetId: number }
  | { type: 'changePassword'; oldPassword: string; newPassword: string }
  | { type: 'setRole'; userId: number; role: string }
  | { type: 'saveSettings'; settings: Settings }
  | { type: 'archiveImage'; sessionId: string; eventId: number; dataUrl: string }
  | { type: 'reserveArchive' | 'cancelArchive'; sessionId: string; eventId: number }
  | { type: 'archiveImageData'; id: number };
