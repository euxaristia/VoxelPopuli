import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import test from 'node:test';

test('browser input smoke test requires consent before launching anything', () => {
  const result = spawnSync(process.execPath,
    ['scripts/test-web-browser.mjs', 'target/browser-that-must-not-launch'],
    { encoding: 'utf8', timeout: 5000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /requires explicit desktop-input consent/);
});
