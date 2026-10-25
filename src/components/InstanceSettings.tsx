import { useEffect, useState, type ReactNode } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { api, type Instance, type Loader } from '../lib/api';
import { pi } from '../lib/art';
import { gb } from '../lib/format';
import {
  JAVA_MAJORS,
  JVM_PRESETS,
  LOADERS,
  LOADER_LABEL,
  WINDOW_PRESETS,
} from '../lib/labels';
import { BUILTIN_ICONS } from '../lib/builtin';
import { recommendedRamMb } from '../lib/perf';
import { applyInstance, askConfirm, cancelSave, curAccount, iconUrl, saveInstance, useArtTheme, useStore, type Sec } from '../store';
import { useT } from '../lib/i18n';
import { VERSION_GROUPS } from '../lib/versions';
import { Avatar, IUp, RamSlider, Seg, Toggle } from './Controls';
import { ContentTab } from './ContentTab';
import { ConsoleTab } from './ConsoleTab';
import { PackTab } from './PackTab';
import { ScannerTab } from './ScannerTab';
import { ScreenshotsTab } from './ScreenshotsTab';
import { BenchTab } from './BenchTab';
import { SyncTab } from './SyncTab';
import { UpdatesTab } from './UpdatesTab';
import { WorldsTab } from './WorldsTab';

const SECS: [Sec, string][] = [
  ['g', 'Общие'],
  ['j', 'Java и память'],
  ['w', 'Окно и запуск'],
  ['c', 'Контент'],
  ['p', 'Сборка'],
  ['u', 'Обновления'],
  ['m', 'Миры'],
  ['v', 'Консоль'],
  ['s', 'Скриншоты'],
  ['sc', 'Безопасность'],
  ['sy', 'Синхронизация'],
  ['bm', 'Бенчмарк'],
];

export function InstanceSettings({ inst }: { inst: Instance }) {
  const s = useStore();
  const t = useT();
  const theme = useArtTheme();
  const acc = curAccount(s);
  const idx = Math.max(0, s.instances.indexOf(inst));
  const maxRam = Math.max(2048, Math.floor(s.systemRamMb / 512) * 512);
  const upd = (patch: Partial<Instance>) => saveInstance({ ...inst, ...patch });
  /** Мод, конфиги которого нужно показать на вкладке «Сборка». */
  const [cfgMod, setCfgMod] = useState<string | null>(null);

  const updNum = (field: 'width' | 'height', raw: string) => {
    const n = parseInt(raw, 10);
    if (Number.isNaN(n)) return;
    upd({ [field]: Math.max(1, n) } as Partial<Instance>);
  };

  // Версии выбранного загрузчика (для Vanilla список пустой).
  useEffect(() => {
    if (inst.loader === 'vanilla') {
      useStore.getState().set({ loaderVersions: [] });
      return;
    }
    let alive = true;
    api
      .listLoaderVersions(inst.loader, inst.mcVersion)
      .then((list) => {
        if (alive) useStore.getState().set({ loaderVersions: list });
      })
      .catch((e) => {
        if (!alive) return;
        useStore.getState().set({ loaderVersions: [] });
        useStore.getState().toastMsg(String(e));
      });
    return () => {
      alive = false;
    };
  }, [inst.loader, inst.mcVersion]);

  const pickIcon = async () => {
    try {
      const p = await open({
        multiple: false,
        directory: false,
        filters: [{ name: t('Иконка'), extensions: ['png', 'jpg', 'jpeg', 'ico'] }],
      });
      if (typeof p !== 'string') return;
      const next = await api.setInstanceIcon(inst.id, p);
      applyInstance(next);
      saveInstance({ ...next, iconPreset: null });
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  const clearIcon = async () => {
    try {
      applyInstance(await api.clearInstanceIcon(inst.id));
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  /** Путь к Java вручную: выбор файла вместо автопоиска. */
  const pickJava = async () => {
    try {
      const p = await open({ multiple: false, directory: false });
      if (typeof p !== 'string') return;
      saveInstance({ ...inst, javaPath: p });
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  /** Своя папка игры: существующий .minecraft или пустая папка. */
  const pickGameDir = async () => {
    try {
      const p = await open({ multiple: false, directory: true });
      if (typeof p !== 'string') return;
      applyInstance(await api.setGameDir(inst.id, p));
      s.toastMsg(t('Папка игры изменена'));
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  const resetGameDir = async () => {
    try {
      applyInstance(await api.setGameDir(inst.id, null));
      s.toastMsg(t('Папка игры снова в профиле'));
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  const remove = async () => {
    const ok = await askConfirm({
      title: t('Удалить профиль?'),
      text: t('Профиль «{name}» и его папка с модами, конфигами и мирами будут удалены безвозвратно.', { name: inst.name }),
      yes: t('Удалить'),
      danger: true,
    });
    if (!ok) return;
    cancelSave(inst.id);
    try {
      await api.deleteInstance(inst.id);
      const rest = s.instances.filter((x) => x.id !== inst.id);
      s.set({ instances: rest, sel: rest[0]?.id ?? null, view: 'home', tab: 'play', sec: 'g' });
    } catch (e) {
      s.toastMsg(String(e));
    }
  };

  let body: ReactNode = null;

  // Текущая версия может отсутствовать в списке (например, пришла из сборки).
  const lvOpts =
    inst.loaderVersion && !s.loaderVersions.includes(inst.loaderVersion)
      ? [inst.loaderVersion, ...s.loaderVersions]
      : s.loaderVersions;

  if (s.sec === 'g') {
    body = (
      <>
        <label className="l">{t('Название')}</label>
        <input type="text" value={inst.name} onChange={(e) => upd({ name: e.target.value })} />

        <label className="l">{t('Загрузчик')}</label>
        <Seg
          items={LOADERS.map((l) => [l, LOADER_LABEL[l]] as [Loader, string])}
          value={inst.loader}
          onPick={(l) => upd({ loader: l, loaderVersion: null })}
        />

        {inst.loader !== 'vanilla' && (
          <>
            <label className="l">{t('Версия загрузчика')}</label>
            <select
              value={inst.loaderVersion ?? ''}
              onChange={(e) => upd({ loaderVersion: e.target.value || null })}
            >
              <option value="">
                {s.loaderVersions[0]
                  ? t('Автоматически ({v})', { v: s.loaderVersions[0] })
                  : t('Автоматически')}
              </option>
              {lvOpts.map((v) => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </select>
          </>
        )}

        <label className="l">{t('Версия игры')}</label>
        <select
          value={inst.mcVersion}
          onChange={(e) => {
            const id = e.target.value;
            const v = s.versions.find((x) => x.id === id);
            const old = v?.kind === 'old_alpha' || v?.kind === 'old_beta';
            upd({ mcVersion: id, loaderVersion: null, ...(old ? { javaMajor: 8 } : {}) });
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

        <label className="l">{t('Иконка профиля')}</label>
        <div className="row">
          {inst.icon || inst.iconPreset ? (
            pi({ biome: inst.biome, icon: iconUrl(s.dataRoot, inst.id, inst.icon, s.iconBust[inst.id]), preset: inst.iconPreset }, 64, idx, theme)
          ) : (
            <Avatar name={acc?.name ?? '?'} skin={acc ? s.skins[acc.id] : null} size={64} />
          )}
          <div>
            <button className="btn sm" onClick={() => void pickIcon()}>
              <IUp />
              {t('Загрузить файл')}
            </button>{' '}
            <button className="btn sm" onClick={() => void clearIcon()}>
              {t('Сбросить')}
            </button>
            <div className="hint" style={{ marginTop: 5 }}>
              .png, .jpg, .ico
            </div>
          </div>
        </div>

        <div className="presets">
          {BUILTIN_ICONS.map((ic) => (
            <button
              key={ic.id}
              type="button"
              className={'preset ic' + (inst.iconPreset === ic.id ? ' on' : '')}
              onClick={() => upd({ iconPreset: ic.id })}
              title={ic.id}
            >
              <img src={ic.url} alt="" />
            </button>
          ))}
        </div>

        {s.config.showPlaytimeStats && (
          <>
            <label className="l" style={{ marginTop: 18 }}>
              {t('Время игры')}
            </label>
            <div className="panel" style={{ padding: 14 }}>
              <b style={{ fontSize: 18, fontWeight: 600 }}>
                {t('{h} ч всего', {
                  h: (inst.playtimeSec / 3600).toFixed(1),
                })}
              </b>
              <div className="row" style={{ alignItems: 'flex-end', gap: 3, height: 44, marginTop: 10 }}>
                {(() => {
                  const days = Object.entries(inst.playtimeDays)
                    .map(([d, sec]) => ({ d, sec }))
                    .sort((a, b) => (a.d < b.d ? -1 : 1))
                    .slice(-28);
                  const max = Math.max(1, ...days.map((x) => x.sec));
                  return days.map((x) => (
                    <div
                      key={x.d}
                      title={`${x.d}: ${(x.sec / 3600).toFixed(1)} ч`}
                      style={{
                        flex: 1,
                        minWidth: 4,
                        height: `${Math.max(6, (x.sec / max) * 100)}%`,
                        background: 'var(--pri)',
                        borderRadius: 2,
                        opacity: 0.85,
                      }}
                    />
                  ));
                })()}
              </div>
              <div className="hint" style={{ marginTop: 6 }}>
                {t('Последние 28 дней игры')}
              </div>
            </div>
          </>
        )}

        <label className="l" style={{ marginTop: 18 }}>{t('Изолированная папка')}</label>
        <div className="panel" style={{ padding: 14 }}>
          <div className="hint" style={{ fontFamily: 'ui-monospace,monospace' }}>
            instances/{inst.id}/minecraft/
          </div>
          <div className="row g8" style={{ marginTop: 8, flexWrap: 'wrap' }}>
            {['mods', 'config', 'saves', 'resourcepacks', 'shaderpacks'].map((f) => (
              <span className="lab" key={f}>
                {f}/
              </span>
            ))}
          </div>
          <div className="hint" style={{ marginTop: 8 }}>
            {t('Моды, конфиги и миры не пересекаются с другими профилями')}
          </div>
        </div>

        <label className="l">{t('Папка игры')}</label>
        <div className="panel" style={{ padding: 14 }}>
          <div className="hint" style={{ fontFamily: 'ui-monospace,monospace', wordBreak: 'break-all' }}>
            {inst.gameDir || `instances/${inst.id}/minecraft/`}
          </div>
          <div className="row g8" style={{ marginTop: 8, flexWrap: 'wrap' }}>
            <button className="btn sm" onClick={() => void pickGameDir()}>
              {t('Выбрать папку')}
            </button>
            {inst.gameDir && (
              <button className="btn sm" onClick={() => void resetGameDir()}>
                {t('Вернуть папку профиля')}
              </button>
            )}
          </div>
          <div className="hint" style={{ marginTop: 8 }}>
            {t('Моды, миры, скриншоты и конфиги хранятся в этой папке. Подойдёт и существующий .minecraft — его содержимое останется на месте.')}
          </div>
        </div>

        <div style={{ marginTop: 18 }}>
          <button className="btn" onClick={() => void remove()}>
            {t('Удалить профиль')}
          </button>
        </div>
      </>
    );
  } else if (s.sec === 'j') {
    body = (
      <>
        <label className="l">{t('Оперативная память')}</label>
        <div className="panel" style={{ padding: 18 }}>
          <div className="row" style={{ justifyContent: 'space-between' }}>
            <span>{t('Минимум')}</span>
            <b>{gb(inst.ramMinMb)}</b>
          </div>
          <RamSlider
            value={inst.ramMinMb}
            min={512}
            max={maxRam}
            onChange={(v) => upd({ ramMinMb: v, ramMaxMb: Math.max(v, inst.ramMaxMb) })}
          />
          <div className="row" style={{ justifyContent: 'space-between', marginTop: 10 }}>
            <span>{t('Максимум')}</span>
            <b>{gb(inst.ramMaxMb)}</b>
          </div>
          <RamSlider
            value={inst.ramMaxMb}
            min={512}
            max={maxRam}
            onChange={(v) => upd({ ramMaxMb: v, ramMinMb: Math.min(v, inst.ramMinMb) })}
          />
          <div className="hint" style={{ marginTop: 4 }}>
            {t('В системе доступно {x}. Для сборок обычно хватает 4–8 ГБ.', { x: gb(s.systemRamMb) })}
          </div>
          {s.systemRamMb > 0 && inst.ramMaxMb + 1024 > s.systemRamMb && (
            <div className="hint" style={{ marginTop: 4, color: 'var(--err,#d9503f)' }}>
              {t('Столько ОЗУ система не отдаст: оставьте ей хотя бы 1 ГБ, иначе игра вылетит при запуске.')}
            </div>
          )}
          {inst.ramMaxMb < 2048 && (
            <div className="hint" style={{ marginTop: 4, color: '#e0a13f' }}>
              {t('Для сборок с модами меньше 2 ГБ почти всегда мало — начнутся лаги и вылеты.')}
            </div>
          )}
          <div className="row" style={{ justifyContent: 'flex-end', marginTop: 8 }}>
            <button
              className="btn sm"
              onClick={() => {
                const mods = (s.sel === inst.id ? s.content : []).filter((c) => c.kind === 'mod' && c.enabled).length;
                const target = recommendedRamMb(mods, s.systemRamMb);
                upd({ ramMaxMb: target, ramMinMb: Math.min(inst.ramMinMb, Math.max(512, target / 4)) });
              }}
            >
              {t('Подобрать по модам')}
            </button>
          </div>
        </div>

        <div className="two">
          <div>
            <label className="l">{t('Версия Java')}</label>
            <Seg
              items={JAVA_MAJORS.map((j) => [String(j), String(j)] as [string, string])}
              value={String(inst.javaMajor)}
              onPick={(j) => upd({ javaMajor: Number(j) })}
            />
          </div>
          <div>
            <label className="l">{t('Путь к Java')}</label>
            <input
              type="text"
              placeholder={t('Автоопределение')}
              value={inst.javaPath ?? ''}
              onChange={(e) => upd({ javaPath: e.target.value || null })}
            />
            <div className="row g8" style={{ marginTop: 8 }}>
              <button className="btn sm" onClick={() => void pickJava()}>
                {t('Выбрать файл')}
              </button>
              {inst.javaPath && (
                <button className="btn sm" onClick={() => upd({ javaPath: null })}>
                  {t('Сбросить')}
                </button>
              )}
            </div>
            <div className="hint" style={{ marginTop: 6 }}>
              {t('Можно указать java/java.exe вручную — автопоиск тогда не используется.')}
            </div>
          </div>
        </div>

        <label className="l">{t('JVM-аргументы')}</label>
        <textarea
          value={inst.jvmArgs}
          placeholder="-Xss1M -XX:+UseG1GC"
          onChange={(e) => upd({ jvmArgs: e.target.value })}
        />
        <div className="row g8" style={{ marginTop: 8 }}>
          {JVM_PRESETS.map(([n, v]) => (
            <button key={n} className="btn sm" onClick={() => upd({ jvmArgs: v })}>
              {n}
            </button>
          ))}
        </div>
      </>
    );
  } else if (s.sec === 'w') {
    body = (
      <>
        <div className="sw">
          <div>
            <b style={{ fontWeight: 500 }}>{t('Полноэкранный режим')}</b>
            <div className="hint">{t('Запускать игру на весь экран')}</div>
          </div>
          <Toggle on={inst.fullscreen} onClick={() => upd({ fullscreen: !inst.fullscreen })} />
        </div>
        <div className="two" style={inst.fullscreen ? { opacity: 0.4, pointerEvents: 'none' } : undefined}>
          <div>
            <label className="l">{t('Ширина')}</label>
            <input type="text" value={inst.width} onChange={(e) => updNum('width', e.target.value)} />
          </div>
          <div>
            <label className="l">{t('Высота')}</label>
            <input type="text" value={inst.height} onChange={(e) => updNum('height', e.target.value)} />
          </div>
        </div>
        <div className="row g8" style={{ marginTop: 12 }}>
          {WINDOW_PRESETS.map(([w, h]) => (
            <button key={w} className="btn sm" onClick={() => upd({ width: w, height: h })}>
              {w}×{h}
            </button>
          ))}
        </div>
      </>
    );
  } else if (s.sec === 'c') {
    body = (
      <ContentTab
        inst={inst}
        onOpenConfigs={(name) => {
          setCfgMod(name);
          s.set({ sec: 'p' });
        }}
      />
    );
  } else if (s.sec === 'p') {
    body = <PackTab inst={inst} initialMod={cfgMod} />;
  } else if (s.sec === 'u') {
    body = <UpdatesTab inst={inst} />;
  } else if (s.sec === 'm') {
    body = <WorldsTab inst={inst} />;
  } else if (s.sec === 'v') {
    body = <ConsoleTab inst={inst} />;
  } else if (s.sec === 'sc') {
    body = <ScannerTab inst={inst} />;

  } else if (s.sec === 'sy') {
    body = <SyncTab inst={inst} />;
  } else if (s.sec === 'bm') {
    body = <BenchTab inst={inst} />;
  } else {
    body = <ScreenshotsTab inst={inst} />;
  }

  return (
    <div className="row" style={{ alignItems: 'flex-start', gap: 28 }}>
      <div className="side panel">
        {SECS.map(([key, label]) => (
          <div
            key={key}
            className={'sd' + (s.sec === key ? ' on' : '')}
            onClick={() => {
              // Вкладка «Сборка» из меню открывается без фильтра по моду.
              setCfgMod(null);
              s.set({ sec: key });
            }}
          >
            {t(label)}
          </div>
        ))}
      </div>
      <div style={{ flex: 1, minWidth: 0, maxWidth: ['c', 'p', 'm', 's', 'v', 'sc', 'sy', 'bm'].includes(s.sec) ? 'none' : 640 }}>{body}</div>
    </div>
  );
}
