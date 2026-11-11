// Окно обновления лаунчера: скачивает обновление само и перезапускает лаунчер.
import { useEffect, useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { api } from '../lib/api';
import { onProgress } from '../lib/events';
import { useT } from '../lib/i18n';
import { Markdown } from '../lib/markdown';
import { useStore } from '../store';

export function LauncherUpdateModal() {
  const upd = useStore((s) => s.appUpdate);
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [pct, setPct] = useState(0);

  useEffect(() => {
    let un: (() => void) | null = null;
    let dead = false;
    void onProgress((p) => {
      if (!dead && p.id === 'launcher-update') setPct(p.pct);
    }).then((f) => {
      if (dead) f();
      else un = f;
    });
    return () => {
      dead = true;
      un?.();
    };
  }, []);

  if (!upd) return null;

  const close = () => {
    if (busy) return;
    useStore.getState().set({ appUpdate: null });
  };
  const open = (url: string) => {
    if (!url) return;
    void openUrl(url).catch((e) => useStore.getState().toastMsg(String(e)));
  };

  const install = async () => {
    setBusy(true);
    setPct(0);
    try {
      const path = await api.launcherUpdateDownload();
      setPct(100);
      // На Windows лаунчер завершится и заменит себя; на Linux AppImage —
      // перезапустится. Если платформа не умеет — покажем ошибку.
      await api.launcherUpdateInstall(path);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
      setBusy(false);
    }
  };

  return (
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div className="mod">
        <div className="r">
          <div className="con-h">
            <b style={{ flex: 1, fontWeight: 600 }}>{t('Обновление лаунчера')}</b>
            <button className="btn sm" onClick={close} disabled={busy}>
              {t('Закрыть')}
            </button>
          </div>
          <p className="hint" style={{ marginBottom: 10 }}>
            {t('Доступна версия {v}, у вас {c}.', { v: upd.version, c: upd.current })}
          </p>
          {upd.notes && (
            <div className="pk-log">
              <Markdown text={upd.notes} />
            </div>
          )}
          {busy && (
            <div style={{ marginTop: 12 }}>
              <div className="hint">{t('Скачивание обновления… {p}%', { p: Math.round(pct) })}</div>
              <div className="upd-bar">
                <i style={{ width: `${Math.max(2, pct)}%` }} />
              </div>
            </div>
          )}
          <div className="row g8" style={{ marginTop: 14, justifyContent: 'flex-end' }}>
            {upd.page && (
              <button className="btn sm" onClick={() => open(upd.page)} disabled={busy}>
                {t('Что нового')}
              </button>
            )}
            <button className="btn sm" onClick={() => open(upd.download)} disabled={busy}>
              {t('Скачать вручную')}
            </button>
            <button className="btn pr" onClick={() => void install()} disabled={busy}>
              {busy ? t('Устанавливаем…') : t('Обновить и перезапустить')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
