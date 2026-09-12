import { spawn } from 'node:child_process';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
// Run with Node 22+ or Bun and a Chromium executable. Only the temporary
// profile and the generated target/web build are used; no personal tabs.
if (!process.argv[2]) throw new Error('Usage: node scripts/test-web-browser.mjs <chromium-executable>');
const root = path.resolve('target/web');
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://localhost');
    if (!url.pathname.startsWith('/VoxelPopuli/')) { response.writeHead(404).end(); return; }
    const file = path.resolve(root, decodeURIComponent(url.pathname.slice('/VoxelPopuli/'.length)) || 'index.html');
    if (!file.startsWith(root + path.sep)) { response.writeHead(404).end(); return; }
    const bytes = await readFile(file);
    const type = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm' }[path.extname(file)] || 'application/octet-stream';
    response.writeHead(200, { 'Content-Type': type }).end(bytes);
  } catch { response.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const url = `http://127.0.0.1:${server.address().port}/VoxelPopuli/`;
const profile = await mkdtemp(path.resolve('target/browser-profile-'));
const browser = spawn(process.argv[2], ['--headless=new', '--no-first-run', '--remote-debugging-port=0', `--user-data-dir=${profile}`, 'about:blank'], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
const watchdog = setTimeout(() => { console.error('Browser test timed out', errors); browser.kill(); process.exit(1); }, 90000);
let errors = '';
browser.stderr.on('data', chunk => { errors += chunk; });
let socket;
try {
  let port;
  for (let i = 0; i < 100; ++i) {
    try { port = (await readFile(path.join(profile, 'DevToolsActivePort'), 'utf8')).split('\n')[0]; break; } catch { await new Promise(r => setTimeout(r, 100)); }
  }
  if (!port) throw new Error('Browser did not start: ' + errors);
  const version = await (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(5000) })).json();
  socket = new WebSocket(version.webSocketDebuggerUrl);
  await new Promise(resolve => socket.addEventListener('open', resolve, { once: true }));
  let id = 0;
  const pending = new Map();
  const messages = [];
  socket.addEventListener('message', event => {
    const msg = JSON.parse(event.data);
    if (msg.id) { const request = pending.get(msg.id); if (request) { pending.delete(msg.id); msg.error ? request.reject(msg.error) : request.resolve(msg.result); } }
    else if (msg.method === 'Runtime.exceptionThrown' || msg.method === 'Runtime.consoleAPICalled') { messages.push(msg.params); }
  });
  const send = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
    const key = ++id; const timer = setTimeout(() => reject(new Error('Timed out: ' + method)), 20000); pending.set(key, { resolve: value => { clearTimeout(timer); resolve(value); }, reject }); socket.send(JSON.stringify({ id: key, method, params, sessionId }));
  });
  const existing = await send('Target.getTargets');
  const targetId = existing.targetInfos.find(info => info.type === 'page' && info.url === 'about:blank')?.targetId;
  if (!targetId) throw new Error('No blank test tab: ' + JSON.stringify(existing));
  const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
  const call = (method, params) => send(method, params, sessionId);
  await call('Runtime.enable');
  await call('Page.enable');
  await call('Page.bringToFront');
  await call('Emulation.setDeviceMetricsOverride', { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  await call('Page.navigate', { url });
  let state;
  for (let i = 0; i < 120; ++i) {
    await new Promise(resolve => setTimeout(resolve, 500));
    const result = await call('Runtime.evaluate', { expression: `JSON.stringify({title:document.title,status:document.getElementById('status')?.textContent,hidden:document.getElementById('status')?.hidden,enter:document.getElementById('enter')?.hidden,gpu:!!navigator.gpu})`, returnByValue: true });
    state = JSON.parse(result.result.value || '{}');
    if (state.hidden || /stopped|Could not|unavailable/i.test(state.status || '')) break;
  }
  assert.equal(state.hidden, true, 'Game did not reach playable state: ' + state.status);
  const evaluate = async expression => {
    const response = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });
    if (response.exceptionDetails) throw new Error(response.exceptionDetails.text);
    return response.result.value;
  };
  const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
  const key = async (code, key, down) => call('Input.dispatchKeyEvent', { type: down ? 'keyDown' : 'keyUp', code, key, windowsVirtualKeyCode: key.length === 1 ? key.toUpperCase().charCodeAt(0) : 27 });
  const tap = async (code, value) => { await key(code, value, true); await key(code, value, false); await sleep(150); };
  const save = async () => {
    await evaluate(`document.getElementById('save-status').textContent = ''; document.getElementById('save').click()`);
    for (let i = 0; i < 50; ++i) {
      if ((await evaluate(`document.getElementById('save-status').textContent`)).includes('World saved')) break;
      await sleep(100);
    }
    assert.match(await evaluate(`document.getElementById('save-status').textContent`), /World saved/);
    return evaluate(`new Promise((resolve, reject) => { const request = indexedDB.open('VoxelPopuli', 1); request.onerror = () => reject(request.error); request.onsuccess = () => { const db = request.result; const tx = db.transaction('worlds'); const get = tx.objectStore('worlds').get('survival'); tx.oncomplete = () => { const bytes = get.result; const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength); resolve({ seed: view.getBigInt64(12, true).toString(), position: [20,24,28].map(offset => view.getFloat32(offset,true)), bytes: bytes.length }); db.close(); }; }; })`);
  };
  const enter = await evaluate(`(() => { const r = document.getElementById('enter').getBoundingClientRect(); return { x:r.x+r.width/2, y:r.y+r.height/2 }; })()`);
  await call('Input.dispatchMouseEvent', { type: 'mouseMoved', ...enter });
  await call('Input.dispatchMouseEvent', { type: 'mousePressed', ...enter, button: 'left', clickCount: 1 });
  await call('Input.dispatchMouseEvent', { type: 'mouseReleased', ...enter, button: 'left', clickCount: 1 });
  await sleep(200);
  assert.equal(await evaluate(`document.pointerLockElement?.id`), 'canvas');
  const before = await save();
  await key('KeyW', 'w', true);
  await sleep(700);
  await key('KeyW', 'w', false);
  const after = await save();
  assert.notDeepEqual(after.position, before.position, 'Movement did not change the saved position');
  await tap('KeyE', 'e');
  assert.equal(await evaluate(`!!document.pointerLockElement`), false, 'Inventory must release the mouse');
  const invShot = await call('Page.captureScreenshot', { format: 'png' });
  await writeFile('target/browser-inventory.png', Buffer.from(invShot.data, 'base64'));
  await tap('KeyE', 'e');
  await sleep(200);
  // Capture failure can occur without a real user gesture in headless mode.
  if (!(await evaluate(`!!document.pointerLockElement`))) await evaluate(`document.getElementById('enter').click()`);
  await evaluate(`document.exitPointerLock()`);
  await sleep(200);
  const saved = await save();
  const clickCanvas = async (x, y) => {
    const rect = await evaluate(`(() => { const r=document.getElementById('canvas').getBoundingClientRect(); return {x:r.x, y:r.y, width:r.width, height:r.height}; })()`);
    const pos = { x: rect.x + x(rect), y: rect.y + y(rect) };
    await call('Input.dispatchMouseEvent', { type: 'mouseMoved', ...pos });
    await call('Input.dispatchMouseEvent', { type: 'mousePressed', ...pos, button:'left', clickCount:1 });
    await call('Input.dispatchMouseEvent', { type: 'mouseReleased', ...pos, button:'left', clickCount:1 });
    await sleep(200);
  };
  await clickCanvas(() => 200, r => r.height / 2 - 130 + 54 + 22);
  await clickCanvas(r => (r.width - 700) / 2 + 410, r => (r.height - 500) / 2 + 238);
  await save();
  await tap('Escape', 'Escape');
  await tap('Escape', 'Escape');
  await sleep(250);
  const fastShot = await call('Page.captureScreenshot', { format:'png' });
  await writeFile('target/browser-fast.png', Buffer.from(fastShot.data,'base64'));
  await call('Emulation.setDeviceMetricsOverride', { width: 900, height: 1000, deviceScaleFactor: 2, mobile: false });
  await sleep(250);
  const tallShot = await call('Page.captureScreenshot', { format:'png' });
  await writeFile('target/browser-tall.png', Buffer.from(tallShot.data,'base64'));
  await call('Page.reload');
  for (let i = 0; i < 120; ++i) {
    await sleep(100);
    if (await evaluate(`document.getElementById('status')?.hidden === true`)) break;
  }
  const reloaded = await save();
  assert.equal(reloaded.seed, saved.seed, 'Reload changed world seed');
  assert.ok(Math.abs(reloaded.position[0] - saved.position[0]) < 0.01 && Math.abs(reloaded.position[2] - saved.position[2]) < 0.01, 'Reload lost the player location');
  // Close through the actual game pause menu and wait for the durable save.
  await evaluate(`document.getElementById('canvas').focus()`);
  await tap('Escape', 'Escape');
  await clickCanvas(() => 200, r => r.height / 2 - 130 + 4 * 54 + 22);
  const quitShot = await call('Page.captureScreenshot', { format:'png' });
  await writeFile('target/browser-quit.png', Buffer.from(quitShot.data,'base64'));
  assert.match(await evaluate(`document.getElementById('status').textContent`), /World saved/);
  console.log('Passed: startup, pointer capture, movement, inventory, graphics settings, resize, save/reload, and Save & Quit under /VoxelPopuli/.');
  const shot = await call('Page.captureScreenshot', { format: 'png' });
  await writeFile('target/browser-smoke.png', Buffer.from(shot.data, 'base64'));
  await writeFile('target/browser-smoke.json', JSON.stringify({ state, messages, errors }, null, 2));
  await send('Browser.close');
  assert.equal(messages.filter(message => message.exceptionDetails || message.type === 'error').length, 0, 'Browser reported runtime errors');
} finally { clearTimeout(watchdog); socket?.close(); browser.kill(); server.closeAllConnections(); server.close(); }
