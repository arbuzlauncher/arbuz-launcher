// Вкладка «Логи»: разбор свежего лога/краш-репорта, журнал запусков и отчёт.
import { useCallback, useEffect, useState } from 'react';
import { save } from '@tauri-apps/plugin-dialog';
import { api, type Instance, type LogAnalysis, type LogFinding } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

const fmtDate = (unix: number): string => {
  const d = new Date(unix * 1000);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
};

const SEV_LABEL: Record<string, string> = {
  error: 'Ошибка',
  warn: 'Предупреждение',
  info: 'Инфо',
};

export function LogsTab({ inst }: { inst: Instance }) {
  const t = useT();
  const [data, setData] = useState<LogAnalysis | null>(null);
  const [busy, setBusy] = useState(true);
  const [report, setReport] = useState('');
  const [rulesUrl, setRulesUrl] = useState('');
  const [rulesBusy, setRulesBusy] = useState(false);

  const toast = (m: string) => useStore.getState().toastMsg(m);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      setData(await api.analyzeLogs(inst.id));
    } catch (e) {
      toast(String(e));
    } finally {
      setBusy(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [inst.id]);

  useEffect(() => {
    void load();
  }, [load]);

  const applyFix = async (f: LogFinding, kind: string, value: string | null) => {
    try {
      await api.logApplyFix(inst.id, kind, value);
      toast(t('Исправление применено: {x}', { x: f.title }));
      await load();
    } catch (e) {
      toast(String(e));
    }
  };

  const makeReport = async () => {
    try {
      setReport(await api.logSupportReport(inst.id));
    } catch (e) {
      toast(String(e));
    }
  };

  const saveReport = async () => {
    try {
      const dest = await save({
        defaultPath: `arbuz-report-${inst.name.replace(/[^\w.-]+/g, '-')}.txt`,
        filters: [{ name: 'Text', extensions: ['txt'] }],
      });
      if (!dest) return;
      await api.logSaveReport(inst.id, dest);
      toast(t('Отчёт сохранён'));
    } catch (e) {
      toast(String(e));
    }
  };

  const copyReport = async () => {
    try {
      const text = report || (await api.logSupportReport(inst.id));
      setReport(text);
      await navigator.clipboard.writeText(text);
      toast(t('Отчёт скопирован'));
    } catch (e) {
      toast(String(e));
    }
  };

  const updateRules = async () => {
    if (!rulesUrl.trim()) return;
    setRulesBusy(true);
    try {
      const n = await api.logUpdateRules(rulesUrl.trim());
      toast(t('Правил загружено: {n}', { n }));
      setRulesUrl('');
      await load();
    } catch (e) {
      toast(String(e));
    } finally {
      setRulesBusy(false);
    }
  };

  if (busy && !data) {
    return <div className="hint">{t('Разбираем лог…')}</div>;
  }

  const findings = data?.findings ?? [];
  const journal = data?.journal ?? [];

  return (
    <div className="logs-tab" style={{ maxWidth: 880 }}>
      <div className="logs-head">
        <b>{t('Анализатор логов')}</b>
        <span className="hint">
          {data?.found
            ? t('Файл: {f}', { f: data.file ?? '' })
            : t('Свежий лог или краш-репорт не найден')}
        </span>
        <button className="btn sm" disabled={busy} onClick={() => void load()}>
          {busy ? t('Проверяем…') : t('Перепроверить')}
        </button>
      </div>

      {findings.length === 0 ? (
        <div className="logs-ok">✓ {t('Проблем в логе не найдено.')}</div>
      ) : (
        <div className="logs-findings">
          {findings.map((f) => (
            <div className={'log-row ' + (f.severity === 'error' ? 'err' : 'warn')} key={f.id}>
              <div className="log-main">
                <div className="log-title">
                  <span className={'log-tag ' + f.severity}>{t(SEV_LABEL[f.severity] ?? f.severity)}</span>
                  <b>{t(f.title)}</b>
                </div>
                <div className="hint">{t(f.detail)}</div>
              </div>
              <div className="log-actions">
                {f.fixes.map((fx) => (
                  <button
                    key={fx.kind + fx.label}
                    className="btn sm"
                    onClick={() => void applyFix(f, fx.kind, fx.value)}
                  >
                    {t(fx.label)}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="logs-head" style={{ marginTop: 18 }}>
        <b>{t('Журнал запусков')}</b>
        <span className="hint">{t('Последние {n}', { n: journal.length })}</span>
      </div>
      {journal.length === 0 ? (
        <div className="hint">{t('Пока нет записей: запустите игру.')}</div>
      ) : (
        <table className="logs-journal">
          <thead>
            <tr>
              <th>{t('Начало')}</th>
              <th>{t('Конец')}</th>
              <th>{t('Код выхода')}</th>
              <th>{t('Лог')}</th>
            </tr>
          </thead>
          <tbody>
            {journal.map((r, i) => (
              <tr key={`${r.started}-${i}`}>
                <td>{fmtDate(r.started)}</td>
                <td>{fmtDate(r.ended)}</td>
                <td className={r.exitCode === 0 ? 'ok' : 'bad'}>{r.exitCode ?? '—'}</td>
                <td className="hint" title={r.log ?? ''}>
                  {(r.log ?? '—').split(/[\\/]/).pop()}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <div className="logs-head" style={{ marginTop: 18 }}>
        <b>{t('Отчёт для поддержки')}</b>
        <span className="hint">{t('Секреты и пути обезличиваются.')}</span>
        <button className="btn sm" onClick={() => void makeReport()}>
          {t('Собрать')}
        </button>
        <button className="btn sm" onClick={() => void copyReport()}>
          {t('Копировать')}
        </button>
        <button className="btn sm" onClick={() => void saveReport()}>
          {t('Сохранить')}
        </button>
      </div>
      {report && <textarea className="logs-report" readOnly value={report} spellCheck={false} />}

      <div className="logs-head" style={{ marginTop: 18 }}>
        <b>{t('Обновление правил')}</b>
        <span className="hint">{t('HTTPS-ссылка на JSON с правилами.')}</span>
      </div>
      <div className="row g8">
        <input
          type="text"
          style={{ flex: 1 }}
          placeholder="https://example.com/log-rules.json"
          value={rulesUrl}
          onChange={(e) => setRulesUrl(e.target.value)}
        />
        <button className="btn sm" disabled={rulesBusy || !rulesUrl.trim()} onClick={() => void updateRules()}>
          {rulesBusy ? t('Загрузка…') : t('Обновить')}
        </button>
      </div>
    </div>
  );
}
