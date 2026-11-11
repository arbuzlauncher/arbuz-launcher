import { getCurrentWindow } from '@tauri-apps/api/window';
import { useT } from '../lib/i18n';
import { flushConfig, useStore } from '../store';

export function Titlebar() {
  const win = getCurrentWindow();
  const view = useStore((s) => s.view);
  const pluginView = useStore((s) => s.pluginView);
  const canBack = useStore((s) => s.hp > 0);
  const canFwd = useStore((s) => s.hp < s.hist.length - 1);
  const t = useT();
  const title =
    view === 'home'
      ? t('Главная')
      : view === 'installs'
        ? t('Установки')
        : view === 'browse'
          ? t('Каталог')
          : view === 'accounts'
            ? t('Аккаунты')
            : view === 'plugin'
              ? pluginView?.title ?? t('Плагин')
              : view === 'app'
                ? t('Настройки')
                : t('Настройки');

  return (
    <div id="tb" data-plugin-el="titlebar" data-tauri-drag-region>
      <div className="tb-brand">
        <img src="/launcher.png" alt="" />
        <b>Arbuz Launcher</b>
      </div>
      <div className="tb-mid">
        <button
          className="tb-icobtn"
          data-plugin-el="titlebar.back"
          aria-label={t('Назад')}
          disabled={!canBack}
          onClick={() => useStore.getState().back()}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="16" height="16">
            <path d="M19 12H5M11 6l-6 6 6 6" />
          </svg>
        </button>
        <button
          className="tb-icobtn"
          data-plugin-el="titlebar.forward"
          aria-label={t('Вперёд')}
          disabled={!canFwd}
          onClick={() => useStore.getState().forward()}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" width="16" height="16">
            <path d="M5 12h14M13 6l6 6-6 6" />
          </svg>
        </button>
        <span className="tb-title" data-plugin-el="titlebar.title">{title}</span>
      </div>
      <div className="tb-win">
        <button aria-label="Minimize" onClick={() => void win.minimize()}>
          <svg viewBox="0 0 12 12">
            <path d="M2 6h8" />
          </svg>
        </button>
        <button aria-label="Maximize" onClick={() => void win.toggleMaximize()}>
          <svg viewBox="0 0 12 12">
            <rect x="2" y="2" width="8" height="8" />
          </svg>
        </button>
        <button
          aria-label="Close"
          onClick={() => {
            // Успеть записать настройки, изменённые меньше 300 мс назад.
            void flushConfig().then(() => win.close());
          }}
        >
          <svg viewBox="0 0 12 12">
            <path d="M2 2l8 8M10 2l-8 8" />
          </svg>
        </button>
      </div>
    </div>
  );
}
