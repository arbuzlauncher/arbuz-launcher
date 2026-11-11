// Экспериментальные: Python-плагины, их панели и источники каталога.
import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  api,
  type PluginInfo,
  type PluginPanel,
  type PluginPanelContent,
  type PluginPreview,
} from '../lib/api';
import { useT } from '../lib/i18n';
import { askConfirm, useStore } from '../store';
import { Toggle } from './Controls';
import { PluginFrame } from './PluginFrame';
import { PluginInstallModal } from './PluginInstallModal';

function perms(list: string[]): string {
  return list.length ? list.join(' · ') : '—';
}

export function PluginsPanel() {
  const t = useT();
  const enabled = useStore((s) => s.config.pluginsEnabled);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [panels, setPanels] = useState<PluginPanel[]>([]);
  const [sources, setSources] = useState<Record<string, unknown>[]>([]);
  const [busy, setBusy] = useState(false);
  const [selected, setSelected] = useState<PluginPanel | null>(null);
  const [content, setContent] = useState<PluginPanelContent | null>(null);
  const [panelBusy, setPanelBusy] = useState(false);
  const [preview, setPreview] = useState<PluginPreview | null>(null);
  const [previewPath, setPreviewPath] = useState('');
  const [previewBusy, setPreviewBusy] = useState(false);
  const logs = useStore((s) => s.pluginLogs);
  const [logLevel, setLogLevel] = useState('all');
  const filteredLogs = logs.filter((l) => logLevel === 'all' || l.level === logLevel);

  const load = useCallback(async () => {
    try {
      const [p, pn, src] = await Promise.all([
        api.pluginsList(),
        api.pluginPanels(),
        api.pluginSources(),
      ]);
      setPlugins(p);
      setPanels(pn);
      setSources(src);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, enabled]);

  useEffect(() => {
    void api
      .pluginLogs()
      .then((list) => useStore.getState().set({ pluginLogs: list }))
      .catch(() => {});
  }, []);

  const install = async () => {
    try {
      const p = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Arbuz Plugin', extensions: ['aplug', 'arbz'] }],
      });
      if (typeof p !== 'string') return;
      setBusy(true);
      const info = await api.pluginInspect(p);
      setPreview(info);
      setPreviewPath(p);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const confirmInstall = async () => {
    if (!previewPath) return;
    setPreviewBusy(true);
    try {
      const list = await api.pluginInstall(previewPath);
      setPreview(null);
      setPreviewPath('');
      await load();
      useStore.getState().toastMsg(t('Установлено плагинов: {n}', { n: list.length }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setPreviewBusy(false);
    }
  };

  const refreshLogs = async () => {
    try {
      const list = await api.pluginLogs();
      useStore.getState().set({ pluginLogs: list });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const clearLogs = async () => {
    try {
      await api.pluginLogsClear();
      useStore.getState().set({ pluginLogs: [] });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const copyLogs = () => {
    const text = filteredLogs
      .map((l) => `${new Date(l.ts).toLocaleString()} [${l.level}] ${l.plugin}: ${l.message}`)
      .join('\n');
    void navigator.clipboard
      .writeText(text)
      .then(() => useStore.getState().toastMsg(t('Логи скопированы')))
      .catch((e) => useStore.getState().toastMsg(String(e)));
  };

  const reload = async () => {
    setBusy(true);
    try {
      await api.pluginReload();
      await load();
      useStore.getState().toastMsg(t('Плагины перезагружены'));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const toggle = async (p: PluginInfo, on: boolean) => {
    try {
      await api.pluginSetEnabled(p.id, on);
      await load();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const remove = async (p: PluginInfo) => {
    const ok = await askConfirm({
      title: t('Удалить плагин?'),
      text: p.name,
      yes: t('Удалить'),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.pluginUninstall(p.id);
      await load();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const renderPanel = async (p: PluginPanel) => {
    setSelected(p);
    setContent(null);
    setPanelBusy(true);
    try {
      const c = await api.pluginPanelRender(p.plugin, p.id);
      setContent(c);
    } catch (e) {
      setContent({ error: String(e) });
    } finally {
      setPanelBusy(false);
    }
  };

  // Панель общается с плагином через PluginFrame (ArbuzSDK).

  return (
    <>
      <div className="sw">
        <span>{t('Система плагинов')}</span>
        <Toggle
          on={enabled}
          onClick={() =>
            useStore.getState().setConfig({
              ...useStore.getState().config,
              pluginsEnabled: !enabled,
            })
          }
        />
      </div>
      <div className="hint" style={{ paddingBottom: 10 }}>
        {t('Python-плагины .aplug и архивы .arbz: свои экраны, источники, темы и хуки. Плагины выполняются в процессе лаунчера — ставьте только доверенные.')}
      </div>

      {enabled && (
        <div className="plugin-box">
          <div className="row g8" style={{ flexWrap: 'wrap', marginBottom: 10 }}>
            <button className="btn sm" disabled={busy} onClick={() => void install()}>
              {t('Установить плагин')}
            </button>
            <button className="btn sm" disabled={busy} onClick={() => void reload()}>
              {t('Перезагрузить')}
            </button>
            <button className="btn sm" onClick={() => void api.pluginOpenFolder()}>
              {t('Папка плагинов')}
            </button>
            <button className="btn sm" onClick={() => void api.pluginOpenLibs()}>
              {t('Папка библиотек')}
            </button>
            <span className="hint" style={{ alignSelf: 'center' }}>
              {t('Плагинов: {n}', { n: plugins.length })}
            </span>
          </div>
          <div className="hint" style={{ marginBottom: 10 }}>
            {t('Свои библиотеки лежат в папке libs и импортируются в плагинах обычным import.')}
          </div>

          <div className="plugin-list">
            {plugins.map((p) => (
              <div className="plugin-row" key={p.id}>
                <div className="plugin-ico">{p.icon ? '◆' : '🐍'}</div>
                <div className="plugin-main">
                  <div className="plugin-title">
                    <b>{p.name}</b>
                    <span className="hint">v{p.version}</span>
                  </div>
                  <div className="hint">
                    {p.description || t('Без описания')}
                  </div>
                  <div className="hint plugin-meta">
                    {p.author || '—'} · API {p.api} · {perms(p.permissions)}
                  </div>
                  {p.error && <div className="plugin-error">{p.error}</div>}
                </div>
                <div className="plugin-actions">
                  <Toggle on={p.enabled} onClick={() => void toggle(p, !p.enabled)} />
                  <button className="btn sm" onClick={() => void remove(p)}>
                    {t('Удалить')}
                  </button>
                </div>
              </div>
            ))}
            {plugins.length === 0 && (
              <div className="hint" style={{ padding: 12 }}>
                {t('Плагинов пока нет. Установите .aplug или .arbz.')}
              </div>
            )}
          </div>

          {panels.length > 0 && (
            <>
              <div className="plugin-sub">{t('Экраны плагинов')}</div>
              <div className="plugin-panels">
                {panels.map((p) => (
                  <button
                    key={`${p.plugin}:${p.id}`}
                    className={'plugin-panel-card' + (selected?.id === p.id ? ' on' : '')}
                    onClick={() => void renderPanel(p)}
                  >
                    <b>{p.title || p.id}</b>
                    <span className="hint">{p.description || p.plugin}</span>
                  </button>
                ))}
              </div>
            </>
          )}

          {selected && (
            <div className="plugin-stage">
              <div className="row g8" style={{ alignItems: 'center', marginBottom: 8 }}>
                <b style={{ flex: 1 }}>{selected.title || selected.id}</b>
                <span className="hint">{selected.plugin}</span>
                <button className="btn sm" onClick={() => { setSelected(null); setContent(null); }}>
                  {t('Закрыть')}
                </button>
              </div>
              {panelBusy && <div className="hint">{t('Загрузка панели…')}</div>}
              {content?.error && <div className="plugin-error">{content.error}</div>}
              {content?.html && (
                <PluginFrame
                  plugin={selected.plugin}
                  panel={selected.id}
                  content={content}
                  title={selected.title || selected.id}
                  className="plugin-frame"
                  defaultHeight={selected.height ?? 520}
                />
              )}
            </div>
          )}

          {sources.length > 0 && (
            <>
              <div className="plugin-sub">{t('Источники каталога от плагинов')}</div>
              <div className="hint">
                {sources
                  .map((s) => String(s.name ?? s.id ?? 'source'))
                  .join(' · ')}
              </div>
            </>
          )}

          <div className="plugin-logs" data-plugin-el="plugin.logs">
            <div className="plugin-logs-head">
              <b>{t('Логи плагинов')}</b>
              <select value={logLevel} onChange={(e) => setLogLevel(e.target.value)} style={{ width: 150 }}>
                <option value="all">{t('Все')}</option>
                <option value="info">{t('Информация')}</option>
                <option value="warn">{t('Предупреждения')}</option>
                <option value="error">{t('Ошибки')}</option>
              </select>
              <button className="btn sm" onClick={() => void refreshLogs()}>
                {t('Обновить')}
              </button>
              <button className="btn sm" onClick={copyLogs}>
                {t('Скопировать')}
              </button>
              <button className="btn sm" onClick={() => void clearLogs()}>
                {t('Очистить')}
              </button>
            </div>
            <div className="plugin-logs-body">
              {filteredLogs.length === 0 ? (
                <div className="plugin-logs-empty">{t('Логов пока нет.')}</div>
              ) : (
                filteredLogs.map((l, i) => (
                  <div className="plugin-log" key={`${l.ts}-${i}`}>
                    <time>{new Date(l.ts).toLocaleTimeString()}</time>
                    <span className={'lv ' + l.level}>{l.level}</span>
                    <span className="pl" title={l.plugin}>
                      {l.plugin}
                    </span>
                    <span className="msg">{l.message}</span>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      )}

      {preview && (
        <PluginInstallModal
          preview={preview}
          busy={previewBusy}
          onCancel={() => {
            if (!previewBusy) setPreview(null);
          }}
          onInstall={() => void confirmInstall()}
        />
      )}
    </>
  );
}
