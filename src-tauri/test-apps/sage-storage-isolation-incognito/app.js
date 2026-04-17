(async () => {
  const params = new URLSearchParams(window.location.search);
  const runId = params.get('runId');

  if (!runId) {
    throw new Error('missing runId');
  }

  const LOCAL_STORAGE_KEY = 'sage_probe_local_storage';
  const COOKIE_KEY = 'sage_probe_cookie';
  const DB_NAME = 'sage_probe_db';
  const STORE_NAME = 'probe_store';
  const DB_KEY = 'sage_probe_key';

  async function readIndexedDbProbe() {
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
              resolve(typeof req.result === 'string' && req.result.length > 0);
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

  async function report(body) {
    const response = await fetch('sage-app://__sandbox/report/isolation', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      throw new Error(`report failed with status ${response.status}`);
    }
  }

  let localStorageVisible = false;
  let cookieVisible = false;
  let indexedDbVisible = false;
  let error = null;

  try {
    try {
      const value = localStorage.getItem(LOCAL_STORAGE_KEY);
      localStorageVisible = typeof value === 'string' && value.length > 0;
    } catch {
      localStorageVisible = false;
    }

    try {
      cookieVisible = document.cookie
        .split(';')
        .map((part) => part.trim())
        .some((part) => part.startsWith(`${COOKIE_KEY}=`));
    } catch {
      cookieVisible = false;
    }

    indexedDbVisible = await readIndexedDbProbe();
  } catch (err) {
    error = err instanceof Error ? err.message : String(err);
  }

  await report({
    runId,
    mode: 'incognito',
    persistentStorage: false,
    localStorageVisible,
    cookieVisible,
    indexedDbVisible,
    error,
  });
})().catch(async (err) => {
  try {
    const params = new URLSearchParams(window.location.search);
    await fetch('sage-app://__sandbox/report/isolation', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        runId: params.get('runId'),
        mode: 'incognito',
        persistentStorage: false,
        localStorageVisible: false,
        cookieVisible: false,
        indexedDbVisible: false,
        error: err instanceof Error ? err.message : String(err),
      }),
    });
  } catch {
    //
  }
});
