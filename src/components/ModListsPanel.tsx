// Сохранённые списки модов: снимок текущего контента и установка в профиль.
import { useCallback, useEffect, useState } from 'react';
import { api, type Instance, type SavedModList } from '../lib/api';
import { useT } from '../lib/i18n';
import { useStore } from '../store';

export function ModListsPanel({
  inst,
  onInstalled,
}: {
  inst: Instance;
  onInstalled?: () => Promise<void> | void;
}) {
  const t = useT();
  const [lists, setLists] = useState<SavedModList[] | null>(null);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState<string | null>(null);

  const toast = (m: string) => useStore.getState().toastMsg(m);

  const load = useCallback(async () => {
    try {
      setLists(await api.modlistsList());
    } catch (e) {
      toast(String(e));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const saveCurrent = async () => {
    setBusy(true);
    try {
      const l = await api.modlistSave(inst.id, name.trim());
      setName('');
      await load();
      toast(t('Список сохранён: {n} модов', { n: l.mods.length }));
    } catch (e) {
      toast(String(e));
    } finally {
      setBusy(false);
    }
  };

  const install = async (l: SavedModList) => {
    setInstalling(l.id);
    try {
      const n = await api.modlistInstall(l.id, inst.id);
      await onInstalled?.();
      toast(t('Установлено модов: {n}', { n }));
    } catch (e) {
      toast(String(e));
    } finally {
      setInstalling(null);
    }
  };

  const remove = async (l: SavedModList) => {
    try {
      await api.modlistDelete(l.id);
      await load();
      toast(t('Список удалён'));
    } catch (e) {
      toast(String(e));
    }
  };

  return (
    <div className="panel modlists-panel">
      <div className="conflict-head">
        <b>{t('Сохранённые списки модов')}</b>
        <span className="hint">{lists ? t('Всего: {n}', { n: lists.length }) : t('Загрузка…')}</span>
      </div>
      <div className="row g8" style={{ margin: '4px 0 10px' }}>
        <input
          type="text"
          style={{ flex: 1 }}
          placeholder={t('Название списка')}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button className="btn sm pr" disabled={busy} onClick={() => void saveCurrent()}>
          {t('Сохранить текущий')}
        </button>
      </div>
      {lists && lists.length === 0 && (
        <div className="hint">{t('Списков пока нет. Сохраните текущий набор модов.')}</div>
      )}
      {(lists ?? []).map((l) => (
        <div className="modlist-row" key={l.id}>
          <div className="modlist-main">
            <b>{l.name}</b>
            <span className="hint">
              {l.mods.length} · {l.loader} {l.mcVersion}
            </span>
          </div>
          <button
            className="btn sm"
            disabled={installing !== null}
            onClick={() => void install(l)}
          >
            {installing === l.id ? t('Ставлю…') : t('Установить')}
          </button>
          <button className="btn sm" onClick={() => void remove(l)}>
            {t('Удалить')}
          </button>
        </div>
      ))}
    </div>
  );
}
