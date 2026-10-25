// Вкладка «Безопасность»: сканер модов профиля.
//
// Формулировки — «найдены подозрительные признаки», не «вирус»: решение
// всегда остаётся за пользователем. Карантин из параноидального режима
// можно вернуть в mods/ или удалить отсюда.
import { useCallback, useEffect, useState } from 'react';
import { openUrl } from '@tauri-apps/plugin-opener';
import { api, type AiNote, type Instance, type ModReport } from '../lib/api';
import { useT } from '../lib/i18n';
import { askConfirm, saveInstance, useStore } from '../store';
import { Seg, Toggle } from './Controls';

const LEVEL_LABEL: Record<string, string> = {
  ok: 'Признаков не найдено',
  suspicious: 'Подозрительные признаки',
  danger: 'Опасные признаки',
};

const MODE_LABEL: Record<string, string> = {
  normal: 'Обычный',
  strict: 'Усиленный',
  paranoid: 'Параноидальный',
};

const AI_ASSESSMENT: Record<string, string> = {
  benign: 'похоже на ложные срабатывания',
  suspicious: 'признаки требуют внимания',
  malicious: 'паттерн подозрительный',
};

export function ScannerTab({ inst }: { inst: Instance }) {
  const s = useStore();
  const t = useT();
  const [reports, setReports] = useState<ModReport[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [quarantine, setQuarantine] = useState<string[]>([]);
  const [openFile, setOpenFile] = useState<string | null>(null);
  const [aiNotes, setAiNotes] = useState<Record<string, AiNote | null>>({});

  const enabled = s.config.scannerEnabled;
  const mode = inst.scannerMode || s.config.scannerMode || 'normal';
  const upd = (patch: Partial<Instance>) => saveInstance({ ...inst, ...patch });

  const rescan = useCallback(async () => {
    setBusy(true);
    try {
      const [list, q] = await Promise.all([api.scanMods(inst.id), api.scannerQuarantine(inst.id)]);
      setReports(list);
      setQuarantine(q);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  }, [inst.id]);

  useEffect(() => {
    void rescan();
  }, [rescan, mode]);

  const danger = reports?.filter((r) => r.level === 'danger') ?? [];
  const suspicious = reports?.filter((r) => r.level === 'suspicious') ?? [];
  const clean = reports?.filter((r) => r.level === 'ok') ?? [];

  const allow = async (r: ModReport, value: boolean) => {
    const ok = value
      ? await askConfirm({
          title: t('Исключить файл из проверок?'),
          text: t('«{name}» больше не будет блокировать запуск — только если вы уверены в его происхождении.', { name: r.name }),
          yes: t('Исключить'),
          danger: true,
        })
      : true;
    if (!ok) return;
    try {
      await api.scannerAllowMod(inst.id, r.file, value);
      await rescan();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const restore = async (name: string) => {
    try {
      await api.scannerQuarantineRestore(inst.id, name);
      await rescan();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const drop = async (name: string) => {
    const ok = await askConfirm({
      title: t('Удалить файл из карантина?'),
      text: t('«{name}» будет удалён безвозвратно.', { name }),
      yes: t('Удалить'),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.scannerQuarantineDelete(inst.id, name);
      await rescan();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const askAi = async (r: ModReport) => {
    setAiNotes((n) => ({ ...n, [r.file]: null }));
    try {
      const lang = useStore.getState().config.lang;
      const note = await api.aiScanNote(inst.id, r.file, lang);
      setAiNotes((n) => ({ ...n, [r.file]: note }));
    } catch (e) {
      setAiNotes((n) => {
        const copy = { ...n };
        delete copy[r.file];
        return copy;
      });
      useStore.getState().toastMsg(String(e));
    }
  };

  return (
    <div style={{ maxWidth: 860 }}>
      <div className="sw">
        <div>
          <b style={{ fontWeight: 500 }}>{t('Сканер модов')}</b>
          <div className="hint">
            {enabled
              ? t('Включён в настройках лаунчера. Режим: {mode}.', {
                  mode: t(MODE_LABEL[mode] ?? mode),
                })
              : t('Выключен в настройках лаунчера — запуск не проверяется.')}
          </div>
        </div>
        <Toggle
          on={enabled}
          onClick={() => useStore.getState().setConfig({ ...s.config, scannerEnabled: !enabled })}
        />
      </div>

      <label className="l" style={{ marginTop: 14 }}>
        {t('Режим для этого профиля')}
      </label>
      <Seg
        items={[
          ['', t('Как в лаунчере')],
          ['normal', t('Обычный')],
          ['strict', t('Усиленный')],
          ['paranoid', t('Параноидальный')],
        ]}
        value={inst.scannerMode ?? ''}
        onPick={(v) => upd({ scannerMode: v || null })}
      />
      <div className="hint" style={{ marginTop: 6 }}>
        {mode === 'normal' &&
          t('Обычный: блокируется запуск только при опасных признаках; подозрительные — на ваше усмотрение.')}
        {mode === 'strict' &&
          t('Усиленный: блокировка при подозрительных признаках, проверяются и конфиги с паков.')}
        {mode === 'paranoid' &&
          t('Параноидальный: запуск только модов с известным хешем и без признаков; заблокированные уходят в карантин.')}
      </div>

      <div className="row" style={{ justifyContent: 'space-between', alignItems: 'center', marginTop: 18 }}>
        <label className="l" style={{ marginTop: 0 }}>
          {t('Результаты проверки')}
        </label>
        <button className="btn sm" disabled={busy} onClick={() => void rescan()}>
          {busy ? t('Сканирование…') : t('Сканировать')}
        </button>
      </div>

      {reports === null && <div className="hint">{busy ? t('Сканирование…') : ''}</div>}

      {reports !== null && (
        <div className="panel" style={{ padding: '6px 18px' }}>
          <div className="row g8" style={{ padding: '8px 0', flexWrap: 'wrap' }}>
            <span className="lab">{t('Всего: {n}', { n: reports.length })}</span>
            <span className="lab" style={{ color: 'var(--err, #d9503f)' }}>
              {t('опасных: {n}', { n: danger.length })}
            </span>
            <span className="lab" style={{ color: 'var(--warn, #d9a63a)' }}>
              {t('подозрительных: {n}', { n: suspicious.length })}
            </span>
            <span className="lab" style={{ color: 'var(--pri)' }}>
              {t('чистых: {n}', { n: clean.length })}
            </span>
          </div>

          {reports.length === 0 && (
            <div className="hint" style={{ paddingBottom: 12 }}>
              {t('В папках профиля нет архивов для проверки.')}
            </div>
          )}

          {reports.map((r) => (
            <div key={r.file} className="scan-item" style={{ borderTop: '1px solid var(--ln)' }}>
              <div
                className="row"
                style={{ justifyContent: 'space-between', alignItems: 'center', padding: '10px 0', cursor: 'pointer' }}
                onClick={() => setOpenFile(openFile === r.file ? null : r.file)}
              >
                <div style={{ minWidth: 0 }}>
                  <b style={{ fontWeight: 500 }}>{r.name}</b>
                  <span className="hint" style={{ marginLeft: 8, fontFamily: 'ui-monospace,monospace' }}>
                    {r.file}
                  </span>
                  <div className="hint">
                    {t(LEVEL_LABEL[r.level] ?? r.level)}
                    {r.offlineLookup ? t(' · каталоги были недоступны') : ''}
                    {r.knownHash ? t(' · хеш найден в каталоге') : t(' · хеш не найден в каталогах')}
                    {r.allowed ? t(' · исключён из проверок') : ''}
                  </div>
                </div>
                <div className="row g8" style={{ flex: 'none' }}>
                  <span
                    className="lab"
                    style={{
                      color:
                        r.level === 'danger'
                          ? 'var(--err, #d9503f)'
                          : r.level === 'suspicious'
                            ? 'var(--warn)'
                            : 'var(--pri)',
                      fontWeight: 600,
                    }}
                  >
                    {r.score}
                  </span>
                </div>
              </div>

              {openFile === r.file && (
                <div style={{ paddingBottom: 12 }}>
                  {r.findings.length === 0 ? (
                    <div className="hint">{t('Причин для оценки нет — признаков не найдено.')}</div>
                  ) : (
                    <div className="hint" style={{ marginBottom: 6 }}>
                      {t('Почему такая оценка:')}
                    </div>
                  )}
                  {r.findings.map((f, i) => (
                    <div
                      key={i}
                      className="row"
                      style={{ justifyContent: 'space-between', gap: 12, padding: '5px 0', borderTop: '1px dashed var(--ln)' }}
                    >
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: 13 }}>{t(f.what)}</div>
                        <div className="hint" style={{ fontFamily: 'ui-monospace,monospace' }}>
                          {f.rule} · {f.place}
                          {f.hard ? t(' · не гасится известностью мода') : ''}
                        </div>
                      </div>
                      <span className="lab" style={{ flex: 'none' }}>
                        +{f.weight}
                      </span>
                    </div>
                  ))}
                  <div className="row g8" style={{ marginTop: 10, flexWrap: 'wrap' }}>
                    {r.level !== 'ok' && s.config.aiScanner && (
                      <button
                        className="btn sm"
                        disabled={aiNotes[r.file] === null}
                        onClick={() => void askAi(r)}
                      >
                        {aiNotes[r.file] === null
                          ? t('ИИ думает…')
                          : aiNotes[r.file]
                            ? t('Обновить ИИ-заметку')
                            : t('ИИ-заметка')}
                      </button>
                    )}
                    {r.level === 'danger' && (
                      <button
                        className="btn sm"
                        onClick={() =>
                          void openUrl('https://modrinth.com/report').catch((e) =>
                            useStore.getState().toastMsg(String(e)),
                          )
                        }
                      >
                        {t('Пожаловаться на Modrinth')}
                      </button>
                    )}
                    {r.allowed ? (
                      <button className="btn sm" onClick={() => void allow(r, false)}>
                        {t('Снова проверять')}
                      </button>
                    ) : (
                      <button className="btn sm" onClick={() => void allow(r, true)}>
                        {t('Исключить из проверок')}
                      </button>
                    )}
                  </div>
                  {aiNotes[r.file] && (
                    (() => {
                      const n = aiNotes[r.file];
                      return (
                    <div className="panel" style={{ marginTop: 10, padding: 12 }}>
                      <div className="hint" style={{ marginBottom: 4 }}>
                        {t('ИИ-заметка (не приговор): {a}, уверенность {c}%', {
                          a: t(AI_ASSESSMENT[n!.assessment] ?? n!.assessment),
                          c: Math.round(n!.confidence * 100),
                        })}
                      </div>
                      <div style={{ fontSize: 13 }}>{n!.note}</div>
                      <div className="hint" style={{ marginTop: 4 }}>
                        {t('Модель: {m}. Оценка сканера при этом не меняется.', {
                          m: n!.model,
                        })}
                      </div>
                    </div>
                      );
                    })()
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {quarantine.length > 0 && (
        <>
          <label className="l" style={{ marginTop: 18 }}>
            {t('Карантин')}
          </label>
          <div className="panel" style={{ padding: '6px 18px' }}>
            <div className="hint" style={{ padding: '8px 0' }}>
              {t('Файлы, заблокированные параноидальным режимом. Они не удаляются и не запускаются с игрой.')}
            </div>
            {quarantine.map((name) => (
              <div
                key={name}
                className="row"
                style={{ justifyContent: 'space-between', alignItems: 'center', padding: '8px 0', borderTop: '1px solid var(--ln)' }}
              >
                <b style={{ fontWeight: 500, fontFamily: 'ui-monospace,monospace' }}>{name}</b>
                <div className="row g8">
                  <button className="btn sm" onClick={() => void restore(name)}>
                    {t('Вернуть в mods')}
                  </button>
                  <button className="btn sm" onClick={() => void drop(name)}>
                    {t('Удалить')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
