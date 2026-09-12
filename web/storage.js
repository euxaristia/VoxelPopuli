// IndexedDB transactions preserve the previous save if a write fails.
export function openWorldStore(indexedDB) {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('VoxelPopuli', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('worlds');
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error('Close other VoxelPopuli tabs and reload to open world storage.'));
    request.onsuccess = () => {
      const database = request.result;
      database.onversionchange = () => database.close();
      resolve(database);
    };
  });
}
export function readWorld(database) {
  return new Promise((resolve, reject) => {
    const tx = database.transaction('worlds', 'readonly');
    const request = tx.objectStore('worlds').get('survival');
    tx.oncomplete = () => resolve(request.result);
    tx.onabort = () => reject(tx.error ?? new Error('World read was aborted'));
    tx.onerror = () => reject(tx.error);
  });
}
export function writeWorld(database, bytes) {
  return new Promise((resolve, reject) => {
    const tx = database.transaction('worlds', 'readwrite');
    tx.objectStore('worlds').put(bytes, 'survival');
    tx.oncomplete = () => resolve();
    tx.onabort = () => reject(tx.error ?? new Error('World save was aborted'));
    tx.onerror = () => reject(tx.error);
  });
}
