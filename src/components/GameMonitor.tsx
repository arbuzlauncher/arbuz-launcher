// Живой мониторинг процесса игры (Б4): RAM и CPU по PID, график за сессию.
// FPS не показываем — из процесса игры его достать нельзя.
import { useEffect, useRef, useState } from 'react';
import { api, type MonitorSample } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

/** Точка графика: секунда от старта опроса, RAM МБ, CPU %. */
export interface MonitorPoint {
  t: number;
  ram: number;
  cpu: number;
}

const POLL_MS = 2000;
/** Максимум точек графика (около 10 минут при опросе раз в 2 с).
 * Дальше — прореживание: выкидываем каждую вторую точку. */
const MAX_POINTS = 300;

/** Опрос процесса игры. Возвращает последнюю точку и массив истории. */
export function useGameMonitor(id: string | null, live: boolean) {
  const [sample, setSample] = useState<MonitorSample | null>(null);
  const [points, setPoints] = useState<MonitorPoint[]>([]);
  const startedAt = useRef(0);

  useEffect(() => {
    if (!id || !live) {
      setSample(null);
      return;
    }
    startedAt.current = Date.now();
    setPoints([]);
    let alive = true;
    const tick = async () => {
      try {
        const s = await api.gameMonitor(id);
        if (!alive) return;
        setSample(s);
        if (s.found) {
          const t = Math.round((Date.now() - startedAt.current) / 1000);
          setPoints((prev) => {
            const next = [...prev, { t, ram: s.ramMb, cpu: s.cpuPct }];
            if (next.length <= MAX_POINTS) return next;
            // Прореживание вдвое: график остаётся, детализация падает.
            return next.filter((_, i) => i % 2 === 0 || i === next.length - 1);
          });
        }
      } catch {
        /* игра могла завершиться между опросами */
      }
    };
    void tick();
    const timer = window.setInterval(() => void tick(), POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [id, live]);

  return { sample, points };
}

/** Компактная строка «RAM · CPU» для нижней панели. */
export function MonitorStrip({ id }: { id: string }) {
  const live = useStore((s) => s.live[id] === true);
  const { sample } = useGameMonitor(live ? id : null, live);
  const t = useT();
  if (!live || !sample?.found) return null;
  return (
    <span className="hint" style={{ marginLeft: 10, whiteSpace: 'nowrap' }}>
      {t('RAM {r} · CPU {c}%', { r: gbShort(sample.ramMb), c: Math.round(sample.cpuPct) })}
    </span>
  );
}

function gbShort(mb: number): string {
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} ГБ` : `${mb} МБ`;
}

/** Панель с текущими значениями и SVG-графиком за сессию. */
export function MonitorPanel({ id }: { id: string }) {
  const live = useStore((s) => s.live[id] === true);
  const { sample, points } = useGameMonitor(live ? id : null, live);
  const t = useT();

  const w = 560;
  const h = 64;
  const path = (pick: (p: MonitorPoint) => number, max: number): string => {
    if (points.length < 2) return '';
    const maxT = Math.max(points[points.length - 1].t, 1);
    return points
      .map((p, i) => {
        const x = (p.t / maxT) * w;
        const y = h - Math.min(pick(p) / max, 1) * (h - 6) - 2;
        return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(' ');
  };
  const ramMax = Math.max(1024, ...points.map((p) => p.ram));
  const cpuMax = 100;

  return (
    <div className="row" style={{ alignItems: 'center', gap: 14, padding: '8px 14px', flexWrap: 'wrap' }}>
      <b style={{ fontWeight: 600, fontSize: 13 }}>
        {live && sample?.found
          ? `${gbShort(sample.ramMb)} · CPU ${Math.round(sample.cpuPct)}%`
          : t('Процесс завершён')}
      </b>
      <svg width={w} height={h} style={{ background: 'var(--s1, #212225)', borderRadius: 6, flex: 1, minWidth: 220 }} viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
        <path d={path((p) => p.ram, ramMax)} fill="none" stroke="var(--pri)" strokeWidth="2" />
        <path d={path((p) => p.cpu, cpuMax)} fill="none" stroke="var(--warn, #d9a63a)" strokeWidth="1.5" opacity="0.8" />
      </svg>
      <span className="hint" style={{ whiteSpace: 'nowrap' }}>
        {t('RAM до {r} · CPU до {c}% · точек: {n}', {
          r: gbShort(Math.round(ramMax)),
          c: Math.round(Math.max(0, ...points.map((p) => p.cpu))),
          n: points.length,
        })}
      </span>
    </div>
  );
}
