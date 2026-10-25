// Нижняя панель: текущая установка (с быстрым выбором), большая кнопка PLAY и действия.
import { useState } from 'react';
import { api } from '../lib/api';
import { pi } from '../lib/art';
import { useT } from '../lib/i18n';
import { isRunning, playInstance } from '../lib/launch';
import { LOADER_LABEL } from '../lib/labels';
import { Icon } from '../lib/nav-icons';
import { MaterialIcon } from '../lib/material-icons';
import { monetLike } from '../lib/theme';
import { curAccount, curInstance, iconUrl, useArtTheme, useStore } from '../store';
import { Avatar } from './Controls';
import { MonitorStrip } from './GameMonitor';

export function BottomBar() {
  const s = useStore();
  const t = useT();
  const theme = useArtTheme();
  const [open, setOpen] = useState(false);
  const inst = curInstance(s);
  if (!inst) return null;

  const run = isRunning(s, inst.id);
  const acc = curAccount(s);
  const monet = monetLike(s.config.style);
  const tone =
    inst.icon || inst.iconPreset ? (
      pi(
        {
          biome: inst.biome,
          icon: iconUrl(s.dataRoot, inst.id, inst.icon, s.iconBust[inst.id]),
          preset: inst.iconPreset,
        },
        42,
        0,
        theme,
      )
    ) : (
      <Avatar name={acc?.name ?? '?'} skin={acc ? s.skins[acc.id] : null} size={42} />
    );

  return (
    <div className="bottom">
      <button type="button" className="bottom-cur" onClick={() => setOpen((v) => !v)}>
        <span className="inst-tone">{tone}</span>
        <span className="bottom-cur-info">
          <b>{inst.name}</b>
          <small>
            {inst.mcVersion} · {LOADER_LABEL[inst.loader]}
          </small>
        </span>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d={open ? 'M6 15l6-6 6 6' : 'M6 9l6 6 6-6'} />
        </svg>
      </button>

      {open && (
        <>
          <div className="menu-backdrop" onClick={() => setOpen(false)} />
          <div className="bottom-pick">
            <div className="bottom-pick-lb">{t('Версия для игры')}</div>
            {s.instances.map((x) => (
              <button
                key={x.id}
                type="button"
                className={'bottom-pick-item' + (x.id === inst.id ? ' on' : '')}
                onClick={() => {
                  s.set({ sel: x.id });
                  setOpen(false);
                }}
              >
                <b>{x.name}</b>
                <small>
                  {x.mcVersion} · {LOADER_LABEL[x.loader]}
                </small>
              </button>
            ))}
            <button
              type="button"
              className="bottom-pick-item more"
              onClick={() => {
                s.set({ view: 'installs', menu: false });
                setOpen(false);
              }}
            >
              <b>{t('Все установки')}</b>
            </button>
          </div>
        </>
      )}

      <div className="bottom-mid">
        {run && <MonitorStrip id={inst.id} />}
        <button
          type="button"
          className={'bottom-play' + (run ? ' run' : '')}
          onClick={() => void playInstance(inst.id, null)}
        >
          {run ? t('Стоп') : t('Играть').toUpperCase()}
        </button>
      </div>

      <div className="bottom-actions">
        <button
          type="button"
          className="btn"
          title={t('Открыть папку')}
          onClick={() => void api.openInstanceFolder(inst.id).catch((e) => useStore.getState().toastMsg(String(e)))}
        >
          {monet ? <MaterialIcon name="folder_open" size={22} /> : <Icon name="folder" size={18} />}
        </button>
        <button
          type="button"
          className="btn"
          title={t('Контент')}
          onClick={() => s.set({ view: 'p', sel: inst.id, tab: 'mods', menu: false })}
        >
          {monet ? <MaterialIcon name="extension" size={22} /> : <Icon name="modpacks" size={18} />}
        </button>
        <button
          type="button"
          className="btn"
          title={t('Настройки')}
          onClick={() => s.set({ view: 'p', sel: inst.id, tab: 'cfg', sec: 'g', menu: false })}
        >
          {monet ? <MaterialIcon name="settings" size={22} /> : <Icon name="settings" size={18} />}
        </button>
      </div>
    </div>
  );
}
