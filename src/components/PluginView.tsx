// Экран плагина из сайдбара: панель рендерится в изолированном iframe.
import { useCallback, useEffect, useState } from 'react';
import { api, type PluginPanelContent } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';
import { PluginFrame } from './PluginFrame';

export function PluginView() {
  const t = useT();
  const item = useStore((s) => s.pluginView);
  const [content, setContent] = useState<PluginPanelContent | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    if (!item) return;
    setBusy(true);
    setError('');
    try {
      const panel = item.panel || item.id;
      const c = await api.pluginPanelRender(item.plugin, panel);
      setContent(c);
    } catch (e) {
      setContent(null);
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [item]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!item) {
    return (
      <div className="screen" data-plugin-el="plugin.empty">
        <div className="hint">{t('Панель плагина не выбрана.')}</div>
      </div>
    );
  }

  const panel = item.panel || item.id;

  return (
    <div className="screen plugin-screen" data-plugin-el={`plugin.${item.id}`}>
      <div className="screen-head">
        <div>
          <div className="screen-title">{item.title}</div>
          <div className="screen-sub">{item.description || item.plugin}</div>
        </div>
        <div className="row g8">
          <button className="btn sm" disabled={busy} onClick={() => void load()}>
            {t('Обновить')}
          </button>
          <button className="btn sm" onClick={() => useStore.getState().set({ view: 'app' })}>
            {t('Настройки')}
          </button>
        </div>
      </div>

      {busy && <div className="hint">{t('Загрузка панели…')}</div>}
      {error && <div className="plugin-error">{error}</div>}
      {content?.error && <div className="plugin-error">{content.error}</div>}
      {!busy && !error && !content?.error && !content?.html && (
        <div className="hint">{t('Плагин не вернул содержимое панели.')}</div>
      )}
      {content?.html && (
        <PluginFrame
          plugin={item.plugin}
          panel={panel}
          content={content}
          title={item.title}
          className="plugin-frame plugin-frame-full"
          defaultHeight={content.height ?? 520}
        />
      )}
    </div>
  );
}
