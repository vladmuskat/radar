import { test } from 'node:test';
import assert from 'node:assert/strict';
import { normalizeSettings } from '../src/settings-validation.ts';
import { trainingViewConfig } from '../src/training-view.ts';

const defaults = { direction: false, trails: true, sound: false, volume: 30, archive: true, home: [30.26, 59.88], zoom: 12 };
const normalize = (value) => normalizeSettings(value, defaults, [30.08,59.79,30.44,59.97], 11, 16);

test('invalid persisted preferences cannot supply booleans, audio gain or map positions', () => {
  for (const volume of [-1, 101, 3.5, NaN, Infinity, null, '80']) {
    assert.equal(normalize({ volume }).volume, 30);
  }
  assert.deepEqual(normalize(null), defaults);
  assert.deepEqual(normalize({ sound: 'yes', direction: 1, archive: null, home: [0,0], zoom: 100 }), defaults);
  assert.equal(normalize({ volume: 100 }).volume, 100);
  assert.equal(normalize({ volume: 0 }).volume, 0);
  assert.equal(normalize({ sound: true }).sound, true);
});

test('confirmed session configuration wins over edits made while startup was pending', () => {
  const submitted = { exam: true, durationSeconds: 120, maxTargets: 12, seed: 1, zones: [] };
  const editedDraft = { ...submitted, exam: false, durationSeconds: 30 };
  for (const status of ['running', 'paused', 'finished']) {
    assert.equal(trainingViewConfig({ status, config: submitted }, editedDraft), submitted);
  }
  assert.equal(trainingViewConfig({ status: 'ready', config: submitted }, editedDraft), editedDraft);
});
