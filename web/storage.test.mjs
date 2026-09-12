import { test } from 'node:test';
import assert from 'node:assert/strict';
import { openWorldStore, readWorld, writeWorld } from './storage.js';

function controlledStore(initial) {
  let saved = initial;
  let pending;
  let request;
  const transaction = {
    error: null,
    objectStore() {
      return {
        get() { request = { result: saved }; return request; },
        put(value) { pending = value; },
      };
    },
  };
  return {
    database: { transaction() { return transaction; } },
    transaction,
    commit() { if (pending !== undefined) saved = pending; transaction.oncomplete(); },
    abort() { transaction.error = new Error('quota exceeded'); transaction.onabort(); },
    saved() { return saved; },
  };
}

test('a save is acknowledged only after its transaction commits', async () => {
  const store = controlledStore(new Uint8Array([1]));
  let acknowledged = false;
  const saving = writeWorld(store.database, new Uint8Array([2])).then(() => { acknowledged = true; });
  await Promise.resolve();
  assert.equal(acknowledged, false);
  assert.deepEqual(store.saved(), new Uint8Array([1]));
  store.commit();
  await saving;
  assert.equal(acknowledged, true);
  assert.deepEqual(store.saved(), new Uint8Array([2]));
});

test('an aborted save rejects and preserves the previous world', async () => {
  const store = controlledStore(new Uint8Array([1]));
  const saving = writeWorld(store.database, new Uint8Array([2]));
  store.abort();
  await assert.rejects(saving, /quota exceeded/);
  assert.deepEqual(store.saved(), new Uint8Array([1]));
});

test('loading waits for completion and distinguishes a missing world', async () => {
  for (const initial of [undefined, new Uint8Array([3, 4])]) {
    const store = controlledStore(initial);
    const loading = readWorld(store.database);
    store.commit();
    assert.deepEqual(await loading, initial);
  }
});

test('a failed read is never treated as a new world', async () => {
  const store = controlledStore(new Uint8Array([7]));
  const loading = readWorld(store.database);
  store.abort();
  await assert.rejects(loading, /quota exceeded/);
});

test('blocked database opening reports an actionable error', async () => {
  const request = {};
  const opening = openWorldStore({ open() { return request; } });
  request.onblocked();
  await assert.rejects(opening, /Close other VoxelPopuli tabs/);
});
