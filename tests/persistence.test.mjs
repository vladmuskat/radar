import { test } from 'node:test';
import assert from 'node:assert/strict';
import { Preferences, PendingCapture } from '../src/persistence.ts';

const deferred = () => {
  let resolve;
  const promise = new Promise((r) => { resolve = r; });
  return { promise, resolve };
};

test('rapid patches merge immediately and writes are serialized', async () => {
  const delay = deferred(), writes = [];
  const p = new Preferences({ sound: false, trails: true }, async (value) => {
    writes.push(value);
    if (writes.length === 1) await delay.promise;
  });
  const first = p.patch({ sound: true });
  const second = p.patch({ trails: false });
  assert.deepEqual(p.current, { sound: true, trails: false });
  await Promise.resolve();
  assert.equal(writes.length, 1);
  delay.resolve();
  await Promise.all([first, second]);
  assert.deepEqual(writes.at(-1), p.current);
});

test('failed preferences keep current UI value and flush retries the latest value', async () => {
  let fail = true, stored;
  const p = new Preferences({ volume: 30 }, async (value) => {
    if (fail) throw Error('SQLite');
    stored = value;
  });
  await assert.rejects(p.patch({ volume: 80 }));
  assert.equal(p.current.volume, 80);
  fail = false;
  await p.flush();
  assert.equal(stored.volume, 80);
});

test('session transition waits for delayed PNG and its durable write', async () => {
  const delay = deferred(), q = new PendingCapture(), order = [];
  const capture = q.start(() => delay.promise, async (png) => { order.push(png); });
  const transition = q.flush().then(() => order.push('new session'));
  await Promise.resolve();
  assert.deepEqual(order, []);
  delay.resolve('old session PNG');
  await Promise.all([capture, transition]);
  assert.deepEqual(order, ['old session PNG', 'new session']);
  assert.equal(q.pending, false);
});

test('failed PNG upload retains original bytes and blocks transition until retry', async () => {
  const q = new PendingCapture();
  let fail = true, captures = 0, saved;
  await assert.rejects(q.start(async () => { captures++; return 'original PNG'; }, async (png) => {
    if (fail) throw Error('SQLite');
    saved = png;
  }));
  assert.equal(q.pending, true);
  await assert.rejects(q.flush());
  fail = false;
  await q.flush();
  assert.equal(saved, 'original PNG');
  assert.equal(captures, 1);
});
