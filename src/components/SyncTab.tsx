// Вкладка «Синхронизация»: общие настройки между профилями.
//
// Пользователь выбирает главный профиль и ключи options.txt (или пресеты
// «управление/звук/графика»), опционально servers.dat. При запуске любого
// другого профиля лаунчер подмешивает только выбранные ключи, которые
// целевая версия игры знает.
import { useEffect, useMemo, useState } from 'react';
import { api, type Instance, type SyncSettings } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';
import { Toggle } from './Controls';

/** Пресеты: какие ключи включают (совпадает с sync::PRESETS). */
const PRESETS: [string, string, string[]][] = [
  ['control', 'Управление', ['key_']],
  ['sound', 'Звук', ['soundCategory_']],
  [
    'graphics',
    'Графика',
    [
      'fov', 'gamma', 'renderDistance', 'graphicsMode', 'maxFps', 'particles',
      'mipmapLevels', 'biomeBlendRadius', 'entityShadows', 'smoothLighting',
      'entityDistanceScaling', 'guiScale',
    ],
  ],
];

const EMPTY: SyncSettings = { master: '', keys: [], serversDat: false };

export function SyncTab({ inst }: { inst: Instance }) {
  const s = useStore();
  const t = useT();
  const sync = s.config.sync ?? EMPTY;
  const [options, setOptions] = useState<Record<string, string> | null>(null);
  const [filter, setFilter] = useState('');

  const setSync = (patch: Partial<SyncSettings>) =>
    useStore.getState().setConfig({ ...s.config, sync: { ...sync, ...patch } });

  // Ключи главного профиля подгружаем при смене мастера.
  useEffect(() => {
    if (!sync.master) {
      setOptions(null);
      return;
    }
    let alive = true;
    void api
      .syncMasterOptions()
      .then((o) => {
        if (alive) setOptions(o);
      })
      .catch((e) => useStore.getState().toastMsg(String(e)));
    return () => {
      alive = false;
    };
  }, [sync.master]);

  const keys = useMemo(() => Object.keys(options ?? {}).sort(), [options]);
  const shown = useMemo(() => {
    const f = filter.trim().toLowerCase();
    if (!f) return keys;
    return keys.filter((k) => k.toLowerCase().includes(f));
  }, [keys, filter]);

  const applyPreset = (pats: string[]) => {
    const picked = new Set(sync.keys);
    for (const k of keys) {
      if (pats.some((p) => k === p || k.startsWith(p))) picked.add(k);
    }
    setSync({ keys: [...picked] });
  };

  const toggleKey = (k: string) => {
    const has = sync.keys.includes(k);
    setSync({ keys: has ? sync.keys.filter((x) => x !== k) : [...sync.keys, k] });
  };

  return (
    <div style={{ maxWidth: 720 }}>
      <label className="l">{t('Главный профиль')}</label>
      <select
        value={sync.master}
        onChange={(e) => setSync({ master: e.target.value, keys: [] })}
      >
        <option value="">{t('Не выбран')}</option>
        {s.instances.map((i) => (
          <option key={i.id} value={i.id}>
            {i.name === inst.name ? i.name : i.name}
          </option>
        ))}
      </select>
      <div className="hint" style={{ marginTop: 6 }}>
        {t('Настройки берутся из главного профиля и подмешиваются в остальные перед запуском. Этот профиль: {name}.', {
          name: sync.master === inst.id ? t('главный') : t('получатель'),
        })}
      </div>

      <div className="sw" style={{ marginTop: 14 }}>
        <div>
          <b style={{ fontWeight: 500 }}>{t('Список серверов (servers.dat)')}</b>
          <div className="hint">{t('Переносить список серверов главного профиля; старый файл остаётся как servers.dat.bak-sync')}</div>
        </div>
        <Toggle on={sync.serversDat} onClick={() => setSync({ serversDat: !sync.serversDat })} />
      </div>

      {sync.master && (
        <>
          <label className="l" style={{ marginTop: 18 }}>
            {t('Общие ключи options.txt')}
          </label>
          {!options ? (
            <div className="hint">{t('Загрузка…')}</div>
          ) : keys.length === 0 ? (
            <div className="hint">
              {t('В главном профиле нет options.txt — запустите его хотя бы раз.')}
            </div>
          ) : (
            <>
              <div className="row g8" style={{ flexWrap: 'wrap', marginBottom: 8 }}>
                {PRESETS.map(([, label, pats]) => (
                  <button key={label} className="btn sm" onClick={() => applyPreset(pats)}>
                    {t(label)}
                  </button>
                ))}
                <button className="btn sm" onClick={() => setSync({ keys: [] })}>
                  {t('Сбросить')}
                </button>
                <span className="lab">{t('выбрано: {n}', { n: sync.keys.length })}</span>
              </div>
              <input
                type="text"
                placeholder={t('Поиск ключа…')}
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                style={{ marginBottom: 8 }}
              />
              <div className="panel" style={{ padding: '6px 12px', maxHeight: 340, overflowY: 'auto' }}>
                {shown.map((k) => (
                  <div
                    key={k}
                    className="row"
                    style={{ justifyContent: 'space-between', alignItems: 'center', padding: '5px 0', cursor: 'pointer' }}
                    onClick={() => toggleKey(k)}
                  >
                    <div style={{ minWidth: 0 }}>
                      <div style={{ fontFamily: 'ui-monospace,monospace', fontSize: 12.5 }}>{k}</div>
                      <div className="hint" style={{ fontFamily: 'ui-monospace,monospace' }}>
                        {(options[k] ?? '').slice(0, 60) || '—'}
                      </div>
                    </div>
                    <div className={'ck' + (sync.keys.includes(k) ? ' on' : '')} style={{ padding: 0 }}>
                      <i style={{ borderRadius: 3 }} />
                    </div>
                  </div>
                ))}
              </div>
              <div className="hint" style={{ marginTop: 8 }}>
                {t('Подмешиваются только ключи, которые есть в options.txt запускаемого профиля: неизвестные версии игры ключи пропускаются.')}
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}
