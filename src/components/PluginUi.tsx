// Универсальные UI-хуки плагинов: патчи элементов и дополнительные слоты.
import { useEffect, useRef, useState } from 'react';
import { api, type PluginElementPatch, type PluginSlotResult } from '../lib/api';
import { useStore } from '../store';

/** Применяет патч к DOM-элементу. Возвращает функцию отмены. */
function applyPatch(el: HTMLElement, patch: PluginElementPatch): () => void {
  const undo: Array<() => void> = [];
  if (typeof patch.hidden === 'boolean') {
    const prev = el.style.display;
    el.style.display = patch.hidden ? 'none' : prev || '';
    undo.push(() => {
      el.style.display = prev;
    });
  }
  if (typeof patch.text === 'string') {
    const prev = el.textContent ?? '';
    el.textContent = patch.text;
    undo.push(() => {
      el.textContent = prev;
    });
  }
  if (typeof patch.className === 'string') {
    const prev = el.className;
    el.className = patch.className;
    undo.push(() => {
      el.className = prev;
    });
  }
  if (patch.style && typeof patch.style === 'object') {
    const prev: Record<string, string> = {};
    for (const [k, v] of Object.entries(patch.style)) {
      if (typeof v !== 'string') continue;
      prev[k] = el.style.getPropertyValue(k);
      el.style.setProperty(k, v);
    }
    undo.push(() => {
      for (const [k, v] of Object.entries(prev)) {
        if (v) el.style.setProperty(k, v);
        else el.style.removeProperty(k);
      }
    });
  }
  if (typeof patch.html === 'string') {
    const prev = el.innerHTML;
    el.innerHTML = patch.html;
    undo.push(() => {
      el.innerHTML = prev;
    });
  }
  for (const [where, key] of [
    ['beforebegin', 'before'],
    ['afterend', 'after'],
  ] as const) {
    const html = patch[key];
    if (typeof html !== 'string' || !html) continue;
    const wrap = document.createElement('span');
    wrap.dataset.pluginInjected = '1';
    wrap.innerHTML = html;
    el.insertAdjacentElement(where, wrap);
    undo.push(() => wrap.remove());
  }
  return () => {
    for (const fn of undo.reverse()) fn();
  };
}

/**
 * Проходит по всем элементам с `data-plugin-el` и применяет патчи из хуков
 * `ui.element.<id>` / `ui.element`. Так плагины могут менять любой элемент,
 * которому разработчик дал стабильный id.
 */
export function PluginUiLayer() {
  const view = useStore((s) => s.view);
  const pluginView = useStore((s) => s.pluginView);
  const sidebar = useStore((s) => s.pluginSidebar);
  const panels = useStore((s) => s.pluginPanels);
  const tick = useStore((s) => s.pluginUiTick);

  useEffect(() => {
    let alive = true;
    const cleanups: Array<() => void> = [];
    const run = async () => {
      const nodes = Array.from(document.querySelectorAll<HTMLElement>('[data-plugin-el]'));
      for (const el of nodes) {
        const id = el.dataset.pluginEl;
        if (!id) continue;
        try {
          const patch = await api.pluginUiElement(id, {
            view,
            plugin: pluginView?.plugin ?? null,
            tag: el.tagName.toLowerCase(),
            text: (el.textContent ?? '').slice(0, 300),
            className: el.className,
          });
          if (!alive) return;
          if (patch && Object.keys(patch).length > 0) cleanups.push(applyPatch(el, patch));
        } catch {
          // Плагин мог выгрузиться между запросами — не мешаем интерфейсу.
        }
      }
    };
    void run();
    return () => {
      alive = false;
      for (const fn of cleanups.reverse()) fn();
    };
  }, [view, pluginView, sidebar, panels, tick]);

  return null;
}

/** Дополнительные пункты/HTML, которые плагины добавляют в конкретный слот. */
export function PluginSlot({
  slot,
  props,
  className,
}: {
  slot: string;
  props?: unknown;
  className?: string;
}) {
  const [result, setResult] = useState<PluginSlotResult | null>(null);
  const ref = useRef<HTMLDivElement | null>(null);
  const tick = useStore((s) => s.pluginUiTick);
  const propsKey = JSON.stringify(props ?? null);

  useEffect(() => {
    let alive = true;
    void api
      .pluginUiSlot(slot, props)
      .then((r) => {
        if (alive) setResult(r);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
    // propsKey — стабильный ключ, чтобы не перезапрашивать слот на каждый рендер.
    // tick — перезапрос после загрузки/перезагрузки плагинов.
  }, [slot, propsKey, tick]);

  useEffect(() => {
    if (!result || !ref.current || !result.patch) return;
    const patch = result.patch as PluginElementPatch;
    if (Object.keys(patch).length === 0) return;
    return applyPatch(ref.current, patch);
  }, [result]);

  if (!result) return null;
  const hasPatch = result.patch && Object.keys(result.patch).length > 0;
  if (!result.html && result.items.length === 0 && !hasPatch) return null;

  return (
    <div ref={ref} className={className} data-plugin-slot={slot}>
      {result.html ? <div dangerouslySetInnerHTML={{ __html: result.html }} /> : null}
      {result.items.map((it, i) => (
        <button
          key={it.id ?? `${slot}-${i}`}
          type="button"
          className="btn sm"
          title={it.label}
          onClick={() => {
            if (it.command) {
              const plugin = it.plugin ?? '';
              if (plugin) void api.pluginRunCommand(plugin, it.command, it.args ?? null).catch(() => {});
            } else if (it.panel) {
              const st = useStore.getState();
              const target = st.pluginSidebar.find(
                (p) => p.panel === it.panel || p.id === it.panel,
              );
              if (target) st.set({ view: 'plugin', pluginView: target, menu: false });
              else st.set({ view: 'app', menu: false });
            }
          }}
        >
          {it.label}
        </button>
      ))}
    </div>
  );
}
