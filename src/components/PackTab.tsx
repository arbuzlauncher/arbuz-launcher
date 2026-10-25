// Вкладка «Сборка»: всё для модпака — описание, проверка, конфиги и экспорт.
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import {
  api,
  type ConfigDoc,
  type ConfigEntry,
  type Instance,
  type PackInfo,
  type PackIssue,
  type PackManifest,
} from '../lib/api';
import { fmtBytes } from '../lib/format';
import { exportPackAs } from '../lib/packExport';
import { useT } from '../lib/i18n';
import { LOADER_LABEL } from '../lib/labels';
import { useStore } from '../store';

const baseName = (p: string) => p.split('/').pop() ?? p;
const dirName = (p: string) => p.split('/').slice(0, -1).join('/');

const SEV_MARK: Record<string, string> = { error: '!', warn: '?', info: 'i' };

/**
 * Вкладка «Сборка» профиля.
 *
 * `initialMod` — имя мода, по которому нужно сразу отфильтровать конфиги
 * (кнопка «Конфиги» в списке контента).
 */
export function PackTab({ inst, initialMod }: { inst: Instance; initialMod: string | null }) {
  const t = useT();
  const dataRoot = useStore((s) => s.dataRoot);
  const toast = (m: string) => useStore.getState().toastMsg(m);

  const [info, setInfo] = useState<PackInfo | null>(null);
  const [form, setForm] = useState<PackManifest | null>(null);
  const [busy, setBusy] = useState(true);
  const [saving, setSaving] = useState(false);

  const [cfgs, setCfgs] = useState<ConfigEntry[] | null>(null);
  const [search, setSearch] = useState(initialMod ?? '');
  const [sel, setSel] = useState<string | null>(null);
  const [doc, setDoc] = useState<ConfigDoc | null>(null);
  const [text, setText] = useState('');
  const [docBusy, setDocBusy] = useState(false);

  const [issues, setIssues] = useState<PackIssue[] | null>(null);
  const [checking, setChecking] = useState(false);
  const [exporting, setExporting] = useState<string | null>(null);

  const taRef = useRef<HTMLTextAreaElement>(null);
  const gutRef = useRef<HTMLDivElement>(null);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const [i, c] = await Promise.all([api.packInfo(inst.id), api.packConfigs(inst.id)]);
      setInfo(i);
      setForm(i.manifest);
      setCfgs(c);
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

  // Пришли из «Контента» по кнопке «Конфиги» — фильтруем список.
  useEffect(() => {
    setSearch(initialMod ?? '');
    setSel(null);
    setDoc(null);
    setText('');
  }, [initialMod, inst.id]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    const list = cfgs ?? [];
    if (!q) return list;
    return list.filter(
      (c) =>
        c.path.toLowerCase().includes(q) ||
        (c.modName ?? '').toLowerCase().includes(q) ||
        (c.modId ?? '').toLowerCase().includes(q),
    );
  }, [cfgs, search]);

  const openCfg = useCallback(
    async (path: string) => {
      setSel(path);
      setDocBusy(true);
      try {
        const d = await api.readPackConfig(inst.id, path);
        setDoc(d);
        setText(d.content);
      } catch (e) {
        toast(String(e));
        setDoc(null);
      } finally {
        setDocBusy(false);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [inst.id],
  );

  const dirty = doc !== null && text !== doc.content;

  const jsonError = useMemo(() => {
    if (!doc || !doc.path.toLowerCase().endsWith('.json')) return null;
    try {
      JSON.parse(text);
      return null;
    } catch (e) {
      return String(e);
    }
  }, [doc, text]);

  const lines = useMemo(() => text.split('\n'), [text]);

  const saveCfg = async () => {
    if (!doc) return;
    if (jsonError) {
      toast(t('Исправьте ошибку JSON перед сохранением'));
      return;
    }
    setDocBusy(true);
    try {
      const d = await api.savePackConfig(inst.id, doc.path, text);
      setDoc(d);
      setText(d.content);
      toast(t('Сохранено: {f}', { f: d.path }));
    } catch (e) {
      toast(String(e));
    } finally {
      setDocBusy(false);
    }
  };

  const restoreCfg = async () => {
    if (!doc) return;
    setDocBusy(true);
    try {
      const d = await api.restorePackConfig(inst.id, doc.path);
      setDoc(d);
      setText(d.content);
      toast(t('Откатили из резервной копии'));
    } catch (e) {
      toast(String(e));
    } finally {
      setDocBusy(false);
    }
  };

  const formatJson = () => {
    if (!doc || !doc.path.toLowerCase().endsWith('.json')) return;
    try {
      setText(JSON.stringify(JSON.parse(text), null, 2));
    } catch {
      /* ошибку покажет jsonError */
    }
  };

  const saveInfo = async () => {
    if (!form) return;
    setSaving(true);
    try {
      const i = await api.savePackInfo(inst.id, form);
      setInfo(i);
      setForm(i.manifest);
      toast(t('Сборка сохранена'));
    } catch (e) {
      toast(String(e));
    } finally {
      setSaving(false);
    }
  };

  const check = async () => {
    setChecking(true);
    try {
      setIssues(await api.validatePack(inst.id));
    } catch (e) {
      toast(String(e));
    } finally {
      setChecking(false);
    }
  };

  const doExport = async (fmt: 'mrpack' | 'zip' | 'mmc') => {
    if (!form) return;
    setExporting(fmt);
    try {
      const r = await exportPackAs(inst, fmt, {
        name: form.name || inst.name,
        version: form.version,
        dir: dataRoot,
      });
      if (!r) return;
      toast(t('Экспортировано файлов: {n} ({size})', { n: r.files, size: fmtBytes(r.bytes) }));
    } catch (e) {
      toast(String(e));
    } finally {
      setExporting(null);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key !== 'Tab') return;
    e.preventDefault();
    const el = e.currentTarget;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    setText(text.slice(0, start) + '  ' + text.slice(end));
    requestAnimationFrame(() => {
      el.selectionStart = el.selectionEnd = start + 2;
    });
  };

  const syncScroll = () => {
    if (gutRef.current && taRef.current) {
      gutRef.current.scrollTop = taRef.current.scrollTop;
    }
  };

  if (busy && !info) {
    return <div className="hint">{t('Загружаю сборку…')}</div>;
  }
  if (!info || !form) {
    return <div className="hint">{t('Не удалось загрузить сборку')}</div>;
  }

  const set = (patch: Partial<PackManifest>) => setForm({ ...form, ...patch });

  return (
    <div className="pk">
      <div className="pk-head">
        <div className="pk-icon">
          {info.icon ? (
            <img src={convertFileSrc(info.icon)} alt="" />
          ) : (
            <span>{(form.name || info.name).trim().charAt(0).toUpperCase() || '?'}</span>
          )}
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div className="row g8" style={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div style={{ minWidth: 0 }}>
              <div className="pk-title">{form.name || info.name}</div>
              <div className="hint">
                {LOADER_LABEL[info.loader as keyof typeof LOADER_LABEL] ?? info.loader}{' '}
                {info.mcVersion}
                {info.loaderVersion ? ` · ${info.loaderVersion}` : ''}
              </div>
            </div>
            <button
              className="btn sm"
              onClick={() => void api.openPackConfigs(inst.id).catch((e) => toast(String(e)))}
            >
              {t('Открыть папку config')}
            </button>
          </div>
          <div className="pk-stats">
            <div className="pk-stat">
              <b>{info.mods}</b>
              {t('модов')}
            </div>
            <div className="pk-stat">
              <b>{info.configs}</b>
              {t('конфигов')}
            </div>
            <div className="pk-stat">
              <b>{fmtBytes(info.sizeBytes)}</b>
              {t('размер')}
            </div>
            <div className="pk-stat">
              <b>{Math.max(0, info.modsTotal - info.mods)}</b>
              {t('выключено')}
            </div>
          </div>
        </div>
      </div>

      <h2 style={{ marginTop: 22 }}>{t('Описание сборки')}</h2>
      <div className="two">
        <div>
          <label className="l">{t('Название')}</label>
          <input type="text" value={form.name} onChange={(e) => set({ name: e.target.value })} />
        </div>
        <div>
          <label className="l">{t('Версия')}</label>
          <input
            type="text"
            placeholder="1.0.0"
            value={form.version}
            onChange={(e) => set({ version: e.target.value })}
          />
        </div>
      </div>
      <div className="two">
        <div>
          <label className="l">{t('Автор')}</label>
          <input type="text" value={form.author} onChange={(e) => set({ author: e.target.value })} />
        </div>
        <div>
          <label className="l">{t('Коротко')}</label>
          <input
            type="text"
            placeholder={t('О чём эта сборка')}
            value={form.summary}
            onChange={(e) => set({ summary: e.target.value })}
          />
        </div>
      </div>
      <label className="l">{t('Описание')}</label>
      <textarea
        value={form.description}
        placeholder={t('Полное описание: моды, настройки, для кого сборка')}
        style={{ minHeight: 92 }}
        onChange={(e) => set({ description: e.target.value })}
      />
      <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 10 }}>
        <button className="btn" disabled={saving} onClick={() => setForm(info.manifest)}>
          {t('Отменить')}
        </button>
        <button className="btn pr" disabled={saving} onClick={() => void saveInfo()}>
          {saving ? t('Сохраняю…') : t('Сохранить сборку')}
        </button>
      </div>

      <div className="pk-card">
        <div className="row g8" style={{ justifyContent: 'space-between' }}>
          <div>
            <b>{t('Проверка сборки')}</b>
            <div className="hint">{t('Дубликаты, версии модов, память, зависимости и Java')}</div>
          </div>
          <button className="btn sm" disabled={checking} onClick={() => void check()}>
            {checking ? t('Проверяю…') : t('Проверить')}
          </button>
        </div>
        {issues && issues.length === 0 && (
          <div className="pk-ok">{t('Ошибок не найдено — сборка выглядит целой.')}</div>
        )}
        {issues && issues.length > 0 && (
          <div className="pk-issues">
            {issues.map((x, i) => (
              <div key={i} className={'pk-issue ' + x.severity}>
                <span className="sev">{SEV_MARK[x.severity] ?? 'i'}</span>
                <div style={{ minWidth: 0 }}>
                  <b>{x.title}</b>
                  <div className="hint">{x.detail}</div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="pk-card">
        <div className="row g8" style={{ justifyContent: 'space-between', marginBottom: 10 }}>
          <div>
            <b>{t('Конфиги модов')}</b>
            <div className="hint">
              {t('Файлы из config/ и defaultconfigs/. При сохранении создаётся .bak')}
            </div>
          </div>
          <a
            className="hint"
            style={{ cursor: 'pointer', color: 'var(--pri)' }}
            onClick={() => setSearch('')}
          >
            {t('Показать все')}
          </a>
        </div>

        <div className="pk-cfg">
          <div className="pk-cfg-list">
            <input
              type="text"
              placeholder={t('Поиск по моду или файлу…')}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              style={{ marginBottom: 6 }}
            />
            {cfgs === null && (
              <div className="hint" style={{ padding: 10 }}>
                {t('Сканирую конфиги…')}
              </div>
            )}
            {cfgs && filtered.length === 0 && (
              <div className="hint" style={{ padding: 10 }}>
                {initialMod
                  ? t('У этого мода нет файлов конфигурации.')
                  : t('Ничего не найдено.')}
              </div>
            )}
            {filtered.map((c) => (
              <button
                key={c.path}
                type="button"
                className={'pk-cfg-item' + (sel === c.path ? ' on' : '')}
                onClick={() => void openCfg(c.path)}
              >
                <b>{baseName(c.path)}</b>
                <span>
                  {c.modName ?? dirName(c.path)} · {fmtBytes(c.size)}
                </span>
              </button>
            ))}
          </div>

          <div className="pk-editor">
            {!doc ? (
              <div className="pk-empty">
                {docBusy ? t('Открываю…') : t('Выберите файл слева — он откроется здесь.')}
              </div>
            ) : (
              <>
                <div className="pk-editor-bar">
                  <span className="path" title={doc.path}>
                    {doc.path}
                  </span>
                  {doc.path.toLowerCase().endsWith('.json') && (
                    <button className="btn sm" onClick={formatJson}>
                      {t('Формат')}
                    </button>
                  )}
                  {doc.hasBackup && (
                    <button className="btn sm" disabled={docBusy} onClick={() => void restoreCfg()}>
                      {t('Откатить')}
                    </button>
                  )}
                  <button
                    className="btn sm pr"
                    disabled={docBusy || !dirty || !!jsonError}
                    onClick={() => void saveCfg()}
                  >
                    {dirty ? t('Сохранить') : t('Сохранено')}
                  </button>
                </div>
                {jsonError && <div className="pk-json-err">{jsonError}</div>}
                <div className="pk-code-wrap">
                  {lines.length <= 4000 && (
                    <div className="pk-lines" ref={gutRef}>
                      {lines.map((_, i) => (
                        <div key={i}>{i + 1}</div>
                      ))}
                    </div>
                  )}
                  <textarea
                    ref={taRef}
                    className="pk-code"
                    value={text}
                    spellCheck={false}
                    onChange={(e) => setText(e.target.value)}
                    onKeyDown={onKeyDown}
                    onScroll={syncScroll}
                  />
                </div>
                <div className="pk-editor-foot">
                  <span className="hint">
                    {lines.length} {t('строк')} · {fmtBytes(text.length)}
                    {dirty ? ` · ${t('не сохранено')}` : ''}
                  </span>
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      <div className="pk-card">
        <b>{t('Экспорт')}</b>
        <div className="hint" style={{ marginTop: 4 }}>
          {t(
            '.mrpack — формат Modrinth: моды скачиваются по ссылкам, конфиги внутри архива. .zip — формат CurseForge (manifest.json + overrides). MultiMC/Prism .zip — instance.cfg, mmc-pack.json и папка .minecraft, такой архив импортируется в Prism и MultiMC.',
          )}
        </div>
        <div className="row g8" style={{ marginTop: 12, flexWrap: 'wrap' }}>
          <button className="btn pr" disabled={!!exporting} onClick={() => void doExport('mrpack')}>
            {exporting === 'mrpack' ? t('Экспортирую…') : t('Экспорт Modrinth .mrpack')}
          </button>
          <button className="btn" disabled={!!exporting} onClick={() => void doExport('zip')}>
            {exporting === 'zip' ? t('Экспортирую…') : t('Экспорт CurseForge .zip')}
          </button>
          <button className="btn" disabled={!!exporting} onClick={() => void doExport('mmc')}>
            {exporting === 'mmc' ? t('Экспортирую…') : t('Экспорт MultiMC/Prism .zip')}
          </button>
        </div>
      </div>
    </div>
  );
}
