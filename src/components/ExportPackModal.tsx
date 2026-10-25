// Экспорт профиля в CurseForge, Modrinth или MultiMC/Prism.
import { useState } from 'react';
import type { Instance } from '../lib/api';
import { fmtBytes } from '../lib/format';
import { useT } from '../lib/i18n';
import { exportPackAs, type PackFormat } from '../lib/packExport';
import { useStore } from '../store';

export function ExportPackModal({ inst, onClose }: { inst: Instance; onClose: () => void }) {
  const t = useT();
  const [busy, setBusy] = useState<PackFormat | null>(null);

  const run = async (fmt: PackFormat) => {
    setBusy(fmt);
    try {
      const r = await exportPackAs(inst, fmt, { dir: useStore.getState().dataRoot });
      if (!r) return;
      useStore
        .getState()
        .toastMsg(t('Экспортировано файлов: {n} ({size})', { n: r.files, size: fmtBytes(r.bytes) }));
      onClose();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget && !busy) onClose();
      }}
    >
      <div className="mod">
        <div className="r">
          <h2>{t('Экспорт профиля')}</h2>
          <p className="hint" style={{ marginBottom: 14 }}>
            {t('«{name}» можно перенести в другой лаунчер: выберите формат.', { name: inst.name })}
          </p>

          <div className="row g8" style={{ flexWrap: 'wrap' }}>
            <button className="btn pr" disabled={!!busy} onClick={() => void run('zip')}>
              {busy === 'zip' ? t('Экспортирую…') : t('CurseForge (.zip)')}
            </button>
            <button className="btn" disabled={!!busy} onClick={() => void run('mrpack')}>
              {busy === 'mrpack' ? t('Экспортирую…') : t('Modrinth (.mrpack)')}
            </button>
            <button className="btn" disabled={!!busy} onClick={() => void run('mmc')}>
              {busy === 'mmc' ? t('Экспортирую…') : t('MultiMC/Prism (.zip)')}
            </button>
          </div>

          <div className="hint" style={{ marginTop: 12 }}>
            {t(
              'CurseForge и Prism понимают .zip с manifest.json, Modrinth — .mrpack, MultiMC/Prism — .zip с instance.cfg и mmc-pack.json.',
            )}
          </div>

          <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 18 }}>
            <button className="btn" onClick={onClose} disabled={!!busy}>
              {t('Закрыть')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
