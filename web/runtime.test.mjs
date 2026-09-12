import { test } from 'node:test';
import assert from 'node:assert/strict';

class Target {
  handlers = new Map();
  hidden = false;
  textContent = '';
  clientWidth = 800;
  clientHeight = 600;
  addEventListener(name, handler) { this.handlers.set(name, handler); }
  fire(name, event = {}) { this.handlers.get(name)?.({ preventDefault() {}, ...event }); }
  getBoundingClientRect() { return { left: 20, top: 68, width: 800, height: 600 }; }
  focus() { document.activeElement = this; }
  requestPointerLock() { document.pointerLockElement = this; document.fire('pointerlockchange'); return Promise.resolve(); }
}
const elements = Object.fromEntries(['canvas', 'enter', 'save', 'fullscreen', 'status', 'save-status'].map(id => [id, new Target()]));
globalThis.document = new Target();
document.getElementById = id => elements[id];
document.exitPointerLock = () => { document.pointerLockElement = null; document.fire('pointerlockchange'); };
globalThis.window = new Target();
globalThis.ResizeObserver = class { constructor(callback) { this.callback = callback; } observe() { this.callback(); } };
const runtime = await import('./runtime.js');
runtime.setup();
runtime.ready();
elements.canvas.focus();

test('simulation waits for the first successful pointer capture', () => {
  assert.equal(runtime.hasStarted(), false);
  runtime.cursorMode(true);
  assert.equal(runtime.hasStarted(), true);
  runtime.cursorMode(false);
  assert.equal(runtime.hasStarted(), true);
});

test('relative look motion remains continuous after opening the inventory', () => {
  runtime.cursorMode(true);
  elements.canvas.fire('mousemove', { clientX: 100, clientY: 100, movementX: 7, movementY: -3 });
  assert.deepEqual(runtime.drainEvents(), [[5, 7, -3]]);
  runtime.cursorMode(false);
  elements.canvas.fire('mousemove', { clientX: 420, clientY: 368, movementX: 0, movementY: 0 });
  assert.deepEqual(runtime.cursorPosition(), [400, 300]);
  assert.deepEqual(runtime.drainEvents(), [[1, 400, 300]]);
  runtime.cursorMode(true);
  elements.canvas.fire('mousemove', { clientX: 400, clientY: 300, movementX: 2, movementY: 1 });
  assert.deepEqual(runtime.drainEvents(), [[5, 2, 1]]);
});

test('Escape releases held inputs and produces one pause event', () => {
  document.fire('keydown', { code: 'KeyW' });
  assert.equal(runtime.keyDown(87), true);
  runtime.drainEvents();
  document.fire('keydown', { code: 'Escape' });
  assert.deepEqual(runtime.drainEvents(), []);
  document.exitPointerLock();
  assert.equal(runtime.keyDown(87), false);
  assert.deepEqual(runtime.drainEvents(), [[2, 0, 0, 0], [2, 1, 0, 0], [4]]);
  assert.equal(runtime.takeSaveRequest(), true);
});

test('losing focus releases movement and both mouse buttons', () => {
  document.fire('keydown', { code: 'KeyW' });
  runtime.drainEvents();
  window.fire('blur');
  assert.equal(runtime.keyDown(87), false);
  assert.deepEqual(runtime.drainEvents(), [[2, 0, 0, 0], [2, 1, 0, 0], [4]]);
});

test('clicking to capture the pointer does not mine a block', () => {
  runtime.cursorMode(true);
  // Simulate the browser rejecting a lock request without a user gesture.
  document.pointerLockElement = null;
  elements.canvas.fire('mousedown', { button: 0 });
  assert.deepEqual(runtime.drainEvents(), []);
  assert.equal(document.pointerLockElement, elements.canvas);
});
