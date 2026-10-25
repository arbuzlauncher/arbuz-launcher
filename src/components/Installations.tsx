// Экран «Установки»: все профили лаунчера в виде карточек.
import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { pi } from '../lib/art';
import { api, type Instance, type ModrinthProfile } from '../lib/api';
import { fmtRelative, gb } from '../lib/format';
import { useT } from '../lib/i18n';
import { isRunning, playInstance } from '../lib/launch';
import { LOADER_LABEL } from '../lib/labels';
import { askConfirm, cancelSave, curAccount, iconUrl, useArtTheme, useStore, visibleInstances } from '../store';
import { AiPackModal } from './AiPackModal';
import { Avatar } from './Controls';
import { ExportPackModal } from './ExportPackModal';
import { NewProfileModal } from './NewProfileModal';

export function Installations() {
  const s = useStore();
  const t = useT();
  const theme = useArtTheme();
  const acc = curAccount(s);
  const list = visibleInstances(s);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  const [newOpen, setNewOpen] = useState(false);
  const [aiOpen, setAiOpen] = useState(false);
  const [mrOpen, setMrOpen] = useState(false);
  const [mrList, setMrList] = useState<ModrinthProfile[] | null>(null);
  const [mrBusy, setMrBusy] = useState(false);
  const [prismOpen, setPrismOpen] = useState(false);
  const [exportFor, setExportFor] = useState<Instance | null>(null);

  /** Готовая сборка файлом: CurseForge .zip, Modrinth .mrpack или архив MultiMC/Prism. */
  const importPackFile = async (kind: 'curseforge' | 'modrinth' | 'prism') => {
    const ext = kind === 'modrinth' ? 'mrpack' : 'zip';
    const filter =
      kind === 'curseforge'
        ? t('Сборка CurseForge')
        : kind === 'modrinth'
          ? t('Сборка Modrinth')
          : t('Архив MultiMC/Prism');
    let path: string | string[] | null = null;
    try {
      path = await open({
        multiple: false,
        filters: [{ name: filter, extensions: [ext] }],
      });
    } catch (e) {
      useStore.getState().toastMsg(String(e));
      return;
    }
    if (typeof path !== 'string') return;

    const file = path.split(/[\\/]/).pop() ?? t('Сборка');
    const st = useStore.getState();
    st.set({ packName: file, packPct: 0 });
    try {
      const inst = await api.importPack(path);
      const list = await api.listInstances();
      useStore.getState().set({ instances: list, sel: inst.id });
      useStore.getState().toastMsg(t('Сборка импортирована: {name}', { name: inst.name }));
      if (kind === 'modrinth') setMrOpen(false);
      if (kind === 'prism') setPrismOpen(false);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      useStore.getState().set({ packPct: null });
    }
  };

  const openModrinth = async () => {
    setMrOpen(true);
    setMrList(null);
    setMrBusy(true);
    try {
      setMrList(await api.listModrinthProfiles());
    } catch (e) {
      useStore.getState().toastMsg(String(e));
      setMrList([]);
    } finally {
      setMrBusy(false);
    }
  };

  const importModrinth = async (p: ModrinthProfile) => {
    setMrBusy(true);
    try {
      const inst = await api.importModrinthProfile(p.path);
      const st = useStore.getState();
      st.set({ instances: [...st.instances, inst], sel: inst.id });
      st.toastMsg(t('Профиль импортирован: {name}', { name: inst.name }));
      setMrOpen(false);
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setMrBusy(false);
    }
  };

  const importPrism = async () => {
    try {
      const p = await open({ directory: true, multiple: false });
      if (typeof p !== 'string') return;
      const inst = await api.importPrismInstance(p);
      const st = useStore.getState();
      st.set({ instances: [...st.instances, inst], sel: inst.id });
      st.toastMsg(t('Профиль импортирован: {name}', { name: inst.name }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    }
  };

  const remove = async (i: Instance) => {
    setMenuFor(null);
    const ok = await askConfirm({
      title: t('Удалить профиль?'),
      text: t('Профиль «{name}» и его папка с модами, конфигами и мирами будут удалены безвозвратно.', { name: i.name }),
      yes: t('Удалить'),
      danger: true,
    });
    if (!ok) return;
    cancelSave(i.id);
    try {
      await api.deleteInstance(i.id);
      const rest = s.instances.filter((x) => x.id !== i.id);
      s.set({ instances: rest, sel: rest[0]?.id ?? null });
      if (s.sel === i.id) s.set({ view: 'installs' });
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  return (
    <div className="screen">
      <div className="screen-head">
        <div>
          <div className="screen-title">{t('Установки')}</div>
          <div className="screen-sub">{t('Каждая установка живёт в своей папке со своими модами и мирами.')}</div>
        </div>
        <div className="row g8" style={{ flexWrap: 'wrap', justifyContent: 'flex-end' }}>
          <button className="btn" onClick={() => void importPackFile('curseforge')}>
            {t('Импорт CurseForge')}
          </button>
          <button className="btn" onClick={() => setPrismOpen(true)}>
            {t('Импорт MultiMC/Prism')}
          </button>
          <button className="btn" onClick={() => void openModrinth()}>
            {t('Импорт Modrinth')}
          </button>
          {s.config.aiBuilder && (
            <button className="btn" onClick={() => setAiOpen(true)}>
              {t('ИИ-сборка')}
            </button>
          )}
          <button className="btn pr" onClick={() => setNewOpen(true)}>
            + {t('Новый профиль')}
          </button>
        </div>
      </div>

      {menuFor && <div className="menu-backdrop" onClick={() => setMenuFor(null)} />}

      <div className="inst-list">
        {list.map((i) => {
          const on = s.sel === i.id;
          const run = isRunning(s, i.id);
          const tone = i.icon || i.iconPreset ? (
            pi({ biome: i.biome, icon: iconUrl(s.dataRoot, i.id, i.icon, s.iconBust[i.id]), preset: i.iconPreset }, 48, s.instances.indexOf(i), theme)
          ) : (
            <Avatar name={acc?.name ?? '?'} skin={acc ? s.skins[acc.id] : null} size={48} />
          );
          return (
            <div className={'inst-card' + (on ? ' on' : '')} key={i.id}>
              <button
                type="button"
                className="inst-main"
                onClick={() => s.set({ view: 'p', sel: i.id, sec: 'g', menu: false })}
              >
                <span className="inst-tone">{tone}</span>
                <span className="inst-info">
                  <span className="inst-name">
                    {i.name}
                    {on && <span className="sel">{t('Выбран')}</span>}
                  </span>
                  <span className="inst-tags">
                    <span className="inst-tag">{LOADER_LABEL[i.loader]}</span>
                    <span>{i.mcVersion}</span>
                    <span>{gb(i.ramMaxMb)}</span>
                    <span className="inst-path">instances/{i.id.slice(0, 8)}</span>
                  </span>
                </span>
              </button>

              <span className="inst-last">{fmtRelative(i.lastPlayed)}</span>
              <div className="inst-actions">
                <button
                  type="button"
                  className="inst-tone"
                  style={{ width: 38, height: 38, borderRadius: 5, cursor: 'pointer', border: 0 }}
                  title={t('Открыть папку')}
                  onClick={() => void api.openInstanceFolder(i.id).catch((e) => s.toastMsg(String(e)))}
                >
                  <svg viewBox="0 0 24 24" width="17" height="17" style={{ width: 17, height: 17 }} aria-hidden="true">
                    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z" fill="none" stroke="currentColor" strokeWidth="2" />
                  </svg>
                </button>
                <button
                  type="button"
                  className="btn pr"
                  style={{ height: 38, padding: '0 16px' }}
                  onClick={() => void playInstance(i.id, null)}
                >
                  {run ? t('Стоп') : t('Играть')}
                </button>
                <button
                  type="button"
                  className={'btn inst-more' + (menuFor === i.id ? ' on' : '')}
                  title={t('Ещё')}
                  onClick={(e) => {
                    e.stopPropagation();
                    setMenuFor(menuFor === i.id ? null : i.id);
                  }}
                >
                  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                    <circle cx="12" cy="5" r="1.7" />
                    <circle cx="12" cy="12" r="1.7" />
                    <circle cx="12" cy="19" r="1.7" />
                  </svg>
                </button>
              </div>
              {menuFor === i.id && (
                <div className="inst-menu">
                  <button
                    type="button"
                    onClick={() => {
                      setMenuFor(null);
                      s.set({ view: 'p', sel: i.id, sec: 'g', menu: false });
                    }}
                  >
                    {t('Настройки')}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setMenuFor(null);
                      void api.openInstanceFolder(i.id).catch((e) => s.toastMsg(String(e)));
                    }}
                  >
                    {t('Открыть папку')}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setMenuFor(null);
                      setExportFor(i);
                    }}
                  >
                    {t('Экспорт…')}
                  </button>
                  <button type="button" className="danger" onClick={() => void remove(i)}>
                    {t('Удалить')}
                  </button>
                </div>
              )}
            </div>
          );
        })}
      </div>

      {mrOpen && (
        <div
          className="ov"
          onClick={(e) => {
            if (e.target === e.currentTarget && !mrBusy) setMrOpen(false);
          }}
        >
          <div className="mod">
            <div className="r">
              <h2>{t('Импорт Modrinth')}</h2>
              <p className="hint" style={{ marginBottom: 14 }}>
                {t('Моды, конфиги и миры профиля скопируются в новый профиль лаунчера.')}
              </p>

              {mrList === null && <div className="hint">{t('Ищу профили…')}</div>}
              {mrList && mrList.length === 0 && (
                <div className="hint">{t('Профили Modrinth App не найдены.')}</div>
              )}
              {mrList && mrList.length > 0 && (
                <div className="acc-pick" style={{ maxHeight: 360, overflow: 'auto' }}>
                  {mrList.map((p) => (
                    <button
                      key={p.path}
                      type="button"
                      className="acc-card"
                      disabled={mrBusy}
                      onClick={() => void importModrinth(p)}
                    >
                      <span className="acc-tx">
                        <b>{p.name}</b>
                        <span className="hint">
                          {(LOADER_LABEL as Record<string, string>)[p.loader] ?? p.loader}{' '}
                          {p.mcVersion}
                          {p.loaderVersion ? ` · ${p.loaderVersion}` : ''}
                          {p.mods > 0 ? ` · ${t('модов: {n}', { n: p.mods })}` : ''}
                        </span>
                      </span>
                      <span className="acc-go">→</span>
                    </button>
                  ))}
                </div>
              )}

              <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 18 }}>
                <button className="btn" disabled={mrBusy} onClick={() => void importPackFile('modrinth')}>
                  {t('Выбрать .mrpack')}
                </button>
                <button className="btn" onClick={() => setMrOpen(false)} disabled={mrBusy}>
                  {t('Закрыть')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {exportFor && <ExportPackModal inst={exportFor} onClose={() => setExportFor(null)} />}

      {prismOpen && (
        <div
          className="ov"
          onClick={(e) => {
            if (e.target === e.currentTarget) setPrismOpen(false);
          }}
        >
          <div className="mod">
            <div className="r">
              <h2>{t('Импорт MultiMC/Prism')}</h2>
              <p className="hint" style={{ marginBottom: 14 }}>
                {t('Выберите папку установленного профиля или архив .zip, который экспортировал лаунчер.')}
              </p>
              <div className="row g8" style={{ flexWrap: 'wrap' }}>
                <button
                  className="btn pr"
                  onClick={() => {
                    setPrismOpen(false);
                    void importPrism();
                  }}
                >
                  {t('Папка профиля')}
                </button>
                <button className="btn" onClick={() => void importPackFile('prism')}>
                  {t('Архив .zip')}
                </button>
              </div>
              <div className="row g8" style={{ justifyContent: 'flex-end', marginTop: 18 }}>
                <button className="btn" onClick={() => setPrismOpen(false)}>
                  {t('Закрыть')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      <NewProfileModal open={newOpen} onClose={() => setNewOpen(false)} />
      <AiPackModal open={aiOpen} onClose={() => setAiOpen(false)} />
    </div>
  );
}
