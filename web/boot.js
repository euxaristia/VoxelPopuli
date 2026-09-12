const status = document.getElementById('status');
function failure(message) {
  status.hidden = false;
  status.textContent = message;
  document.getElementById('enter').hidden = true;
}
window.addEventListener('error', event => failure(`The game stopped: ${event.message}. Reload to restore your last save.`));
window.addEventListener('unhandledrejection', event => failure(`The game stopped: ${event.reason}. Reload to restore your last save.`));
if (!navigator.gpu) {
  failure('WebGPU is unavailable. Open this page in a WebGPU-enabled desktop browser with hardware acceleration turned on.');
} else {
  try {
    // Prevent two tabs from silently overwriting the same saved world.
    if (!navigator.locks) throw new Error('This browser does not support the lock needed to protect saved worlds.');
    await navigator.locks.request('VoxelPopuli-world', { ifAvailable: true }, async lock => {
      if (!lock) { failure('VoxelPopuli is already open in another tab. Close that tab, then reload this page.'); return; }
      const { default: init } = await import('./pkg/voxelpopuli.js');
      await init();
      await new Promise(() => {});
    });
  } catch (error) { failure(`Could not start VoxelPopuli: ${error.message}`); }
}
