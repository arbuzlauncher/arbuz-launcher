import { useEffect, useState } from 'react';
import { api, type Loader, type PackPlan, type PlanItem } from '../lib/api';
import { useT } from '../lib/i18n';
import { LOADER_LABEL } from '../lib/labels';
import { applyInstance, useStore } from '../store';

const LOADERS: Loader[] = ['fabric', 'neoforge', 'forge', 'quilt'];

function fmtDl(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + 'M';
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + 'k';
  return String(n);
}

const key = (i: PlanItem): string => i.source + ':' + i.id;

export function AiPackModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useT();
  const versions = useStore((s) => s.versions);
  const enabled = useStore((s) => s.config.aiBuilder);
  const cfgLimit = useStore((s) => s.config.aiPackLimit);
  const cfgSource = useStore((s) => s.config.aiPackSource);
  const cfgModel = useStore((s) => s.config.aiPackModel);
  const cfgProvider = useStore((s) => s.config.aiProvider);

  const [phase, setPhase] = useState<'form' | 'plan' | 'build'>('form');
  const [prompt, setPrompt] = useState('');
  const [mc, setMc] = useState('');
  const [loader, setLoader] = useState<Loader>('fabric');
  const [limit, setLimit] = useState('');
  const [source, setSource] = useState('');
  const [model, setModel] = useState('');
  const [models, setModels] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [plan, setPlan] = useState<PackPlan | null>(null);
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [off, setOff] = useState<Record<string, boolean>>({});
  const [done, setDone] = useState(0);
  const [total, setTotal] = useState(0);
  const [cur, setCur] = useState('');
  const [fails, setFails] = useState<string[]>([]);

  useEffect(() => {
    if (open) return;
    setPhase('form');
    setPlan(null);
    setErr(null);
    setFails([]);
    setDone(0);
    setTotal(0);
    setCur('');
  }, [open]);

  // Открыли окно — берём лимит, источник и модель из настроек (их же можно поменять здесь).
  useEffect(() => {
    if (!open) return;
    setLimit(cfgLimit > 0 ? String(cfgLimit) : '');
    setSource(cfgSource);
    setModel(cfgModel);
    void api
      .aiModels()
      .then(setModels)
      .catch(() => {});
  }, [open, cfgLimit, cfgSource, cfgModel, cfgProvider]);

  if (!open || !enabled) return null;

  const releases = versions.filter((v) => v.kind === 'release').slice(0, 80);

  const picked = (list: PlanItem[]): PlanItem[] => list.filter((i) => !off[key(i)]);
  const all = plan ? [...plan.mods, ...plan.resourcepacks, ...plan.shaders] : [];
  const chosen = picked(all);

  const makePlan = async () => {
    if (prompt.trim().length < 4) {
      setErr(t('Опишите сборку хотя бы одним предложением'));
      return;
    }
    setBusy(true);
    setErr(null);
    try {
      const n = parseInt(limit, 10);
      const lim = Number.isFinite(n) && n > 0 ? Math.min(200, n) : null;
      const p = await api.aiPackPlan(
        prompt.trim(),
        useStore.getState().config.lang,
        mc || null,
        loader,
        lim,
        source || null,
        model || null,
      );
      setPlan(p);
      setName(p.name);
      setDesc(p.description);
      setOff({});
      setPhase('plan');
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const build = async () => {
    if (!plan) return;
    const items = chosen;
    setPhase('build');
    setErr(null);
    setFails([]);
    setDone(0);
    setTotal(items.length);
    setCur('');
    try {
      const profile = name.trim() || t('Новая сборка');
      const inst = await api.createInstance(profile, plan.mcVersion, plan.loader as Loader);
      useStore.getState().set({ instances: [...useStore.getState().instances, inst], sel: inst.id });

      let n = 0;
      for (const it of items) {
        setCur(it.title);
        try {
          await api.installCatalogItem(inst.id, it.source, it.id, it.kind, null);
        } catch (e) {
          setFails((f) => [...f, `${it.title} — ${String(e)}`]);
        }
        n += 1;
        setDone(n);
      }
      setCur('');

      const icon = plan.mods.find((m) => m.iconUrl)?.iconUrl ?? items.find((i) => i.iconUrl)?.iconUrl;
      if (icon) {
        try {
          applyInstance(await api.setInstanceIconUrl(inst.id, icon));
        } catch {}
      }
      try {
        await api.savePackInfo(inst.id, {
          name: profile,
          version: '1.0.0',
          author: '',
          summary: plan.summary,
          description: desc,
          icon: null,
        });
      } catch {}

      const fresh = await api.listInstances().catch(() => null);
      const st = useStore.getState();
      if (fresh) st.set({ instances: fresh });
      st.set({ view: 'p', sel: inst.id, sec: 'g' });
      st.toastMsg(t('Сборка готова: {name}', { name: profile }));
      onClose();
    } catch (e) {
      setErr(String(e));
      setPhase('plan');
    }
  };

  const row = (it: PlanItem) => {
    const excluded = !!off[key(it)];
    return (
      <label className={'ai-item' + (excluded ? ' off' : '')} key={key(it)}>
        <input
          type="checkbox"
          checked={!excluded}
          onChange={() => setOff((o) => ({ ...o, [key(it)]: !excluded }))}
        />
        {it.iconUrl ? (
          <img className="ai-ico" src={it.iconUrl} alt="" loading="lazy" />
        ) : (
          <span className="ai-ico" />
        )}
        <span className="ai-tx">
          <b>{it.title}</b>
          <small>
            {it.author}
            {it.why ? ' · ' + it.why : ''}
          </small>
        </span>
        <span className={'ai-src s-' + it.source} title={it.source}>
          {it.source === 'modrinth' ? 'Modrinth' : 'CurseForge'}
        </span>
        <span className="ai-dl">{fmtDl(it.downloads)}</span>
      </label>
    );
  };

  const pct = total ? Math.round((done / total) * 100) : 0;

  return (
    <div
      className="ov"
      onClick={(e) => {
        if (e.target === e.currentTarget && phase !== 'build') onClose();
      }}
    >
      <div className="mod ai-mod">
        <div className="r">
          {phase === 'form' && (
            <>
              <h2>{t('ИИ-сборка')}</h2>
              <p className="hint" style={{ marginBottom: 12 }}>
                {t('Опишите сборку словами — ассистент подберёт моды, ресурс-паки и шейдеры и поставит их в новый профиль.')}
              </p>

              <label className="l">{t('Что за сборка?')}</label>
              <textarea
                className="ai-prompt"
                rows={4}
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={t('Например: магическая сборка на 1.20.1 для слабого ноутбука, с квестами и без лишнего')}
              />

              <div className="ai-row">
                <div>
                  <label className="l">{t('Версия Minecraft')}</label>
                  <select value={mc} onChange={(e) => setMc(e.target.value)}>
                    <option value="">{t('Подберёт сам')}</option>
                    {releases.map((v) => (
                      <option key={v.id} value={v.id}>
                        {v.id}
                      </option>
                    ))}
                  </select>
                </div>
                <div>
                  <label className="l">{t('Загрузчик')}</label>
                  <select value={loader} onChange={(e) => setLoader(e.target.value as Loader)}>
                    {LOADERS.map((l) => (
                      <option key={l} value={l}>
                        {LOADER_LABEL[l]}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              <div className="ai-row">
                <div>
                  <label className="l">{t('Лимит модов')}</label>
                  <input
                    type="number"
                    min={1}
                    max={200}
                    value={limit}
                    onChange={(e) => setLimit(e.target.value)}
                    placeholder={t('Без ограничения')}
                  />
                </div>
                <div>
                  <label className="l">{t('Источник модов')}</label>
                  <select value={source} onChange={(e) => setSource(e.target.value)}>
                    <option value="">{t('Оба сразу')}</option>
                    <option value="modrinth">{t('Сначала Modrinth')}</option>
                    <option value="curseforge">{t('Сначала CurseForge')}</option>
                  </select>
                </div>
              </div>
              <div className="ai-row" style={{ gridTemplateColumns: '1fr' }}>
                <div>
                  <label className="l">{t('Модель ИИ')}</label>
                  <select
                    value={model}
                    onChange={(e) => {
                      const v = e.target.value;
                      setModel(v);
                      const cfg = useStore.getState().config;
                      useStore.getState().setConfig({ ...cfg, aiPackModel: v });
                    }}
                  >
                    <option value="">
                      {cfgProvider === 'opencode' ? t('Первая доступная') : t('По умолчанию')}
                    </option>
                    {models.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <div className="hint" style={{ marginBottom: 4 }}>
                {t('Пусто в лимите — модов будет сколько нужно сборке. «Оба сразу» ищет в двух каталогах, приоритетный — сначала в нём, а если там пусто, то в другом. Модель — кто именно собирает сборку.')}
              </div>

              {err && <div className="ai-err">{err}</div>}

              <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 18 }}>
                <button type="button" className="btn" onClick={onClose} disabled={busy}>
                  {t('Назад')}
                </button>
                <button type="button" className="btn pr" onClick={() => void makePlan()} disabled={busy}>
                  {busy ? t('Подбираю сборку…') : t('Составить сборку')}
                </button>
              </div>
            </>
          )}

          {phase === 'plan' && plan && (
            <>
              <h2>{t('План сборки')}</h2>

              <label className="l">{t('Название')}</label>
              <input type="text" value={name} onChange={(e) => setName(e.target.value)} />

              <div className="ai-meta">
                <span className="inst-tag">{LOADER_LABEL[plan.loader as Loader] ?? plan.loader}</span>
                <span className="inst-tag">{plan.mcVersion}</span>
                <span className="hint">{t('модель: {m}', { m: plan.model })}</span>
              </div>

              <label className="l">{t('Описание')}</label>
              <textarea rows={3} value={desc} onChange={(e) => setDesc(e.target.value)} />

              <div className="ai-list-wrap">
                <div className="ai-sec">{t('Моды: {n}', { n: plan.mods.length })}</div>
                <div className="ai-list">{plan.mods.map(row)}</div>

                {plan.resourcepacks.length > 0 && (
                  <>
                    <div className="ai-sec">{t('Ресурс-паки: {n}', { n: plan.resourcepacks.length })}</div>
                    <div className="ai-list">{plan.resourcepacks.map(row)}</div>
                  </>
                )}

                {plan.shaders.length > 0 && (
                  <>
                    <div className="ai-sec">{t('Шейдеры: {n}', { n: plan.shaders.length })}</div>
                    <div className="ai-list">{plan.shaders.map(row)}</div>
                  </>
                )}
              </div>

              {plan.skipped.length > 0 && (
                <div className="hint" style={{ marginTop: 8 }}>
                  {t('Не нашлись в каталоге: {list}', { list: plan.skipped.join(', ') })}
                </div>
              )}

              {err && <div className="ai-err">{err}</div>}

              <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 18 }}>
                <button type="button" className="btn" onClick={() => setPhase('form')}>
                  {t('Назад')}
                </button>
                <button
                  type="button"
                  className="btn pr"
                  disabled={chosen.length === 0}
                  onClick={() => void build()}
                >
                  {t('Создать профиль и поставить {n}', { n: chosen.length })}
                </button>
              </div>
            </>
          )}

          {phase === 'build' && (
            <>
              <h2>{t('Ставлю сборку…')}</h2>
              <div className="ai-bar">
                <span style={{ width: pct + '%' }} />
              </div>
              <div className="hint" style={{ marginTop: 8 }}>
                {done} / {total}
                {cur ? ' · ' + cur : ''}
              </div>
              {fails.length > 0 && (
                <div className="ai-err" style={{ marginTop: 12 }}>
                  {t('Не установились:')} {fails.join('; ')}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
