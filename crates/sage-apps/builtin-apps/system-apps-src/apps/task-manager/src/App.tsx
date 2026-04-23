import { useEffect, useMemo, useState } from 'react';
import {
  focusRuntime,
  hideRuntime,
  killRuntime,
  listRuntimes,
  type RuntimeRecord,
} from './taskManagerApi';

function formatDuration(ms: number) {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return [h, m, sec].map((v) => String(v).padStart(2, '0')).join(':');
}

export function App() {
  const [runtimes, setRuntimes] = useState<RuntimeRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [busyAppId, setBusyAppId] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    try {
      setRuntimes(await listRuntimes());
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  const sorted = useMemo(
    () => [...runtimes].sort((a, b) => a.appName.localeCompare(b.appName)),
    [runtimes],
  );

  return (
    <div
      style={{
        fontFamily:
          'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
        padding: 16,
        background: '#0b0b0c',
        color: '#f5f5f5',
        minHeight: '100vh',
      }}
    >
      <div style={{ maxWidth: 1100, margin: '0 auto' }}>
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: 16,
          }}
        >
          <div>
            <h1 style={{ margin: 0, fontSize: 24 }}>Task Manager</h1>
            <div style={{ opacity: 0.7, marginTop: 4, fontSize: 14 }}>
              Built-in system app
            </div>
          </div>

          <button
            onClick={() => void refresh()}
            style={{
              height: 36,
              padding: '0 14px',
              borderRadius: 10,
              border: '1px solid rgba(255,255,255,0.14)',
              background: 'rgba(255,255,255,0.06)',
              color: 'inherit',
              cursor: 'pointer',
            }}
          >
            Refresh
          </button>
        </div>

        <div
          style={{
            border: '1px solid rgba(255,255,255,0.12)',
            borderRadius: 18,
            overflow: 'hidden',
            background: 'rgba(255,255,255,0.03)',
          }}
        >
          {loading ? (
            <div style={{ padding: 18, opacity: 0.75 }}>Loading runtimes…</div>
          ) : sorted.length === 0 ? (
            <div style={{ padding: 18, opacity: 0.75 }}>No running apps.</div>
          ) : (
            sorted.map((runtime) => (
              <div
                key={runtime.runtimeId}
                style={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  gap: 16,
                  padding: 16,
                  borderTop: '1px solid rgba(255,255,255,0.08)',
                }}
              >
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontWeight: 600 }}>{runtime.appName}</div>
                  <div style={{ fontSize: 12, opacity: 0.7, marginTop: 4 }}>
                    {runtime.appId}
                  </div>
                  <div style={{ fontSize: 12, opacity: 0.7, marginTop: 4 }}>
                    kind={runtime.runtimeKind} · state={runtime.state} · mode=
                    {runtime.mode}
                  </div>
                  <div style={{ fontSize: 12, opacity: 0.7, marginTop: 4 }}>
                    uptime={formatDuration(Date.now() - runtime.startedAt)} ·
                    requests={runtime.inFlightRequestCount} · batches=
                    {runtime.activeBatchCount} · sockets=
                    {runtime.activeSocketCount}
                  </div>
                </div>

                <div style={{ display: 'flex', gap: 8, flexShrink: 0 }}>
                  <button
                    disabled={busyAppId === runtime.appId}
                    onClick={async () => {
                      setBusyAppId(runtime.appId);
                      try {
                        await focusRuntime(runtime.appId);
                      } finally {
                        setBusyAppId(null);
                      }
                    }}
                  >
                    Focus
                  </button>

                  <button
                    disabled={busyAppId === runtime.appId}
                    onClick={async () => {
                      setBusyAppId(runtime.appId);
                      try {
                        await hideRuntime(runtime.appId);
                      } finally {
                        setBusyAppId(null);
                      }
                    }}
                  >
                    Hide
                  </button>

                  <button
                    disabled={busyAppId === runtime.appId}
                    onClick={async () => {
                      setBusyAppId(runtime.appId);
                      try {
                        await killRuntime(runtime.appId);
                        await refresh();
                      } finally {
                        setBusyAppId(null);
                      }
                    }}
                  >
                    Kill
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
