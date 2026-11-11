// Сборка серверного пака: ядро + моды и конфиги профиля в одном zip.
import { useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { api, type Instance } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

const KINDS: [string, string][] = [
  ['vanilla', 'Vanilla'],
  ['paper', 'Paper'],
  ['purpur', 'Purpur'],
  ['fabric', 'Fabric'],
  ['quilt', 'Quilt'],
  ['forge', 'Forge'],
  ['neoforge', 'NeoForge'],
];

export function ServerPackCard({ inst }: { inst: Instance }) {
  const t = useT();
  const [kind, setKind] = useState('vanilla');
  const [mc, setMc] = useState(inst.mcVersion);
  const [loader, setLoader] = useState(inst.loaderVersion ?? '');
  const [ram, setRam] = useState(String(inst.ramMaxMb || 4096));
  const [busy, setBusy] = useState(false);

  const toast = (m: string) => useStore.getState().toastMsg(m);
  const needsLoader = ['fabric', 'quilt', 'forge', 'neoforge'].includes(kind);

  const build = async () => {
    try {
      const dest = await save({
        defaultPath: `server-${inst.name.replace(/[^\w.-]+/g, '-')}.zip`,
        filters: [{ name: 'Zip', extensions: ['zip'] }],
      });
      if (!dest) return;
      setBusy(true);
      const res = await api.buildServerPack({
        instanceId: inst.id,
        kind,
        mcVersion: mc.trim(),
        loaderVersion: needsLoader && loader.trim() ? loader.trim() : null,
        dest,
        ramMb: Math.max(1024, parseInt(ram, 10) || 4096),
      });
      toast(
        t('Серверный пак собран: {n} файлов, {b}', {
          n: res.files,
          b: `${(res.bytes / 1024 / 1024).toFixed(1)} МБ`,
        }),
      );
    } catch (e) {
      toast(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="pk-card">
      <b>{t('Серверный пак')}</b>
      <div className="hint" style={{ marginTop: 4 }}>
        {t(
          'Скачает серверное ядро, положит моды и конфиги профиля, добавит скрипты запуска и eula.txt.',
        )}
      </div>
      <div className="two" style={{ marginTop: 12 }}>
        <div>
          <label className="l">{t('Ядро')}</label>
          <select value={kind} onChange={(e) => setKind(e.target.value)}>
            {KINDS.map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="l">{t('Версия Minecraft')}</label>
          <input type="text" value={mc} onChange={(e) => setMc(e.target.value)} />
        </div>
      </div>
      {needsLoader && (
        <div style={{ marginTop: 8 }}>
          <label className="l">{t('Версия загрузчика (пусто — последняя)')}</label>
          <input
            type="text"
            value={loader}
            placeholder={t('например, 0.15.11')}
            onChange={(e) => setLoader(e.target.value)}
          />
        </div>
      )}
      <div style={{ marginTop: 8 }}>
        <label className="l">{t('Память, МБ')}</label>
        <input type="text" value={ram} onChange={(e) => setRam(e.target.value)} />
      </div>
      <div className="row g8" style={{ marginTop: 12 }}>
        <button className="btn pr" disabled={busy || !mc.trim()} onClick={() => void build()}>
          {busy ? t('Собираю…') : t('Собрать серверный пак')}
        </button>
      </div>
    </div>
  );
}
