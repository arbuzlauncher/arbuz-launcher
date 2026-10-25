import { memo, useEffect, useMemo, useState, type CSSProperties } from 'react';
import { art, artCols, pi } from '../lib/art';
import type { Instance } from '../lib/api';
import { api, type PinnedServer } from '../lib/api';
import { fmtHours, fmtRelative, gb } from '../lib/format';
import { playInstance, stat } from '../lib/launch';
import { LOADER_LABEL } from '../lib/labels';
import { useT } from '../lib/i18n';import { builtinWallpaper } from '../lib/builtin';
import { curAccount, iconUrl, saveInstance, useArtTheme, useStore } from '../store';
import { Avatar, PlayIcon } from './Controls';
import { CustomVersionModal } from './CustomVersionModal';
import { LanShare } from './LanShare';

/** Группы версий в селекторе, свои — сверху. */
const VERSION_GROUPS: [string, string][] = [
  ['custom', 'Свои версии'],
  ['release', 'Релизы'],
  ['snapshot', 'Снапшоты'],
  ['old_beta', 'Старые беты'],
  ['old_alpha', 'Старые альфы'],
];

function dayKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

export const PlayTab = memo(function PlayTab({ inst }: { inst: Instance }) {
  const s = useStore();
  const t = useT();
  const theme = useArtTheme();
  const acc = curAccount(s);
  const skin = acc ? s.skins[acc.id] : null;
  const [customOpen, setCustomOpen] = useState(false);
  const k = Math.max(0, s.instances.indexOf(inst));

  const hero = useMemo(
    () => art(inst.biome, inst.created % 100000, 912, 268, 24, artCols(inst.biome, k, theme)),
    [inst.biome, inst.created, k, theme],
  );
  const wp = inst.wallpaper ? builtinWallpaper(inst.wallpaper) : '';

  // Гистограмма за последние 7 дней по реальному playtime_days
  const bars = useMemo(() => {
    const now = new Date();
    const out: number[] = [];
    for (let d = 6; d >= 0; d--) {
      const dt = new Date(now.getFullYear(), now.getMonth(), now.getDate() - d);
      out.push(inst.playtimeDays[dayKey(dt)] ?? 0);
    }
    const max = Math.max(1, ...out);
    return out.map((v) => 8 + Math.round((v / max) * 26));
  }, [inst.playtimeDays]);

  // Закреплённые серверы профиля: быстрый вход прямо из лаунчера.
  const [pinned, setPinned] = useState<PinnedServer[]>([]);
  useEffect(() => {
    let alive = true;
    void api
      .pinnedServers(inst.id)
      .then((list) => {
        if (alive) setPinned(list);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [inst.id, s.live[inst.id]]);

  // Плитка «Последний мир»: бэкенд уже отсортировал миры по свежести.
  const lastWorld = s.worlds[0] ?? null;
  const worldArt = useMemo(
    () => art(inst.biome, (inst.created % 100000) + 40, 288, 180, 12, artCols(inst.biome, k, theme)),
    [inst.biome, inst.created, k, theme],
  );
  const contentCount = s.content.length;

  // «Остановить» показываем только когда процесс игры реально стартовал.
  // Во время скачивания кнопка показывает прогресс (как в макете, pbH).
  const live = s.live[inst.id] === true;
  const pct = s.running[inst.id];

  const openFolder = async () => {
    try {
      await api.openInstanceFolder(inst.id);
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  return (
    <>
      <div className="hero">
        {wp ? (
          <img className="hero-img" src={wp} alt="" />
        ) : (
          <div style={{ position: 'absolute', inset: 0 }} dangerouslySetInnerHTML={{ __html: hero }} />
        )}
        <div className="hc">
          <span className="lab">
            {LOADER_LABEL[inst.loader]} {inst.mcVersion}
          </span>
          <span className="lab">Java {inst.javaMajor}</span>
        </div>
        <div className="row g8 ho">
          <button
            className="btn sm"
            onClick={() => useStore.getState().set({ console: inst.id })}
            title={t('Консоль и логи Minecraft')}
          >
            {t('Консоль')}
          </button>
          <button className="btn sm" onClick={() => void openFolder()}>
            {t('Открыть папку')}
          </button>
        </div>
      </div>
      <div className="ctl">
        <div className="vs">
          {inst.icon || inst.iconPreset ? (
            pi({ biome: inst.biome, icon: iconUrl(s.dataRoot, inst.id, inst.icon, s.iconBust[inst.id]), preset: inst.iconPreset }, 32, k, theme)
          ) : (
            <Avatar name={acc?.name ?? '?'} skin={skin} size={32} />
          )}
          <div style={{ flex: 1 }}>
            <b>Minecraft {inst.mcVersion}</b>
            <small>{LOADER_LABEL[inst.loader]} · {t('сменить версию')}</small>
          </div>
          <span className="hint">▾</span>
          <select
            value={inst.mcVersion}
            onChange={(e) => {
              const id = e.target.value;
              const v = s.versions.find((x) => x.id === id);
              const old = v?.kind === 'old_alpha' || v?.kind === 'old_beta';
              saveInstance({
                ...inst,
                mcVersion: id,
                loaderVersion: null,
                ...(old ? { javaMajor: 8 } : {}),
              });
            }}
          >
            {!s.versions.some((v) => v.id === inst.mcVersion) && (
              <option value={inst.mcVersion}>{inst.mcVersion}</option>
            )}
            {VERSION_GROUPS.map(([kind, label]) => {
              const list = s.versions.filter((v) => v.kind === kind);
              if (list.length === 0) return null;
              return (
                <optgroup key={kind} label={t(label)}>
                  {list.map((v) => (
                    <option key={v.id} value={v.id}>
                      {v.id}
                    </option>
                  ))}
                </optgroup>
              );
            })}
          </select>
        </div>
        <button className={'play' + (live ? ' run' : '')} data-k="play" onClick={() => void playInstance(inst.id, null)}>
          {live ? (
            <span>{t('Остановить')}</span>
          ) : pct !== undefined ? (
            <>
              <span className="pf" style={{ width: pct + '%' }} />
              <span>
                {stat(pct)} {Math.round(pct)}%
              </span>
            </>
          ) : (
            <>
              <PlayIcon />
              <span>{t('Играть')}</span>
            </>
          )}
        </button>
        <div className="hint" style={{ textAlign: 'right' }}>
          {t('ОЗУ до {x} · Java {y}', { x: gb(inst.ramMaxMb), y: inst.javaMajor })}
          <br />
          <a
            style={{ color: 'var(--pri)', cursor: 'pointer', fontWeight: 500 }}
            onClick={() => useStore.getState().set({ tab: 'cfg', sec: 'j' })}
          >
            {t('Изменить')}
          </a>
          {' · '}
          <a
            style={{ color: 'var(--pri)', cursor: 'pointer', fontWeight: 500 }}
            onClick={() => setCustomOpen(true)}
          >
            {t('Своя версия')}
          </a>
        </div>
      </div>
      <LanShare id={inst.id} live={live} />
      <div className="shelf">
        <div className="tile t1" style={{ '--i': 0 } as CSSProperties}>
          <div
            style={{ width: 150, height: 94, borderRadius: 'var(--r)', overflow: 'hidden', flex: 'none' }}
            dangerouslySetInnerHTML={{ __html: worldArt }}
          />
          <div style={{ minWidth: 0 }}>
            <div className="lb">{t('Последний мир')}</div>
            {lastWorld ? (
              <>
                <b style={{ fontSize: 16, fontWeight: 600 }}>{lastWorld.name}</b>
                <div className="hint">
                  {t('Режим {mode} · {time}', {
                    mode: lastWorld.mode.toLowerCase(),
                    time: fmtRelative(Math.floor(lastWorld.lastPlayedMs / 1000)).toLowerCase(),
                  })}
                </div>
                <button
                  className="btn sm"
                  style={{ marginTop: 12 }}
                  onClick={() => void playInstance(inst.id, lastWorld.dir)}
                >
                  {t('Играть в мир')}
                </button>
              </>
            ) : (
              <>
                <b style={{ fontSize: 16, fontWeight: 600 }}>{t('Миров нет')}</b>
                <div className="hint">{t('Создайте мир в игре — он появится здесь')}</div>
              </>
            )}
          </div>
        </div>
        {pinned.length > 0 && (
          <div className="tile t2" style={{ flexDirection: 'column', '--i': 1 } as CSSProperties}>
            <div className="lb">{t('Быстрый вход на сервер')}</div>
            <div style={{ flex: 1, overflowY: 'auto', marginTop: 6 }}>
              {pinned.slice(0, 4).map((sv) => (
                <button
                  key={sv.ip}
                  className="btn sm"
                  style={{ display: 'block', width: '100%', marginTop: 6, textAlign: 'left', overflow: 'hidden', textOverflow: 'ellipsis' }}
                  title={sv.ip}
                  onClick={() => void playInstance(inst.id, null, sv.ip)}
                >
                  {sv.name}
                </button>
              ))}
            </div>
          </div>
        )}
        <div className="tile t2" style={{ flexDirection: 'column', '--i': 1 } as CSSProperties}>
          <div className="lb">{t('Контент')}</div>
          <div className="big">{contentCount}</div>
          <div className="hint" style={{ flex: 1, marginTop: 4 }}>
            {t('модов, ресурспаков и шейдеров')}
          </div>
          <div className="row g8">
            <button className="btn sm" onClick={() => useStore.getState().set({ tab: 'mods' })}>
              {t('Открыть')}
            </button>
          </div>
        </div>
        <div className="tile t3" style={{ flexDirection: 'column', '--i': 2 } as CSSProperties}>
          <div className="lb">{t('В игре')}</div>
          <div className="big">{fmtHours(inst.playtimeSec)}</div>
          <div className="hint">{t('Запуск: {time}', { time: fmtRelative(inst.lastPlayed).toLowerCase() })}</div>
          <div className="bars">
            {bars.map((b, n) => (
              <i key={n} style={{ height: b + 'px', '--i': n } as CSSProperties} />
            ))}
          </div>
        </div>
      </div>
      <CustomVersionModal
        open={customOpen}
        versions={s.versions}
        onClose={() => setCustomOpen(false)}
      />
    </>
  );
});
