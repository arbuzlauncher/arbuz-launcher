// Динамический визуал Minecraft: панорама из jar выбранной версии и текстуры
// кнопок. Данные приходят из плагина `arbuz.mc-visual` (или Rust-фолбэка).
import { useEffect } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { useStore } from '../store';

export function McVisualLayer() {
  const visual = useStore((s) => s.mcVisual);
  const enabled = useStore((s) => s.config.mcVisual);

  useEffect(() => {
    const win = document.getElementById('win');
    if (!win) return;
    const clear = () => {
      delete win.dataset.mcVisual;
      for (const k of ['--mc-button', '--mc-button-hover', '--mc-button-disabled']) {
        win.style.removeProperty(k);
      }
    };
    if (!enabled || !visual || visual.error) {
      clear();
      return;
    }
    win.dataset.mcVisual = 'on';
    const btn = visual.button ?? {};
    const url = (p?: string) => (p ? `url("${convertFileSrc(p)}")` : '');
    if (btn.normal) win.style.setProperty('--mc-button', url(btn.normal));
    if (btn.hover) win.style.setProperty('--mc-button-hover', url(btn.hover));
    if (btn.disabled) win.style.setProperty('--mc-button-disabled', url(btn.disabled));
    return clear;
  }, [enabled, visual]);

  if (!enabled || !visual || visual.error) return null;
  const faces = visual.panorama ?? [];
  if (!faces.length) return null;

  return (
    <>
      {visual.css ? <style id="mc-visual-plugin-css">{visual.css}</style> : null}
      <div className="mc-panorama" aria-hidden="true">
        {visual.faces === 6 && faces.length >= 6 ? (
          <div className="cube">
            {faces.slice(0, 6).map((p, i) => (
              <div
                key={p}
                className={`face f${i}`}
                style={{ backgroundImage: `url("${convertFileSrc(p)}")` }}
              />
            ))}
          </div>
        ) : (
          <div
            className="face"
            style={{
              inset: '-8%',
              backgroundImage: `url("${convertFileSrc(faces[0])}")`,
              transform: 'scale(1.08)',
              animation: 'mc-pan 80s ease-in-out infinite alternate',
            }}
          />
        )}
        <div className="shade" />
      </div>
    </>
  );
}
