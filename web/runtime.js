// This module is copied into the wasm-bindgen output; all paths remain relative
// so the same build works under /VoxelPopuli/ on GitHub Pages.
import { openWorldStore, readWorld, writeWorld } from './storage.js';

let canvas;
let events = [];
const keys = new Set();
let cursor = [0, 0];
let desiredLock = false;
let playable = false;
let started = false;
let saveRequested = false;
let database;
const keyCodes = {
  Space: 32, Digit1: 49, Digit2: 50, Digit3: 51, Digit4: 52, Digit5: 53,
  Digit6: 54, Digit7: 55, Digit8: 56, Digit9: 57,
  KeyA: 65, KeyD: 68, KeyE: 69, KeyQ: 81, KeyS: 83, KeyW: 87,
  Escape: 256, F3: 292, F6: 295, F11: 300, ShiftLeft: 340, ControlLeft: 341,
};
const mods = event => (event.shiftKey ? 1 : 0) | (event.ctrlKey ? 2 : 0);
const element = id => document.getElementById(id);
const isLocked = () => document.pointerLockElement === canvas;

export function saveStatus(message) { element('save-status').textContent = message; }
export function error(message) {
  element('status').textContent = message;
  element('status').hidden = false;
  element('enter').hidden = true;
}
export function finished() {
  desiredLock = false;
  document.exitPointerLock();
  playable = false;
  element('status').textContent = 'World saved. Reload the page to play again.';
  element('status').hidden = false;
  element('enter').hidden = true;
}
export function ready() {
  if (playable) return;
  playable = true;
  desiredLock = true;
  element('status').hidden = true;
  element('enter').hidden = false;
}
function requestLock() {
  if (!playable || !desiredLock || isLocked()) return;
  try {
    const request = canvas.requestPointerLock();
    request?.catch(() => { element('enter').hidden = false; });
  } catch { element('enter').hidden = false; }
}
export function cursorMode(locked) {
  desiredLock = locked;
  if (locked) requestLock();
  else {
    document.exitPointerLock();
    element('enter').hidden = true;
  }
}
export function gameCanvas() { return canvas; }
export function hasStarted() { return started; }
export function drainEvents() { const result = events; events = []; return result; }
export function keyDown(code) { return playable && keys.has(code); }
export function cursorPosition() { return cursor; }
export function takeSaveRequest() { const value = saveRequested; saveRequested = false; return value; }
export function nextFrame() { return new Promise(resolve => requestAnimationFrame(resolve)); }
export function toggleFullscreen() {
  const request = document.fullscreenElement ? document.exitFullscreen() : element('game').requestFullscreen();
  request?.catch(() => saveStatus('Fullscreen is unavailable. You can still play in this window.'));
}
export function gamepadState(index) {
  if (!playable || !isLocked()) return null;
  const pad = navigator.getGamepads?.()[index];
  if (!pad || pad.mapping !== 'standard') return null;
  return [pad.axes[0] ?? 0, pad.axes[1] ?? 0, pad.axes[2] ?? 0, pad.axes[3] ?? 0,
    (pad.buttons[6]?.value ?? 0) * 2 - 1, (pad.buttons[7]?.value ?? 0) * 2 - 1,
    ...pad.buttons.map(button => button.pressed)];
}
function releaseInput() {
  keys.clear();
  events.push([2, 0, 0, 0], [2, 1, 0, 0], [4]);
  saveRequested = true;
}
export function setup() {
  canvas = element('canvas');
  const resize = () => {
    // A CSS-pixel framebuffer keeps UI hit targets and pointer coordinates
    // identical, and avoids multiplying the GPU cost on high-DPI screens.
    canvas.width = Math.max(1, Math.floor(canvas.clientWidth));
    canvas.height = Math.max(1, Math.floor(canvas.clientHeight));
  };
  new ResizeObserver(resize).observe(canvas);
  resize();
  element('enter').addEventListener('click', () => { canvas.focus(); requestLock(); });
  element('fullscreen').addEventListener('click', toggleFullscreen);
  element('save').addEventListener('click', () => { saveRequested = true; });
  canvas.addEventListener('contextmenu', event => event.preventDefault());
  document.addEventListener('keydown', event => {
    if (!playable || (!isLocked() && document.activeElement !== canvas)) return;
    const code = keyCodes[event.code];
    if (code === undefined) return;
    event.preventDefault();
    // Esc while locked is handled by pointerlockchange. Sending both would
    // pause and immediately resume when the browser releases the pointer.
    if (code === 256 && isLocked()) return;
    if (code === 300) { if (!event.repeat) toggleFullscreen(); return; }
    keys.add(code);
    events.push([0, code, event.repeat ? 2 : 1, mods(event)]);
  });
  document.addEventListener('keyup', event => {
    const code = keyCodes[event.code];
    if (code === undefined) return;
    keys.delete(code);
    events.push([0, code, 0, mods(event)]);
  });
  canvas.addEventListener('mousemove', event => {
    const rect = canvas.getBoundingClientRect();
    cursor = [(event.clientX - rect.left) * canvas.width / rect.width,
      (event.clientY - rect.top) * canvas.height / rect.height];
    if (isLocked()) {
      events.push([5, event.movementX, event.movementY]);
    } else {
      events.push([1, ...cursor]);
    }
  });
  canvas.addEventListener('mousedown', event => {
    if (!playable || event.button === 1) return;
    canvas.focus();
    if (desiredLock && !isLocked()) { requestLock(); return; }
    events.push([2, event.button === 2 ? 1 : 0, 1, mods(event)]);
  });
  document.addEventListener('mouseup', event => {
    if (event.button === 1) return;
    events.push([2, event.button === 2 ? 1 : 0, 0, mods(event)]);
  });
  canvas.addEventListener('wheel', event => {
    if (!playable) return;
    event.preventDefault();
    events.push([3, 0, -Math.sign(event.deltaY)]);
  }, { passive: false });
  document.addEventListener('pointerlockchange', () => {
    if (isLocked()) {
      started = true;
      element('enter').hidden = true;
    } else if (desiredLock && playable) {
      releaseInput();
      desiredLock = false;
      // The game draws its pause menu; clicking Resume reacquires the pointer.
    }
  });
  window.addEventListener('blur', releaseInput);
  document.addEventListener('visibilitychange', () => {
    if (document.hidden) releaseInput();
  });
}

export async function loadSave() {
  database = await openWorldStore(indexedDB);
  return readWorld(database);
}
export async function storeSave(bytes) {
  if (!database) throw new Error('World storage has not been opened');
  // wasm memory can grow or be reused as soon as this function yields.
  const snapshot = bytes.slice();
  await writeWorld(database, snapshot);
}
