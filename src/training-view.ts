import type { Config, Snapshot } from './types';

/** Never use an editable draft to label or score a confirmed running/finished session. */
export function trainingViewConfig(snapshot: Snapshot, draft: Config): Config {
  return snapshot.status === 'ready' ? draft : snapshot.config;
}
