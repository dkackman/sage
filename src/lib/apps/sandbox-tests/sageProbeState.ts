const PROBE_LOCAL_STORAGE_KEY = 'sage_probe_local_storage';
const PROBE_INDEXED_DB_NAME = 'sage_probe_db';
const PROBE_INDEXED_DB_STORE = 'probe_store';
const PROBE_INDEXED_DB_KEY = 'sage_probe_key';

export async function writeSageProbeIndexedDb(runId: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const open = indexedDB.open(PROBE_INDEXED_DB_NAME);

    open.onerror = () => {
      reject(new Error('Failed to open Sage probe IndexedDB.'));
    };

    open.onupgradeneeded = () => {
      const db = open.result;
      if (!db.objectStoreNames.contains(PROBE_INDEXED_DB_STORE)) {
        db.createObjectStore(PROBE_INDEXED_DB_STORE);
      }
    };

    open.onsuccess = () => {
      try {
        const db = open.result;
        const tx = db.transaction(PROBE_INDEXED_DB_STORE, 'readwrite');
        const store = tx.objectStore(PROBE_INDEXED_DB_STORE);
        const put = store.put(runId, PROBE_INDEXED_DB_KEY);

        put.onerror = () => {
          db.close();
          reject(new Error('Failed to write Sage probe IndexedDB record.'));
        };

        put.onsuccess = () => {
          tx.oncomplete = () => {
            db.close();
            resolve();
          };

          tx.onerror = () => {
            db.close();
            reject(
              new Error('Failed to commit Sage probe IndexedDB transaction.'),
            );
          };
        };
      } catch (err) {
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    };
  });
}

export async function writeSageProbes(runId: string): Promise<void> {
  localStorage.setItem(PROBE_LOCAL_STORAGE_KEY, runId);
  await writeSageProbeIndexedDb(runId);
}
