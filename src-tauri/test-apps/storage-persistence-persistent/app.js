(async () => {
  const params = new URLSearchParams(window.location.search);
  const runId = params.get('runId');
  const phase = params.get('phase');

  if (!runId) {
    throw new Error('missing runId');
  }

  if (phase !== 'write' && phase !== 'read') {
    throw new Error('missing or invalid phase');
  }

  const LOCAL_STORAGE_KEY = `sandbox_persistence_local_storage:${runId}:persistent`;
  const COOKIE_KEY = `sandbox_persistence_cookie_${runId}_persistent`;
  const DB_NAME = `sandbox_persistence_db_${runId}_persistent`;
  const STORE_NAME = 'probe_store';
  const DB_KEY = 'probe_key';

  async function writeIndexedDbValue() {
    try {
      return await new Promise((resolve) => {
        const open = indexedDB.open(DB_NAME);

        open.onerror = () => resolve(false);

        open.onupgradeneeded = () => {
          try {
            const db = open.result;
            if (!db.objectStoreNames.contains(STORE_NAME)) {
              db.createObjectStore(STORE_NAME);
            }
          } catch {
            //
          }
        };

        open.onsuccess = () => {
          try {
            const db = open.result;
            const tx = db.transaction(STORE_NAME, 'readwrite');
            const store = tx.objectStore(STORE_NAME);
            const req = store.put('present', DB_KEY);

            req.onerror = () => {
              db.close();
              resolve(false);
            };

            req.onsuccess = () => {
              tx.oncomplete = () => {
                db.close();
                resolve(true);
              };

              tx.onerror = () => {
                db.close();
                resolve(false);
              };
            };
          } catch {
            resolve(false);
          }
        };
      });
    } catch {
      return false;
    }
  }

  async function readIndexedDbValue() {
    try {
      return await new Promise((resolve) => {
        const open = indexedDB.open(DB_NAME);

        open.onerror = () => resolve(false);

        open.onupgradeneeded = () => {
          try {
            const db = open.result;
            if (!db.objectStoreNames.contains(STORE_NAME)) {
              db.createObjectStore(STORE_NAME);
            }
          } catch {
            //
          }
        };

        open.onsuccess = () => {
          try {
            const db = open.result;

            if (!db.objectStoreNames.contains(STORE_NAME)) {
              db.close();
              resolve(false);
              return;
            }

            const tx = db.transaction(STORE_NAME, 'readonly');
            const store = tx.objectStore(STORE_NAME);
            const req = store.get(DB_KEY);

            req.onerror = () => {
              db.close();
              resolve(false);
            };

            req.onsuccess = () => {
              db.close();
              resolve(req.result === 'present');
            };
          } catch {
            resolve(false);
          }
        };
      });
    } catch {
      return false;
    }
  }

  async function report(path, body) {
    const response = await fetch(path, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      throw new Error(`report failed with status ${response.status}`);
    }
  }

  if (phase === 'write') {
    let localStorageWrote = false;
    let cookieWrote = false;
    let indexedDbWrote = false;
    let error = null;

    try {
      try {
        localStorage.setItem(LOCAL_STORAGE_KEY, 'present');
        localStorageWrote =
          localStorage.getItem(LOCAL_STORAGE_KEY) === 'present';
      } catch {
        localStorageWrote = false;
      }

      try {
        document.cookie = `${COOKIE_KEY}=present; path=/`;
        cookieWrote = document.cookie
          .split(';')
          .map((part) => part.trim())
          .some((part) => part === `${COOKIE_KEY}=present`);
      } catch {
        cookieWrote = false;
      }

      indexedDbWrote = await writeIndexedDbValue();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }

    await report('sage-app://__sandbox/report/persistence-write', {
      runId,
      mode: 'persistent',
      persistentStorage: true,
      localStorageWrote,
      cookieWrote,
      indexedDbWrote,
      error,
    });

    return;
  }

  let localStoragePresent = false;
  let cookiePresent = false;
  let indexedDbPresent = false;
  let error = null;

  try {
    try {
      localStoragePresent =
        localStorage.getItem(LOCAL_STORAGE_KEY) === 'present';
    } catch {
      localStoragePresent = false;
    }

    try {
      cookiePresent = document.cookie
        .split(';')
        .map((part) => part.trim())
        .some((part) => part === `${COOKIE_KEY}=present`);
    } catch {
      cookiePresent = false;
    }

    indexedDbPresent = await readIndexedDbValue();
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  await report('sage-app://__sandbox/report/persistence-read', {
    runId,
    mode: 'persistent',
    persistentStorage: true,
    localStoragePresent,
    cookiePresent,
    indexedDbPresent,
    error,
  });
})().catch(async (err) => {
  try {
    const params = new URLSearchParams(window.location.search);
    const phase = params.get('phase');

    if (phase === 'write') {
      await fetch('sage-app://__sandbox/report/persistence-write', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          runId: params.get('runId'),
          mode: 'persistent',
          persistentStorage: true,
          localStorageWrote: false,
          cookieWrote: false,
          indexedDbWrote: false,
          error: err instanceof Error ? err.message : String(err),
        }),
      });
      return;
    }

    await fetch('sage-app://__sandbox/report/persistence-read', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        runId: params.get('runId'),
        mode: 'persistent',
        persistentStorage: true,
        localStoragePresent: false,
        cookiePresent: false,
        indexedDbPresent: false,
        error: err instanceof Error ? err.message : String(err),
      }),
    });
  } catch {
    //
  }
});
