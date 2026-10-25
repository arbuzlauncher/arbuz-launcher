// Экран «Каталог» (vBrowse из макета, ТЗ §10)
import { memo, useCallback, useEffect, useRef, useState, type CSSProperties, type MouseEvent } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { openUrl } from '@tauri-apps/plugin-opener';
import { api, type CatalogItem, type PackVersion, type ProjectDetails } from '../lib/api';
import { fmtShortDate } from '../lib/format';
import { t as tr, tp, useT } from '../lib/i18n';
import { loadProfileContent, useStore } from '../store';
import { LOADER_LABEL } from '../lib/labels';
import { Markdown, plainText } from '../lib/markdown';
import { Check, IUp } from './Controls';

/** Типы контента в каталоге (ТЗ §10): метка -> ключ. */
const KINDS: [string, string][] = [
  ['mod', 'Моды'],
  ['dp', 'Датапаки'],
  ['rp', 'Текстур-паки'],
  ['pk', 'Модпаки'],
  ['sh', 'Шейдеры'],
];

/** Метки загрузчиков как в макете (LD). Vanilla здесь не нужен: это «без загрузчика». */
const LOADERS = ['Fabric', 'Forge', 'NeoForge', 'Quilt'];

/** Категории макета (CATS). */
const CATS = ['Оптимизация', 'Технологии', 'Утилиты', 'Графика', 'Приключения'];

const SORTS: [string, string][] = [
  ['relevance', 'Релевантные'],
  ['downloads', 'По загрузкам'],
  ['newest', 'Новые'],
  ['updated', 'Обновлённые'],
];

const SOURCE_LABEL: Record<string, string> = { modrinth: 'Modrinth', curseforge: 'CurseForge', ftb: 'FTB' };

/** Сколько результатов показывать на одной странице. */
const PAGE = 20;

/** Число загрузок в коротком виде. */
function fmtCount(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, '') + tr(' млн');
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, '') + tr(' тыс.');
  return String(n);
}

/** Иконка каталога: картинка проекта, иначе первая буква названия. */
function CatIcon({ item, size }: { item: CatalogItem; size: number }) {
  const [bad, setBad] = useState(false);
  if (item.iconUrl && !bad) {
    return (
      <img
        className="mi"
        src={item.iconUrl}
        alt=""
        style={{ width: size, height: size, objectFit: 'cover' }}
        onError={() => setBad(true)}
      />
    );
  }
  const ch = (item.title || '?').trim().charAt(0).toUpperCase() || '?';
  return (
    <div className="mi" style={{ width: size, height: size, fontSize: Math.round(size * 0.42) }}>
      {ch}
    </div>
  );
}

/** Строка результата каталога (memo — ТЗ §12-c). */
const CatalogRow = memo(function CatalogRow({
  item,
  index,
  busy,
  onInstall,
  onOpen,
}: {
  item: CatalogItem;
  index: number;
  busy: boolean;
  onInstall: (item: CatalogItem) => void;
  onOpen: (item: CatalogItem) => void;
}) {
  const isPk = item.kind === 'pk';
  const done = item.installed && !isPk;
  const t = useT();
  return (
    <div className="lr" style={{ '--i': index } as CSSProperties}>
      <CatIcon item={item} size={56} />
      <div className="d" style={{ cursor: 'pointer' }} onClick={() => onOpen(item)} title={t('Открыть страницу')}>
        <b style={{ fontSize: 15, fontWeight: 600 }}>{item.title}</b>{' '}
        <span className="hint">{t('от {author}', { author: item.author })}</span>
        <div style={{ margin: '3px 0 8px' }}>{plainText(item.description)}</div>
        <div className="row g8" style={{ flexWrap: 'wrap' }}>
          {item.loaders.map((l) => (
            <span key={l} className="lab">
              {l}
            </span>
          ))}
          {item.categories.slice(0, 3).map((l) => (
            <span key={l} className="hint">
              {l}
            </span>
          ))}
        </div>
      </div>
      <div style={{ textAlign: 'right', flex: 'none' }}>
        <div className="hint" style={{ marginBottom: 8 }}>
          {tp('download', item.downloads, { n: fmtCount(item.downloads) })} · {SOURCE_LABEL[item.source] ?? item.source}
        </div>
        <button
          className={'btn sm' + (done ? '' : ' pr')}
          disabled={busy || done}
          onClick={() => void onInstall(item)}
        >
          {isPk ? t('Создать профиль') : done ? t('Установлено') : t('Установить')}
        </button>
      </div>
    </div>
  );
});

export function Catalog() {
  const src = useStore((s) => s.src);
  const t = useT();
  const bt = useStore((s) => s.bt);
  const bq = useStore((s) => s.bq);
  const bl = useStore((s) => s.bl);
  const bc = useStore((s) => s.bc);
  const bv = useStore((s) => s.bv);
  const bs = useStore((s) => s.bs);
  const tgt = useStore((s) => s.tgt);
  const cat = useStore((s) => s.cat);
  const catTotal = useStore((s) => s.catTotal);
  const catBusy = useStore((s) => s.catBusy);
  const instances = useStore((s) => s.instances);
  const versions = useStore((s) => s.versions);
  const packPct = useStore((s) => s.packPct);
  const packName = useStore((s) => s.packName);

  const [busy, setBusy] = useState(false);
  /** Текущая страница результатов (0-based). */
  const [page, setPage] = useState(0);
  /** Элемент, для которого выбирается мир при установке датапака. */
  const [pick, setPick] = useState<CatalogItem | null>(null);
  const [pickWorlds, setPickWorlds] = useState<{ dir: string; name: string }[]>([]);
  /** Сборка, для которой выбирается версия. */
  const [pack, setPack] = useState<CatalogItem | null>(null);
  const [packVersions, setPackVersions] = useState<PackVersion[]>([]);
  const [packPick, setPackPick] = useState('');
  const [packLoading, setPackLoading] = useState(false);

  const pgRef = useRef<HTMLDivElement | null>(null);
  /** Номер последнего запроса: ответы устаревших запросов игнорируются (ТЗ §12-b). */
  const reqRef = useRef(0);

  // Цель установки по умолчанию — первый профиль.
  useEffect(() => {
    if (tgt === null && instances.length > 0) {
      useStore.getState().set({ tgt: instances[0].id });
    }
  }, [tgt, instances]);

  // FTB отдаёт только сборки: не оставляем активной вкладку модов.
  useEffect(() => {
    if (src === 'FTB' && bt !== 'pk') useStore.getState().set({ bt: 'pk' });
  }, [src, bt]);

  /** Один запрос каталога. `offset` — смещение выбранной страницы. */
  const run = useCallback(async (offset: number) => {
    const st = useStore.getState();
    const my = ++reqRef.current;
    st.set({ catBusy: true });
    try {
      const base = {
        kind: st.bt,
        query: st.bq,
        loaders: st.bl.filter((l) => l.toLowerCase() !== 'vanilla'),
        mcVersion: st.bv || null,
        categories: st.bc,
        offset,
        sort: st.bs,
        instanceId: st.tgt,
      };
      const results = [await api.searchCatalog({ ...base, source: st.src.toLowerCase() })];
      if (my !== reqRef.current) return;
      const seen = new Set<string>();
      const hits = results
        .flatMap((r) => r.hits)
        .filter((h) => {
          const key = h.source + ':' + h.id;
          if (seen.has(key)) return false;
          seen.add(key);
          return true;
        });
      const total = results.reduce((a, r) => a + r.total, 0);
      const cur = useStore.getState();
      cur.set({
        cat: hits,
        catTotal: total,
        catBusy: false,
      });
    } catch (e) {
      if (my !== reqRef.current) return;
      useStore.getState().set({ catBusy: false });
      useStore.getState().toastMsg(String(e));
    }
  }, []);

  // Поиск с задержкой 250 мс; смена фильтра возвращает на первую страницу.
  const filterKey = JSON.stringify([src, bt, bq, bl, bc, bv, bs, tgt]);
  const lastFilter = useRef(filterKey);
  useEffect(() => {
    if (lastFilter.current !== filterKey) {
      lastFilter.current = filterKey;
      if (page !== 0) {
        setPage(0);
        return;
      }
    }
    const t = window.setTimeout(() => void run(page * PAGE), 250);
    return () => window.clearTimeout(t);
  }, [filterKey, page, run]);

  // При смене страницы — наверх списка.
  useEffect(() => {
    pgRef.current?.scrollTo({ top: 0, behavior: 'smooth' });
  }, [page]);

  const toggleLoader = (v: string) => {
    const cur = useStore.getState().bl;
    useStore.getState().set({ bl: cur.includes(v) ? cur.filter((x) => x !== v) : [...cur, v] });
  };

  const toggleCat = (v: string) => {
    const cur = useStore.getState().bc;
    useStore.getState().set({ bc: cur.includes(v) ? cur.filter((x) => x !== v) : [...cur, v] });
  };

  /** Установка обычного контента в выбранный профиль. */
  const installInto = useCallback(async (item: CatalogItem, world: string | null, into?: string) => {
    const st = useStore.getState();
    const id = into ?? st.tgt;
    if (!id) {
      st.toastMsg(t('Сначала создайте профиль'));
      return;
    }
    setBusy(true);
    try {
      await api.installCatalogItem(id, item.source, item.id, item.kind, world);
      await loadProfileContent(id);
      await run(0);
      const name = useStore.getState().instances.find((i) => i.id === id)?.name ?? '';
      useStore.getState().toastMsg(t('Установлено в «{name}»', { name }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  }, [run, t]);

  /** Создание профиля из выбранной версии сборки. */
  const installPack = useCallback(async (item: CatalogItem, versionId: string) => {
    const st = useStore.getState();
    st.set({ packName: item.title, packPct: 0 });
    setBusy(true);
    try {
      const inst = await api.installModpack(item.source, item.id, versionId);
      if (item.iconUrl) {
        await api.setInstanceIconUrl(inst.id, item.iconUrl).catch(() => {});
      }
      const list = await api.listInstances();
      useStore.getState().set({ instances: list, tgt: inst.id });
      useStore.getState().toastMsg(t('Сборка установлена: {title}', { title: item.title }));
      setPack(null);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      useStore.getState().set({ packPct: null });
      setBusy(false);
    }
  }, [t]);

  /** Открывает выбор версии сборки (ТЗ §11). */
  const openPack = useCallback(async (item: CatalogItem) => {
    setPack(item);
    setPackVersions([]);
    setPackPick('');
    setPackLoading(true);
    try {
      const list = await api.listModpackVersions(item.source, item.id);
      setPackVersions(list);
      setPackPick(list[0]?.id ?? '');
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setPackLoading(false);
    }
  }, []);

  /** Контент, для которого сейчас выбирается профиль. */
  const [pickInst, setPickInst] = useState<CatalogItem | null>(null);

  /** Открытая страница мода. */
  const [detail, setDetail] = useState<CatalogItem | null>(null);
  const [det, setDet] = useState<ProjectDetails | null>(null);
  const [detBusy, setDetBusy] = useState(false);

  const openDetail = useCallback(async (item: CatalogItem) => {
    setDetail(item);
    setDet(null);
    setDetBusy(true);
    try {
      setDet(await api.projectDetails(item.source, item.id));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setDetBusy(false);
    }
  }, []);

  /** Ставит контент в выбранный профиль (датапак — ещё и в выбранный мир). */
  const chooseProfile = useCallback(
    async (id: string) => {
      const item = pickInst;
      setPickInst(null);
      if (!item) return;
      useStore.getState().set({ tgt: id });
      if (item.kind === 'dp') {
        try {
          const list = await api.listWorlds(id);
          if (list.length === 0) {
            useStore.getState().toastMsg(t('Сначала создайте мир'));
            return;
          }
          setPickWorlds(list.map((w) => ({ dir: w.dir, name: w.name })));
          setPick(item);
        } catch (e) {
          useStore.getState().toastMsg(String(e));
        }
        return;
      }
      await installInto(item, null, id);
    },
    [pickInst, installInto, t],
  );

  const onInstall = useCallback(
    async (item: CatalogItem) => {
      if (item.kind === 'pk') {
        await openPack(item);
        return;
      }
      if (useStore.getState().instances.length === 0) {
        useStore.getState().toastMsg(t('Сначала создайте профиль'));
        return;
      }
      // Куда ставить — спрашиваем всегда, а не молча в выбранный профиль.
      setPickInst(item);
    },
    [openPack, t],
  );

  /** Импорт файла .mrpack / .zip через системный диалог (ТЗ §11). */
  const importFile = useCallback(async () => {
    let path: string | string[] | null = null;
    try {
      path = await open({
        multiple: false,
        filters: [{ name: t('Модпаки'), extensions: ['mrpack', 'zip'] }],
      });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
      return;
    }
    if (typeof path !== 'string') return;
    const file = path.split(/[\\/]/).pop() ?? t('Сборка');
    useStore.getState().set({ packName: file, packPct: 0 });
    setBusy(true);
    try {
      const inst = await api.importPack(path);
      const list = await api.listInstances();
      useStore.getState().set({ instances: list, tgt: inst.id });
      useStore.getState().toastMsg(t('Сборка импортирована: {name}', { name: inst.name }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      useStore.getState().set({ packPct: null });
      setBusy(false);
    }
  }, [t]);

  const closePick = (e: MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) setPick(null);
  };

  const closePack = (e: MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) setPack(null);
  };

  const releases = versions.filter((v) => v.kind === 'release').slice(0, 60);
  const pages = Math.max(1, Math.ceil(catTotal / PAGE));

  return (
    <>
      <div className="hd">
        <div className="lb">{t('Каталог')}</div>
        <div className="ttl">{src}</div>
        <div className="tabs">
          {KINDS.filter(([k]) => src !== 'FTB' || k === 'pk').map(([k, l]) => (
            <div
              key={k}
              className={'tab' + (bt === k ? ' on' : '')}
              data-k={'btab-' + k}
              onClick={() => useStore.getState().set({ bt: k })}
            >
              {t(l)}
            </div>
          ))}
        </div>
      </div>
      <div className="pg" ref={pgRef}>
        <div className="row g8" style={{ marginBottom: 16 }}>
          <div className="seg">
            {['Modrinth', 'CurseForge', 'FTB'].map((x) => (
              <button
                key={x}
                type="button"
                className={src === x ? 'on' : ''}
                onClick={() =>
                  useStore.getState().set({
                    src: x,
                    ...(x === 'FTB' ? { bt: 'pk' } : {}),
                  })
                }
              >
                {x}
              </button>
            ))}
          </div>
          <span style={{ flex: 1 }} />
          <span className="hint">{t('Установить в')}</span>
          <div style={{ width: 200 }}>
            <select
              value={tgt ?? ''}
              onChange={(e) => useStore.getState().set({ tgt: e.target.value })}
            >
              {instances.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.name}
                </option>
              ))}
            </select>
          </div>
          <button className="btn" disabled={busy} onClick={() => void importFile()}>
            <IUp />
            .mrpack / .zip
          </button>
        </div>

        {packPct !== null && (
          <div className="panel" style={{ padding: '12px 16px', marginBottom: 14 }}>
            <div className="row g8">
              <b style={{ fontWeight: 500 }}>{t('Установка сборки: {name}', { name: packName })}</b>
              <span style={{ flex: 1 }} />
              <span className="hint">{Math.round(packPct)}%</span>
            </div>
            <div className="sbprog" style={{ marginTop: 10, position: 'relative', left: 0, right: 0, bottom: 0 }}>
              <i style={{ width: packPct + '%' }} />
            </div>
          </div>
        )}

        <div className="row" style={{ alignItems: 'flex-start', gap: 18 }}>
          <div className="panel fl">
            {bt !== 'sh' && bt !== 'rp' && bt !== 'dp' && (
              <>
                <h3>{t('Загрузчик')}</h3>
                {LOADERS.map((l) => (
                  <Check key={l} on={bl.includes(l)} label={l} onClick={() => toggleLoader(l)} />
                ))}
              </>
            )}
            {bt !== 'rp' && bt !== 'sh' && (
              <>
                <h3>{t('Версия игры')}</h3>
                <select value={bv} onChange={(e) => useStore.getState().set({ bv: e.target.value })}>
                  <option value="">{t('Любая')}</option>
                  {releases.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.id}
                    </option>
                  ))}
                </select>
              </>
            )}
            <h3>{t('Сортировка')}</h3>
            <select value={bs} onChange={(e) => useStore.getState().set({ bs: e.target.value })}>
              {SORTS.map(([k, l]) => (
                <option key={k} value={k}>
                  {t(l)}
                </option>
              ))}
            </select>
            {src !== 'FTB' && (
              <>
                <h3>{t('Категория')}</h3>
                {CATS.map((c) => (
                  <Check key={c} on={bc.includes(c)} label={t(c)} onClick={() => toggleCat(c)} />
                ))}
              </>
            )}
          </div>

          <div style={{ flex: 1, minWidth: 0 }}>
            <input
              type="text"
              placeholder={t('Поиск в {src}…', { src })}
              value={bq}
              onChange={(e) => useStore.getState().set({ bq: e.target.value })}
              style={{ marginBottom: 12 }}
            />
            <div className="panel">
              {cat.length > 0 ? (
                cat.map((c, n) => (
                  <CatalogRow
                    key={c.source + ':' + c.id}
                    item={c}
                    index={n}
                    busy={busy}
                    onInstall={onInstall}
                    onOpen={openDetail}
                  />
                ))
              ) : (
                <div className="hint" style={{ padding: 40, textAlign: 'center' }}>
                  {catBusy ? t('Загрузка…') : t('Ничего не найдено')}
                </div>
              )}
            </div>

            {pages > 1 && (
              <div className="pager">
                <button
                  className="btn sm pager-btn"
                  disabled={page === 0 || catBusy}
                  aria-label={t('Назад')}
                  title={t('Назад')}
                  onClick={() => setPage((p) => Math.max(0, p - 1))}
                >
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M15 5l-7 7 7 7" />
                  </svg>
                </button>
                <span className="hint">
                  {page + 1} / {pages}
                </span>
                <button
                  className="btn sm pager-btn"
                  disabled={page + 1 >= pages || catBusy}
                  aria-label={t('Вперёд')}
                  title={t('Вперёд')}
                  onClick={() => setPage((p) => p + 1)}
                >
                  <svg viewBox="0 0 24 24" aria-hidden="true">
                    <path d="M9 5l7 7-7 7" />
                  </svg>
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {detail && (
        <div
          className="ov"
          onClick={(e) => {
            if (e.target === e.currentTarget) setDetail(null);
          }}
        >
          <div className="mod wide">
            <div className="r">
              <div className="row g8" style={{ alignItems: 'flex-start', gap: 14 }}>
                <CatIcon item={detail} size={64} />
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 800, fontSize: 20 }}>{detail.title}</div>
                  <div className="hint">{t('от {author}', { author: detail.author })}</div>
                  <div className="row g8" style={{ flexWrap: 'wrap', marginTop: 8 }}>
                    <span className="lab">{SOURCE_LABEL[detail.source] ?? detail.source}</span>
                    {detail.loaders
                      .filter((l) => l.toLowerCase() !== 'vanilla')
                      .map((l) => (
                        <span key={l} className="lab">
                          {l}
                        </span>
                      ))}
                    {detail.categories.slice(0, 4).map((c) => (
                      <span key={c} className="hint">
                        {c}
                      </span>
                    ))}
                  </div>
                </div>
              </div>

              <p className="hint" style={{ marginTop: 12 }}>
                {plainText(detail.description)}
              </p>

              {det && det.links.length > 0 && (
                <>
                  <h2 style={{ marginTop: 16 }}>{t('Ссылки')}</h2>
                  <div className="row g8" style={{ flexWrap: 'wrap' }}>
                    {det.links.map((l) => (
                      <button
                        key={l.url}
                        className="btn sm"
                        onClick={() => void openUrl(l.url).catch((e) => useStore.getState().toastMsg(String(e)))}
                      >
                        {t(l.label)}
                      </button>
                    ))}
                  </div>
                </>
              )}

              <h2 style={{ marginTop: 16 }}>{t('Описание')}</h2>
              <div className="mod-body">
                {detBusy ? (
                  t('Загрузка…')
                ) : det?.body?.trim() ? (
                  <Markdown text={det.body} />
                ) : (
                  t('Описание недоступно')
                )}
              </div>

              <div className="row g8" style={{ marginTop: 18, justifyContent: 'flex-end' }}>
                <button className="btn" onClick={() => setDetail(null)}>
                  {t('Закрыть')}
                </button>
                <button
                  className="btn pr"
                  disabled={busy || (detail.installed && detail.kind !== 'pk')}
                  onClick={() => {
                    const it = detail;
                    setDetail(null);
                    void onInstall(it);
                  }}
                >
                  {detail.kind === 'pk' ? t('Создать профиль') : detail.installed ? t('Установлено') : t('Установить')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {pickInst && (
        <div
          className="ov"
          onClick={(e) => {
            if (e.target === e.currentTarget) setPickInst(null);
          }}
        >
          <div className="mod">
            <div className="r">
              <h2>{t('Куда установить?')}</h2>
              <p className="hint" style={{ marginBottom: 14 }}>
                {pickInst.title}
              </p>
              <div className="acc-pick" style={{ maxHeight: 320, overflow: 'auto' }}>
                {instances.map((x) => (
                  <button key={x.id} type="button" className="acc-card" onClick={() => void chooseProfile(x.id)}>
                    <span className="acc-tx">
                      <b>{x.name}</b>
                      <span className="hint">
                        {LOADER_LABEL[x.loader]} {x.mcVersion}
                      </span>
                    </span>
                    <span className="acc-go">→</span>
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {pick && (
        <div className="ov" onClick={closePick}>
          <div className="mod">
            <div className="r">
              <h2>{t('Выберите мир')}</h2>
              <p className="hint" style={{ marginBottom: 14 }}>
                {t('Датапак будет установлен в папку выбранного мира.')}
              </p>
              <div style={{ maxHeight: 300, overflow: 'auto' }}>
                {pickWorlds.map((w) => (
                  <div
                    key={w.dir}
                    className="lr"
                    style={{ cursor: 'pointer' }}
                    onClick={() => {
                      const item = pick;
                      setPick(null);
                      void installInto(item, w.dir);
                    }}
                  >
                    <div className="d">
                      <b style={{ fontWeight: 500 }}>{w.name}</b>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}

      {pack && (
        <div className="ov" onClick={closePack}>
          <div className="mod">
            <div className="r">
              <h2>{t('Версия сборки')}</h2>
              <p className="hint" style={{ marginBottom: 14 }}>
                {pack.title} · {t('от {author}', { author: pack.author })}
              </p>
              {packLoading ? (
                <div className="hint" style={{ padding: 24, textAlign: 'center' }}>
                  {t('Загрузка версий…')}
                </div>
              ) : packVersions.length === 0 ? (
                <div className="hint" style={{ padding: 24, textAlign: 'center' }}>
                  {t('Подходящих версий не нашлось')}
                </div>
              ) : (
                <>
                  <div style={{ maxHeight: 300, overflow: 'auto' }}>
                    {packVersions.map((v) => (
                      <div
                        key={v.id}
                        className="lr"
                        style={{ cursor: 'pointer', alignItems: 'center' }}
                        onClick={() => setPackPick(v.id)}
                      >
                        <div
                          className={'ck' + (packPick === v.id ? ' on' : '')}
                          style={{ padding: 0 }}
                        >
                          <i style={{ borderRadius: '50%' }} />
                        </div>
                        <div className="d">
                          <b style={{ fontWeight: 500 }}>{v.name || v.id}</b>
                          <div className="hint">
                            {[v.mcVersion, v.loader, v.date ? fmtShortDate(v.date) : '']
                              .filter(Boolean)
                              .join(' · ')}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                  <button
                    className="btn pr"
                    style={{ marginTop: 16 }}
                    disabled={busy || !packPick}
                    onClick={() => void installPack(pack, packPick)}
                  >
                    {t('Создать профиль')}
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
