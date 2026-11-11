// Экспериментальные: движок тем (внешние theme.json + CSS и темы плагинов).
import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api, type ThemePack } from '../lib/api';
import { useT } from '../lib/i18n';
import { DEFAULT_CUSTOM, type CustomTheme } from '../lib/theme';
import { useStore } from '../store';
import { Toggle } from './Controls';

function customFromTheme(t: ThemePack): CustomTheme {
  const c: CustomTheme = { ...DEFAULT_CUSTOM };
  const roles = ['bg', 'sbg', 's1', 's2', 's3', 'on', 'onv', 'ln', 'pri', 'sec', 'ter'] as const;
  for (const r of roles) {
    const v = t.colors[r];
    if (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) c[r] = v;
  }
  c.font = t.font || '';
  c.radius = t.radius;
  c.soft = !!t.soft;
  return c;
}

export function ThemeEnginePanel() {
  const t = useT();
  const enabled = useStore((s) => s.config.themeEngine);
  const active = useStore((s) => s.config.themePack);
  const [themes, setThemes] = useState<ThemePack[]>([]);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setThemes(await api.themesList());
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, enabled]);

  const apply = async (id: string) => {
    try {
      setBusy(true);
      const { theme, css } = await api.themeLoad(id);
      const st = useStore.getState();
      st.setConfig({
        ...st.config,
        experimentalThemes: true,
        themeEngine: true,
        themePack: id,
        style: 'custom',
        dark: theme.dark,
        custom: customFromTheme(theme),
      });
      st.set({ themeCss: css, themeLayout: theme.layout });
      st.toastMsg(t('Тема применена: {name}', { name: theme.name }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const disable = () => {
    const st = useStore.getState();
    st.setConfig({
      ...st.config,
      themeEngine: false,
      themePack: '',
      style: 'custom',
    });
    st.set({ themeCss: '', themeLayout: '' });
  };

  const install = async () => {
    try {
      const p = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Arbuz Theme', extensions: ['arbtheme'] }],
      });
      if (typeof p !== 'string') return;
      setBusy(true);
      const id = await api.themeInstall(p);
      await load();
      useStore.getState().toastMsg(t('Тема установлена: {id}', { id }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string) => {
    try {
      await api.themeRemove(id);
      if (active === id) disable();
      await load();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  return (
    <>
      <div className="sw">
        <span>{t('Движок тем')}</span>
        <Toggle
          on={enabled}
          onClick={() => {
            if (enabled) disable();
            else {
              const st = useStore.getState();
              st.setConfig({ ...st.config, themeEngine: true });
            }
          }}
        />
      </div>
      <div className="hint" style={{ paddingBottom: 10 }}>
        {t('Темы из папки themes: палитра, шрифт, радиусы и свой CSS.')}
      </div>

      {enabled && (
        <div className="theme-engine">
          <div className="row g8" style={{ flexWrap: 'wrap', marginBottom: 10 }}>
            <button className="btn sm" disabled={busy} onClick={() => void install()}>
              {t('Установить тему')}
            </button>
            <button className="btn sm" onClick={() => void api.themeOpenFolder()}>
              {t('Папка тем')}
            </button>
            <button className="btn sm" disabled={busy} onClick={() => void load()}>
              {t('Обновить')}
            </button>
            {active && (
              <button className="btn sm" onClick={disable}>
                {t('Сбросить тему')}
              </button>
            )}
          </div>

          <div className="theme-engine-grid">
            {themes.map((theme) => (
              <div
                key={theme.id}
                className={'theme-engine-card' + (active === theme.id ? ' on' : '')}
                onClick={() => void apply(theme.id)}
              >
                <div
                  className="theme-engine-preview"
                  style={{
                    background: theme.colors.bg ?? '#1a1b1e',
                    color: theme.colors.on ?? '#fff',
                    borderRadius: Math.max(4, Math.min(18, theme.radius + 4)),
                  }}
                >
                  <i style={{ background: theme.colors.sbg ?? '#111' }} />
                  <div>
                    <b style={{ background: theme.colors.pri ?? '#5fb339' }} />
                    <span style={{ background: theme.colors.s1 ?? '#222' }} />
                    <span style={{ background: theme.colors.s2 ?? '#333' }} />
                  </div>
                </div>
                <div className="theme-engine-meta">
                  <div>
                    <b>{theme.name}</b>
                    <span className="hint">
                      {theme.plugin ? t('плагин') : t('пользовательская')}
                      {theme.layout && theme.layout !== 'default' ? ` · ${theme.layout}` : ''}
                    </span>
                  </div>
                  <div className="tcard-dots">
                    {[theme.colors.pri, theme.colors.ter, theme.colors.s3, theme.colors.sbg]
                      .filter(Boolean)
                      .map((c, i) => (
                        <i key={i} style={{ background: c }} />
                      ))}
                  </div>
                </div>
                {!theme.plugin && (
                  <button
                    className="btn sm theme-engine-del"
                    onClick={(e) => {
                      e.stopPropagation();
                      void remove(theme.id);
                    }}
                  >
                    {t('Удалить')}
                  </button>
                )}
              </div>
            ))}
          </div>
          {themes.length === 0 && <div className="hint">{t('Тем пока нет')}</div>}
        </div>
      )}
    </>
  );
}
