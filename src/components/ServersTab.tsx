// Вкладка «Серверы» в настройках профиля: только чтение servers.dat.
// Серверы добавляются в самой игре — лаунчер их не создаёт и не меняет.
import { useCallback, useEffect, useState } from 'react';
import { api, type Instance, type PingInfo, type ServerEntry } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

/** Параллельных пингов. */
const PING_CONCURRENCY = 6;

function pingColor(ms: number): string | undefined {
  if (ms <= 0) return undefined;
  if (ms < 80) return 'var(--pri)';
  if (ms < 200) return undefined;
  return 'var(--warn, #d9a63a)';
}

export function ServersTab({ inst }: { inst: Instance }) {
  const t = useT();
  const [list, setList] = useState<ServerEntry[] | null>(null);
  const [pings, setPings] = useState<Record<string, PingInfo>>({});
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const servers = await api.serversDatList(inst.id);
      setPings({});
      setList(servers);
    } catch (e) {
      setList([]);
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  }, [inst.id]);

  useEffect(() => {
    void load();
  }, [load]);

  // Пинг добавленных серверов волнами, чтобы не забить сеть.
  useEffect(() => {
    if (!list || list.length === 0) return;
    let alive = true;
    const targets = list.slice(0, 80).map((sv) => sv.ip);
    (async () => {
      for (let i = 0; i < targets.length; i += PING_CONCURRENCY) {
        if (!alive) return;
        const batch = targets.slice(i, i + PING_CONCURRENCY);
        const results = await Promise.all(batch.map((ip) => api.serverPing(ip).catch(() => null)));
        if (!alive) return;
        setPings((prev) => {
          const next = { ...prev };
          batch.forEach((ip, j) => {
            const r = results[j];
            if (r) next[ip] = r;
          });
          return next;
        });
      }
    })();
    return () => {
      alive = false;
    };
  }, [list]);

  return (
    <>
      <div className="row g8" style={{ marginBottom: 12, alignItems: 'center', flexWrap: 'wrap' }}>
        <div className="hint" style={{ flex: 1, minWidth: 220 }}>
          {t('Список берётся из servers.dat профиля. Добавьте серверы в игре — они появятся здесь.')}
        </div>
        <button className="btn sm" disabled={busy} onClick={() => void load()}>
          {busy ? t('Обновляю…') : t('Обновить')}
        </button>
      </div>

      {list === null && <div className="hint">{t('Загрузка…')}</div>}

      {list !== null && list.length === 0 && (
        <div className="panel" style={{ padding: 18 }}>
          <b style={{ fontWeight: 600 }}>{t('Серверов пока нет')}</b>
          <div className="hint" style={{ marginTop: 6 }}>
            {t('Добавьте серверы в игре — они появятся здесь автоматически.')}
          </div>
        </div>
      )}

      {(list ?? []).map((sv) => {
        const p = pings[sv.ip];
        return (
          <div key={sv.ip} className="panel" style={{ padding: 14, marginBottom: 10 }}>
            <div className="row" style={{ justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ minWidth: 0 }}>
                <b style={{ fontWeight: 600 }}>{sv.name || sv.ip}</b>{' '}
                <span className="hint" style={{ fontFamily: 'ui-monospace,monospace' }}>
                  {sv.ip}
                </span>
                <div className="hint">
                  {p?.online
                    ? `${t('онлайн {a}/{b}', { a: p.playersOnline, b: p.playersMax })}${p.version ? ` · ${p.version}` : ''}`
                    : p
                      ? t('не отвечает')
                      : t('проверяю…')}
                </div>
                {p?.online && p.motd && (
                  <div style={{ fontSize: 12.5, marginTop: 4, opacity: 0.85 }}>{p.motd}</div>
                )}
              </div>
              {p?.online && (
                <span className="lab" style={{ color: pingColor(p.latencyMs), fontWeight: 600, flex: 'none' }}>
                  {p.latencyMs} ms
                </span>
              )}
            </div>
          </div>
        );
      })}
    </>
  );
}
