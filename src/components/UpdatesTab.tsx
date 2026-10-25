// Настройки профиля → «Обновления»: моды и сборка обновляются только по кнопке.
import { useCallback, useEffect, useState } from 'react';
import { api, type Instance, type PackUpdate, type UpdateInfo } from '../lib/api';
import { useT } from '../lib/i18n';
import { Markdown } from '../lib/markdown';
import { applyInstance, loadProfileContent, useStore } from '../store';
import { Check } from './Controls';
import { ModIcon } from './ContentTab';

export function UpdatesTab({ inst }: { inst: Instance }) {
  const t = useT();
  const items = useStore((s) => s.content);
  const [busy, setBusy] = useState(false);
  /** Проверку запускает игрок — сами по себе обновления не ищутся. */
  const [checked, setChecked] = useState(false);
  const [found, setFound] = useState<UpdateInfo[]>([]);
  const [pick, setPick] = useState<Record<string, boolean>>({});
  const [packUpd, setPackUpd] = useState<PackUpdate | null>(null);
  const [packChecked, setPackChecked] = useState(false);

  // Имена и иконки модов берём из уже просканированного контента профиля.
  useEffect(() => {
    void loadProfileContent(inst.id).catch((e) => useStore.getState().toastMsg(String(e)));
  }, [inst.id]);

  const check = useCallback(async () => {
    setBusy(true);
    try {
      const list = await api.checkUpdates(inst.id);
      setFound(list);
      setPick(Object.fromEntries(list.map((u) => [u.file, true])));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setChecked(true);
      setBusy(false);
    }
  }, [inst.id]);

  const chosen = found.filter((u) => pick[u.file]).map((u) => u.file);

  const apply = async () => {
    if (!chosen.length) return;
    setBusy(true);
    try {
      const n = await api.applyUpdates(inst.id, chosen);
      useStore.getState().toastMsg(t('Обновлено файлов: {n}', { n }));
      await loadProfileContent(inst.id);
      setFound([]);
      setChecked(false);
      await check();
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const checkPack = useCallback(async () => {
    if (!inst.pack) return;
    setBusy(true);
    try {
      setPackUpd(await api.packUpdate(inst.id));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setPackChecked(true);
      setBusy(false);
    }
  }, [inst.id, inst.pack]);

  const updatePack = useCallback(async () => {
    if (!packUpd) return;
    setBusy(true);
    try {
      const next = await api.applyPackUpdate(inst.id, packUpd.versionId);
      applyInstance(next);
      setPackUpd(null);
      await loadProfileContent(inst.id);
      useStore
        .getState()
        .toastMsg(t('Сборка обновлена до {v}', { v: next.pack?.versionName || packUpd.versionName }));
    } catch (e) {
      useStore.getState().toastMsg(String(e));
    } finally {
      setBusy(false);
    }
  }, [inst.id, packUpd, t]);

  const byFile = new Map(items.map((i) => [i.file, i]));

  return (
    <div>
      <h2 style={{ marginTop: 0 }}>{t('Обновления')}</h2>
      <p className="hint" style={{ marginBottom: 14 }}>
        {t('Моды и сборка не обновляются сами: лаунчер проверит новые версии, только когда вы нажмёте кнопку.')}
      </p>

      <div className="panel" style={{ padding: 14 }}>
        <div className="row g8" style={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
          <div style={{ minWidth: 0 }}>
            <b>{t('Моды и контент')}</b>
            <div className="hint">
              {checked && found.length === 0
                ? t('Обновлений нет — всё уже свежее.')
                : found.length > 0
                  ? t('Доступно обновлений: {n}', { n: found.length })
                  : t('Вышли ли новые версии модов, ресурспаков и шейдеров.')}
            </div>
          </div>
          <div className="row g8">
            <button className="btn sm" disabled={busy} onClick={() => void check()}>
              {busy && !found.length ? t('Проверяю…') : t('Обновить моды')}
            </button>
            {found.length > 0 && (
              <button className="btn pr" disabled={busy || !chosen.length} onClick={() => void apply()}>
                {t('Обновить выбранные ({n})', { n: chosen.length })}
              </button>
            )}
          </div>
        </div>

        {found.length > 0 && (
          <div className="upd-list">
            {found.map((u) => {
              const it = byFile.get(u.file);
              return (
                <div className="upd-row" key={u.file}>
                  <Check
                    on={!!pick[u.file]}
                    label=""
                    onClick={() => setPick((p) => ({ ...p, [u.file]: !p[u.file] }))}
                  />
                  {it ? (
                    <ModIcon item={it} size={28} />
                  ) : (
                    <span className="mi" style={{ width: 28, height: 28 }} />
                  )}
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <b style={{ fontWeight: 500 }}>{it?.name ?? u.file}</b>
                    <div className="hint">
                      {it?.version || '—'} → <b style={{ color: 'var(--pri)' }}>{u.newVersion || u.name}</b>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {inst.pack && (
        <div className="panel" style={{ padding: 14, marginTop: 14 }}>
          <div className="row g8" style={{ justifyContent: 'space-between', alignItems: 'flex-start' }}>
            <div style={{ minWidth: 0 }}>
              <b>
                {inst.pack.source === 'curseforge'
                  ? t('Сборка CurseForge')
                  : inst.pack.source === 'ftb'
                    ? t('Сборка FTB')
                    : t('Сборка Modrinth')}
              </b>
              <div className="hint">
                {t('Версия сборки: {v}', { v: inst.pack.versionName || inst.pack.versionId })}
              </div>
              {packUpd && (
                <div className="hint" style={{ marginTop: 6, color: 'var(--pri)' }}>
                  {t('Доступна новая версия: {v}', { v: packUpd.versionName })}
                </div>
              )}
              {packChecked && !packUpd && (
                <div className="hint" style={{ marginTop: 6 }}>
                  {t('Обновлений сборки нет — у вас последняя версия.')}
                </div>
              )}
            </div>
            <div className="row g8">
              <button className="btn sm" disabled={busy} onClick={() => void checkPack()}>
                {t('Проверить обновления сборки')}
              </button>
              {packUpd && (
                <button className="btn pr" disabled={busy} onClick={() => void updatePack()}>
                  {t('Обновить сборку до {v}', { v: packUpd.versionName })}
                </button>
              )}
            </div>
          </div>
          {packUpd?.changelog && (
            <div className="pk-log">
              <Markdown text={packUpd.changelog} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
