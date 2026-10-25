// Экран «Серверы»: каталог серверов из статичного JSON (URL в настройках),
// живой Server List Ping, фильтры по версии и режиму, закрепление на профиле
// и добавление в servers.dat одной кнопкой.
import { useCallback, useEffect, useMemo, useState } from 'react';
import { api, type CatalogServer, type PinnedServer, type PingInfo } from '../lib/api';
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

export function ServersView() {
  const s = useStore();
  const t = useT();
  const [list, setList] = useState<CatalogServer[] | null>(null);
  const [pings, setPings] = useState<Record<string, PingInfo>>({});
  const [pinned, setPinned] = useState<PinnedServer[]>([]);
  const [filterVersion, setFilterVersion] = useState('');
  const [filterMode, setFilterMode] = useState('');
  const [refreshing, setRefreshing] = useState(false);

  const inst = s.instances.find((i) => i.id === s.sel) ?? s.instances[0] ?? null;

  const loadPinned = useCallback(async () => {
    if (!inst) return;
    try {
      setPinned(await api.pinnedServers(inst.id));
    } catch {
      /* профиль мог исчезнуть */
    }
  }, [inst]);

  // Каталог: кэш бэкенда; «Обновить» перечитывает по URL.
  const load = useCallback(
    async (refresh: boolean) => {
      setRefreshing(true);
      try {
        setList(await api.serversCatalog(refresh));
      } catch (e) {
        setList([]);
        useStore.getState().toastMsg(String(e));
      } finally {
        setRefreshing(false);
      }
    },
    [],
  );

  useEffect(() => {
    void load(false);
  }, [load]);

  useEffect(() => {
    void loadPinned();
  }, [loadPinned]);

  const shown = useMemo(() => {
    const v = filterVersion.trim().toLowerCase();
    const m = filterMode.trim().toLowerCase();
    return (list ?? []).filter((sv) => {
      if (v && !sv.version.toLowerCase().includes(v)) return false;
      if (m && !sv.mode.toLowerCase().includes(m)) return false;
      return true;
    });
  }, [list, filterVersion, filterMode]);

  const modes = useMemo(() => {
    const set = new Set((list ?? []).map((sv) => sv.mode).filter(Boolean));
    return [...set].sort();
  }, [list]);

  // Пинг видимых серверов волнами по 6.
  useEffect(() => {
    if (!list) return;
    let alive = true;
    const targets = shown.slice(0, 80).map((sv) => sv.ip);
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
  }, [list, shown]);

  const addToInstance = async (sv: CatalogServer) => {
    if (!inst) {
      useStore.getState().toastMsg(t('Сначала выберите профиль'));
      return;
    }
    try {
      await api.serversDatAdd(inst.id, sv.name, sv.ip);
      useStore.getState().toastMsg(t('«{name}» добавлен в список серверов профиля {inst}', { name: sv.name, inst: inst.name }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const togglePin = async (sv: CatalogServer) => {
    if (!inst) return;
    const isPinned = pinned.some((p) => p.ip === sv.ip);
    try {
      await api.pinServer(inst.id, sv.name, sv.ip, !isPinned);
      await loadPinned();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const card = (sv: CatalogServer) => {
    const p = pings[sv.ip];
    const isPinned = inst ? pinned.some((x) => x.ip === sv.ip) : false;
    return (
      <div key={sv.ip} className="panel" style={{ padding: 14, marginBottom: 10 }}>
        <div className="row" style={{ justifyContent: 'space-between', alignItems: 'center' }}>
          <div style={{ minWidth: 0 }}>
            <b style={{ fontWeight: 600 }}>{sv.name}</b>{' '}
            <span className="hint" style={{ fontFamily: 'ui-monospace,monospace' }}>
              {sv.ip}
            </span>
            <div className="hint">
              {sv.version}
              {sv.mode ? ` · ${sv.mode}` : ''}
              {p?.online
                ? ` · ${t('онлайн {a}/{b}', { a: p.playersOnline, b: p.playersMax })} · ${p.version || ''}`
                : p
                  ? ` · ${t('не отвечает')}`
                  : ` · ${t('проверяю…')}`}
            </div>
            {p?.online && p.motd && (
              <div style={{ fontSize: 12.5, marginTop: 4, opacity: 0.85 }}>{p.motd}</div>
            )}
          </div>
          <div className="row g8" style={{ flex: 'none' }}>
            {p?.online && (
              <span className="lab" style={{ color: pingColor(p.latencyMs), fontWeight: 600 }}>
                {p.latencyMs} ms
              </span>
            )}
            <button className="btn sm" onClick={() => void togglePin(sv)}>
              {isPinned ? t('Открепить') : t('Закрепить')}
            </button>
            <button className="btn sm" onClick={() => void addToInstance(sv)}>
              {t('В профиль')}
            </button>
          </div>
        </div>
      </div>
    );
  };

  const pinnedIps = new Set(pinned.map((p) => p.ip));
  const pinnedCards = (list ?? []).filter((sv) => pinnedIps.has(sv.ip));

  return (
    <div className="screen">
      <div className="hd">
        <div className="lb">{t('Серверы')}</div>
        <div className="ttl">
          {inst ? t('Закреплено на профиле «{name}»', { name: inst.name }) : t('Выберите профиль')}
        </div>
        <div className="tabs" />
      </div>
      <div className="pg">
        <div className="row g8" style={{ marginBottom: 14, flexWrap: 'wrap' }}>
          <input
            type="text"
            placeholder={t('Версия, напр. 1.21')}
            value={filterVersion}
            onChange={(e) => setFilterVersion(e.target.value)}
            style={{ width: 150 }}
          />
          <select value={filterMode} onChange={(e) => setFilterMode(e.target.value)} style={{ width: 150 }}>
            <option value="">{t('Все режимы')}</option>
            {modes.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
          <button className="btn sm" disabled={refreshing} onClick={() => void load(true)}>
            {refreshing ? t('Обновляю…') : t('Обновить список')}
          </button>
          {!s.config.serversUrl && (
            <span className="hint">
              {t('URL списка серверов не задан — укажите его в настройках лаунчера (формат в README)')}
            </span>
          )}
        </div>

        {pinned.length > 0 && (
          <>
            <h2>{t('Закреплённые')}</h2>
            {pinnedCards.map(card)}
            {pinnedCards.length === 0 && (
              <div className="hint" style={{ marginBottom: 10 }}>
                {t('Закреплённые серверы не входят в текущий каталог')}
              </div>
            )}
          </>
        )}

        <h2>
          {t('Каталог')} {list ? `(${shown.length})` : ''}
        </h2>
        {list === null && <div className="hint">{t('Загрузка…')}</div>}
        {list !== null && shown.length === 0 && (
          <div className="hint">
            {list.length === 0
              ? t('Список пуст. Задайте URL списка серверов в настройках и нажмите «Обновить список».')
              : t('Ничего не найдено — измените фильтры.')}
          </div>
        )}
        {shown.filter((sv) => !pinnedIps.has(sv.ip)).map(card)}
      </div>
    </div>
  );
}
