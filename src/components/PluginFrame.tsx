// iframe плагина: HTML/CSS/JS + встроенный ArbuzSDK и мост к Tauri.
import { useEffect, useMemo, useRef, useState } from 'react';
import { api, type PluginPanelContent } from '../lib/api';
import { PANEL_SDK } from '../lib/panel-sdk';
import { useStore } from '../store';

export function PluginFrame({
  plugin,
  panel,
  content,
  title,
  className,
  defaultHeight = 520,
}: {
  plugin: string;
  panel: string;
  content: PluginPanelContent | null;
  title?: string;
  className?: string;
  defaultHeight?: number;
}) {
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const config = useStore((s) => s.config);
  const [height, setHeight] = useState(() =>
    Math.max(240, content?.height ?? defaultHeight),
  );

  useEffect(() => {
    setHeight(Math.max(240, content?.height ?? defaultHeight));
  }, [content?.height, defaultHeight]);

  const theme = useMemo(
    () => ({
      dark: config.dark,
      accent: config.accent,
      style: 'custom',
      radius: config.custom?.radius ?? 8,
    }),
    [config.dark, config.accent, config.custom?.radius],
  );

  const srcDoc = useMemo(() => {
    if (!content?.html) return '';
    const js = (content.js ?? '').replace(/<\/script/gi, '<\\/script');
    return `<!doctype html><html><head><meta charset="utf-8"><meta name="color-scheme" content="dark light"><style>${content.css ?? ''}</style></head><body>${content.html}<script>window.__ARBUZ_PANEL__=${JSON.stringify({ plugin, panel })};</script><script>${PANEL_SDK}</script><script>${js}</script></body></html>`;
  }, [content, plugin, panel]);

  useEffect(() => {
    const onMsg = (e: MessageEvent) => {
      const frame = iframeRef.current?.contentWindow;
      if (!frame || e.source !== frame) return;
      const d = e.data as Record<string, unknown> | null;
      if (!d || typeof d !== 'object') return;
      // Старый протокол панелей: { type: 'arbuz-panel', command, args }.
      if (d.type === 'arbuz-panel' && typeof d.command === 'string') {
        void api.pluginRunCommand(plugin, d.command, d.args ?? null).catch(() => {});
        return;
      }
      if (d.type !== 'arbuz-sdk' || typeof d.id !== 'number') return;
      const id = d.id;
      const method = String(d.method ?? '');
      const args = (d.args ?? {}) as Record<string, unknown>;
      const reply = (result?: unknown, error?: string) => {
        try {
          frame.postMessage({ type: 'arbuz-sdk-result', id, result, error }, '*');
        } catch {
          // Панель уже закрылась.
        }
      };
      switch (method) {
        case 'ready':
          try {
            frame.postMessage({ type: 'arbuz-sdk-ready', theme }, '*');
          } catch {
            // Панель уже закрылась.
          }
          reply(true);
          break;
        case 'command':
          void api
            .pluginRunCommand(plugin, String(args.command ?? ''), args.args ?? null)
            .then((r) => reply(r))
            .catch((err) => reply(null, String(err)));
          break;
        case 'toast':
          useStore.getState().toastMsg(String(args.message ?? ''));
          reply(true);
          break;
        case 'resize': {
          const h = Math.max(200, Math.min(4000, Number(args.height) || 0));
          setHeight(h);
          reply(h);
          break;
        }
        case 'close':
          useStore.getState().set({ view: 'app' });
          reply(true);
          break;
        case 'open': {
          const st = useStore.getState();
          const target = st.pluginSidebar.find(
            (p) => p.panel === args.panel || p.id === args.panel,
          );
          if (target) st.set({ view: 'plugin', pluginView: target, menu: false });
          reply(!!target);
          break;
        }
        case 'storage.get':
          void api
            .pluginStorageGet(plugin, String(args.key ?? ''))
            .then((v) => reply(v ?? args.default ?? null))
            .catch((err) => reply(null, String(err)));
          break;
        case 'storage.set':
          void api
            .pluginStorageSet(plugin, String(args.key ?? ''), args.value)
            .then(() => reply(true))
            .catch((err) => reply(null, String(err)));
          break;
        case 'storage.all':
          void api
            .pluginStorageAll(plugin)
            .then((v) => reply(v))
            .catch((err) => reply(null, String(err)));
          break;
        case 'config.get':
          void api
            .pluginConfigGet(plugin, String(args.key ?? ''))
            .then((v) => reply(v ?? args.default ?? null))
            .catch((err) => reply(null, String(err)));
          break;
        case 'config.set':
          void api
            .pluginConfigSet(plugin, String(args.key ?? ''), args.value)
            .then(() => reply(true))
            .catch((err) => reply(null, String(err)));
          break;
        case 'emit':
          void api
            .pluginPanelEvent(plugin, String(args.event ?? ''), args.data)
            .then((v) => reply(v))
            .catch((err) => reply(null, String(err)));
          break;
        default:
          reply(null, `Unknown SDK method: ${method}`);
      }
    };
    window.addEventListener('message', onMsg);
    return () => window.removeEventListener('message', onMsg);
  }, [plugin, theme]);

  // События из Python (`arbuz.ui.emit`) -> панель (`arbuz.on`).
  useEffect(() => {
    const onEvent = (e: Event) => {
      const detail = (e as CustomEvent).detail as
        | { plugin?: string; event?: string; data?: unknown }
        | null;
      if (!detail || detail.plugin !== plugin || !detail.event) return;
      try {
        iframeRef.current?.contentWindow?.postMessage(
          { type: 'arbuz-sdk-event', event: detail.event, data: detail.data },
          '*',
        );
      } catch {
        // Панель уже закрылась.
      }
    };
    window.addEventListener('arbuz-panel-event', onEvent);
    return () => window.removeEventListener('arbuz-panel-event', onEvent);
  }, [plugin]);

  if (!srcDoc) return null;

  return (
    <iframe
      ref={iframeRef}
      className={className}
      title={title || panel}
      sandbox="allow-scripts"
      srcDoc={srcDoc}
      style={{ height }}
      onLoad={() => {
        // Страховка на случай, если сообщение `ready` пришло до подписки.
        try {
          iframeRef.current?.contentWindow?.postMessage(
            { type: 'arbuz-sdk-ready', theme },
            '*',
          );
        } catch {
          // Панель уже закрылась.
        }
      }}
    />
  );
}
