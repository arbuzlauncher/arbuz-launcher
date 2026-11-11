// Анализатор конфликтов модов: дубликаты, загрузчик, зависимости, известные пары.
import { useCallback, useEffect, useState } from 'react';
import { api, type ConflictAction, type Instance, type ModConflict } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

const KIND_LABEL: Record<string, string> = {
  duplicate: 'Дубликат',
  loader: 'Загрузчик',
  missing_dep: 'Нет зависимости',
  disabled_dep: 'Зависимость выключена',
  known: 'Несовместимость',
};

export function ConflictsPanel({
  inst,
  onApplied,
}: {
  inst: Instance;
  /** Перечитать контент после отключения/включения файлов. */
  onApplied?: () => Promise<void> | void;
}) {
  const t = useT();
  const [list, setList] = useState<ModConflict[]>([]);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState('');

  const load = useCallback(async () => {
    try {
      setErr('');
      setList(await api.analyzeConflicts(inst.id));
    } catch (e) {
      setErr(String(e));
    }
  }, [inst.id]);

  useEffect(() => {
    void load();
  }, [load]);

  const apply = async (a: ConflictAction) => {
    if (a.kind === 'open_catalog') {
      useStore.getState().set({
        view: 'browse',
        tgt: inst.id,
        bt: 'mod',
        bq: a.query ?? a.modId ?? '',
        menu: false,
      });
      return;
    }
    setBusy(true);
    try {
      if (a.kind === 'remove') {
        for (const f of a.files) await api.removeContent(inst.id, f);
      } else {
        const enabled = a.kind === 'enable';
        for (const f of a.files) await api.toggleContent(inst.id, f, enabled);
      }
      await onApplied?.();
      await load();
      useStore.getState().toastMsg(t('Готово'));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (err) {
    return (
      <div className="panel conflict-panel">
        <div className="hint" style={{ padding: 14 }}>
          {t('Не удалось проверить конфликты: {e}', { e: err })}
        </div>
      </div>
    );
  }

  if (list.length === 0) {
    return (
      <div className="panel conflict-panel">
        <div className="conflict-ok">✓ {t('Конфликтов не найдено — сборка выглядит чистой.')}</div>
      </div>
    );
  }

  return (
    <div className="panel conflict-panel">
      <div className="conflict-head">
        <b>{t('Конфликты модов')}</b>
        <span className="hint">
          {t('Найдено проблем: {n}', { n: list.length })}
        </span>
        <button className="btn sm" disabled={busy} onClick={() => void load()}>
          {t('Перепроверить')}
        </button>
      </div>
      {list.map((c) => (
        <div className={'conflict-row ' + (c.severity === 'error' ? 'err' : 'warn')} key={c.id}>
          <div className="conflict-main">
            <div className="conflict-title">
              <span className={'conflict-tag ' + c.severity}>{KIND_LABEL[c.kind] ?? c.kind}</span>
              <b>{c.title}</b>
            </div>
            <div className="hint">{c.detail}</div>
          </div>
          <div className="conflict-actions">
            {c.actions.map((a) => (
              <button
                key={a.id}
                className={'btn sm' + (a.kind === 'disable' || a.kind === 'remove' ? '' : ' pr')}
                disabled={busy}
                onClick={() => void apply(a)}
              >
                {a.label}
              </button>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
