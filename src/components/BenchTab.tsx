// Вкладка «Бенчмарк» (Б5): честные метрики сборки — время загрузки до
// меню/мира и потребление RAM/CPU за сессию. FPS не измеряем: из процесса
// игры его не достать, нужен мод (вариант предложен отдельно).
import { useCallback, useEffect, useState } from 'react';
import { api, type BenchRun, type Instance } from '../lib/api';
import { onBenchDone } from '../lib/events';
import { useT } from '../lib/i18n';
import { isRunning, playInstance } from '../lib/launch';
import { useStore } from '../store';

const fmtDate = (unix: number): string => {
  const d = new Date(unix * 1000);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
};

const fmtMs = (ms: number): string =>
  ms <= 0 ? '—' : ms < 60_000 ? `${(ms / 1000).toFixed(1)} с` : `${Math.floor(ms / 60_000)} мин ${Math.round((ms % 60_000) / 1000)} с`;

const fmtMb = (mb: number): string => (mb >= 1024 ? `${(mb / 1024).toFixed(1)} ГБ` : `${mb} МБ`);

export function BenchTab({ inst }: { inst: Instance }) {
  const s = useStore();
  const t = useT();
  const [runs, setRuns] = useState<BenchRun[] | null>(null);
  const [world, setWorld] = useState<string>('');
  const [busy, setBusy] = useState(false);
  const running = isRunning(s, inst.id);
  const worlds = s.sel === inst.id ? s.worlds : [];

  const load = useCallback(async () => {
    try {
      setRuns(await api.benchmarkList(inst.id));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  }, [inst.id]);

  useEffect(() => {
    void load();
  }, [load]);

  // bench-done приходит по завершении прогона — обновляем таблицу.
  useEffect(() => {
    let un: (() => void) | null = null;
    let dead = false;
    void onBenchDone(() => {
      if (!dead) void load();
    }).then((f) => {
      if (dead) f();
      else un = f;
    });
    return () => {
      dead = true;
      un?.();
    };
  }, [load]);

  const start = async () => {
    if (running) return;
    setBusy(true);
    try {
      await api.benchmarkStart(inst.id, world || null);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
      setBusy(false);
      return;
    }
    try {
      await playInstance(inst.id, world || null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ maxWidth: 760 }}>
      <div className="hint" style={{ marginBottom: 10 }}>
        {t('Бенчмарк измеряет то, что видно из лаунчера: время загрузки игры (до готовности меню или мира) и потребление RAM/CPU процесса за сессию.')}
      </div>
      <div className="hint" style={{ marginBottom: 14 }}>
        {t('FPS не измеряется — из процесса игры его не достать; для этого нужен вспомогательный мод внутри игры.')}
      </div>

      <label className="l">{t('Сценарий')}</label>
      <div className="row g8" style={{ flexWrap: 'wrap' }}>
        <select value={world} onChange={(e) => setWorld(e.target.value)} style={{ width: 220 }}>
          <option value="">{t('До главного меню')}</option>
          {worlds.map((w) => (
            <option key={w.dir} value={w.dir}>
              {t('Мир: {name}', { name: w.name })}
            </option>
          ))}
        </select>
        <button className="btn" disabled={busy || running} onClick={() => void start()}>
          {running ? t('Игра работает…') : t('Запустить бенчмарк')}
        </button>
      </div>
      <div className="hint" style={{ marginTop: 6 }}>
        {t('Прогон обычный запуск игры; итоги сохранятся автоматически, когда вы закроете игру.')}
      </div>

      <label className="l" style={{ marginTop: 18 }}>
        {t('Прошлые прогоны')}
      </label>
      {runs === null && <div className="hint">{t('Загрузка…')}</div>}
      {runs !== null && runs.length === 0 && (
        <div className="hint">{t('Прогонов ещё не было.')}</div>
      )}
      {runs !== null && runs.length > 0 && (
        <div className="panel" style={{ padding: '6px 14px' }}>
          {runs.map((r, i) => {
            const prev = runs[i + 1];
            const delta = prev && prev.loadMs > 0 && r.loadMs > 0 ? r.loadMs - prev.loadMs : null;
            return (
              <div
                key={r.date}
                className="row"
                style={{ justifyContent: 'space-between', alignItems: 'center', padding: '9px 0', borderTop: i === 0 ? 'none' : '1px solid var(--ln)' }}
              >
                <div>
                  <b style={{ fontWeight: 500 }}>{fmtDate(r.date)}</b>
                  <div className="hint">
                    {r.mcVersion} · {r.loader} · {t('модов: {n}', { n: r.mods })}
                    {r.world ? ` · ${t('мир {w}', { w: r.world })}` : ` · ${t('меню')}`}
                  </div>
                </div>
                <div className="hint" style={{ textAlign: 'right', whiteSpace: 'nowrap' }}>
                  <div>
                    {t('загрузка: {v}', { v: fmtMs(r.loadMs) })}
                    {delta !== null && (
                      <span style={{ color: delta > 0 ? 'var(--err, #d9503f)' : 'var(--pri)' }}>
                        {' '}
                        {delta > 0 ? '+' : ''}
                        {(delta / 1000).toFixed(1)} с
                      </span>
                    )}
                  </div>
                  <div>
                    {t('RAM {a} (пик {p}) · CPU {c}% (пик {cp}%)', {
                      a: fmtMb(r.ramAvgMb),
                      p: fmtMb(r.ramPeakMb),
                      c: Math.round(r.cpuAvg),
                      cp: Math.round(r.cpuPeak),
                    })}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
