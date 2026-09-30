import { test } from 'node:test';
import assert from 'node:assert/strict';
import { safeFrames } from '../src/logging.ts';

test('frontend diagnostics omit message, query parameters and arbitrary objects', () => {
  const error = new Error('password=DO_NOT_LOG');
  error.stack = 'Error: password=DO_NOT_LOG\n at run (http://localhost/src/App.tsx:42:9)\n at secret (http://localhost/a.js?token=DO_NOT_LOG:1:2)';
  assert.deepEqual(safeFrames(error), [{ script: 'App.tsx', line: 42, column: 9 }]);
  assert.deepEqual(safeFrames({ password: 'DO_NOT_LOG' }), []);
});

test('frontend stack locations are bounded', () => {
  const error = new Error('private');
  error.stack = 'Error: private\n' + ' at run (http://localhost/index-123.js:1:2)\n'.repeat(30);
  assert.equal(safeFrames(error).length, 8);
});
