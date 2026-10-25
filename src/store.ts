import { useMemo } from 'react';
import { create } from 'zustand';
import { convertFileSrc } from '@tauri-apps/api/core';
import { api, type Account, type CatalogItem, type Config, type ContentItem, type Instance, type JavaInstall, type LauncherUpdate, type McVersion, type NewsItem, type Skin, type World } from './lib/api';
import type { ArtTheme } from './lib/art';
import { DEFAULT_CUSTOM, normStyle } from './lib/theme';
import { cachedAnim } from './lib/anim';

export type View = 'home' | 'installs' | 'accounts' | 'p' | 'app' | 'browse' | 'coop' | 'servers';
export type Tab = 'play' | 'mods' | 'worlds' | 'shots' | 'cfg';
export type Sec = 'g' | 'j' | 'w' | 'c' | 'p' | 'm' | 's' | 'u' | 'v' | 'sc' | 'sy' | 'bm';
export type Modal = { p: 'ms' | 'ely' | 'off' | 'lsk' | 'pick' } | null;

/** Внутрилаунчерный диалог подтверждения вместо системного window.confirm. */
export interface ConfirmReq {
  title: string;
  text?: string;
  yes?: string;
  no?: string;
  danger?: boolean;
  resolve: (ok: boolean) => void;
}

export const DEFAULT_CONFIG: Config = {
  accent: '#5fb339',
  dark: true,
  style: 'flat',
  lang: 'ru',
  experimentalThemes: false,
  homeWallpaper: null,
  onboarded: true,
  activeAccount: null,
  defaultJava: 21,
  closeOnLaunch: false,
  showConsole: true,
  checkLauncherUpdates: true,
  closeToTray: true,
  hideEmpty: false,
  modIsolationEnabled: false,
  modIsolationLevel: 'min',
  scannerEnabled: false,
  scannerMode: 'normal',
  aiScanner: false,
  serversUrl: '',
  showPlaytimeStats: true,
  sync: null,
  curseforgeApiKey: '',
  aiBuilder: true,
  aiPackLimit: 0,
  aiPackSource: '',
  aiPackModel: '',
  // До загрузки config.json берём настройки прошлого запуска из localStorage:
  // первый экран не должен дёргаться у тех, кто анимации выключил.
  anim: cachedAnim(),
  custom: { ...DEFAULT_CUSTOM },
  moves: {},
};

export interface StoreState {
  view: View;
  tab: Tab;  sel: string | null;
  sec: Sec;

  instances: Instance[];
  accounts: Account[];
  /** id аккаунта -> скин (null = скина нет, рисуется Стив). */
  skins: Record<string, Skin | null>;
  config: Config;
  javas: JavaInstall[];
  systemRamMb: number;
  dataRoot: string;

  running: Record<string, number>;
  /** id -> игра запущена (процесс стартовал). */
  live: Record<string, boolean>;
  /** id -> строки лога (максимум 2000). */
  logs: Record<string, string[]>;
  /** id профиля, чья консоль открыта. */
  console: string | null;
  /** Профиль, игра которого упала: показываем окно с подсказкой. */
  crash: { id: string; code: number | null } | null;
  /** Список версий Minecraft из list_mc_versions. */
  versions: McVersion[];
  /** Список версий выбранного загрузчика из list_loader_versions. */
  loaderVersions: string[];
  /** Контент выбранного профиля (list_content). */
  content: ContentItem[];
  /** Миры выбранного профиля (list_worlds). */
  worlds: World[];
  /** id профиля -> число файлов контента (нужно фильтру `hide_empty`). */
  contentCounts: Record<string, number>;
  /** Новости Minecraft с «Главной». */
  news: NewsItem[];
  toast: string;
  modal: Modal;
  confirm: ConfirmReq | null;
  menu: boolean;
  /** Найденное обновление лаунчера (окно с предложением скачать). */
  appUpdate: LauncherUpdate | null;
  /** Режим расстановки: кнопки можно таскать мышью. */
  move: boolean;
  setMove: (v: boolean) => void;
  /** Счётчик обновлений иконки: сбрасывает кэш картинки в webview. */
  iconBust: Record<string, number>;

  // каталог
  src: string;
  bt: string;
  bq: string;
  bl: string[];
  bc: string[];
  tgt: string | null;
  /** Фильтр версии игры в каталоге (пусто = любая). */
  bv: string;
  /** Сортировка каталога: relevance | downloads | newest | updated. */
  bs: string;
  /** Загруженные результаты каталога и их общее число. */
  cat: CatalogItem[];
  catTotal: number;
  catBusy: boolean;
  /** Прогресс установки сборки (id = "pack"). */
  packPct: number | null;
  packName: string;

  // контент профиля
  cq: string;
  flt: string;

  /** История переходов между экранами и позиция в ней (для кнопок ← →). */
  hist: View[];
  hp: number;
  back: () => void;
  forward: () => void;

  set: (p: Partial<StoreState>) => void;
  setConfig: (cfg: Config) => void;
  toastMsg: (m: string) => void;
}

/** Таймер отложенной записи config.json (слайдеры и переключатели). */
let cfgTimer: number | undefined;
/** Очередь записей: старый снимок не должен обогнать новый. */
let cfgChain: Promise<void> = Promise.resolve();

function saveConfig(cfg: Config): Promise<void> {
  cfgChain = cfgChain.then(() =>
    api.setConfig(cfg).catch((e) => {
      useStore.getState().toastMsg(String(e));
    }),
  );
  return cfgChain;
}

export const useStore = create<StoreState>((set, get) => {
  /** Прямая запись без истории — для кнопок «назад/вперёд». */
  const apply = (p: Partial<StoreState>) => set(p);

  /** Запись с историей: смена экрана кладётся в стек (как в браузере). */
  const setSafe = (p: Partial<StoreState>) => {
    const prev = get();
    if (p.view !== undefined && p.view !== prev.view) {
      const hist = [...prev.hist.slice(0, prev.hp + 1), p.view];
      apply({ ...p, hist, hp: hist.length - 1 });
      return;
    }
    apply(p);
  };

  return {
  view: 'home',
  tab: 'play',
  sel: null,
  sec: 'g',

  instances: [],
  accounts: [],
  skins: {},
  config: DEFAULT_CONFIG,
  javas: [],
  systemRamMb: 16384,
  dataRoot: '',

  running: {},
  live: {},
  logs: {},
  console: null,
  crash: null,
  versions: [],
  loaderVersions: [],
  content: [],
  worlds: [],
  contentCounts: {},
  news: [],
  toast: '',
  modal: null,
  confirm: null,
  menu: false,
  appUpdate: null,
  move: false,
  setMove: (v) => set({ move: v }),
  iconBust: {},

  src: 'Modrinth',
  bt: 'mod',
  bq: '',
  bl: [],
  bc: [],
  tgt: null,
  bv: '',
  bs: 'relevance',
  cat: [],
  catTotal: 0,
  catBusy: false,
  packPct: null,
  packName: '',

  cq: '',
  flt: 'mod',

  hist: ['home'],
  hp: 0,
  back: () => {
    const s = get();
    if (s.hp <= 0) return;
    const hp = s.hp - 1;
    apply({ view: s.hist[hp], hp });
  },
  forward: () => {
    const s = get();
    if (s.hp >= s.hist.length - 1) return;
    const hp = s.hp + 1;
    apply({ view: s.hist[hp], hp });
  },

  set: setSafe,
  setConfig: (cfg) => {
    // Немедленное сохранение отменяет отложенное, иначе старый снимок
    // конфига может лечь на диск поверх нового.
    if (cfgTimer) {
      window.clearTimeout(cfgTimer);
      cfgTimer = undefined;
    }
    set({ config: cfg });
    void saveConfig(cfg);
  },
  toastMsg: (m) => {
    set({ toast: m });
    window.setTimeout(() => {
      if (get().toast === m) set({ toast: '' });
    }, 2200);
  },
  };
});

/** Обновляет конфиг в UI сразу, а на диск пишет с задержкой (для слайдеров). */
export function setConfigLive(cfg: Config): void {
  useStore.getState().set({ config: cfg });
  if (cfgTimer) window.clearTimeout(cfgTimer);
  cfgTimer = window.setTimeout(() => {
    cfgTimer = undefined;
    void saveConfig(useStore.getState().config);
  }, 300);
}

/** Досрочно пишет отложенный конфиг — перед закрытием окна или уходом в фон. */
export function flushConfig(): Promise<void> {
  if (!cfgTimer) return cfgChain;
  window.clearTimeout(cfgTimer);
  cfgTimer = undefined;
  return saveConfig(useStore.getState().config);
}

/** Показывает подтверждение внутри лаунчера и ждёт ответа пользователя. */
export function askConfirm(req: Omit<ConfirmReq, 'resolve'>): Promise<boolean> {
  return new Promise((resolve) => {
    const st = useStore.getState();
    st.confirm?.resolve(false);
    st.set({ confirm: { ...req, resolve } });
  });
}

/** Закрывает диалог подтверждения и отдаёт ответ. */
export function answerConfirm(ok: boolean): void {
  const st = useStore.getState();
  const req = st.confirm;
  if (!req) return;
  st.set({ confirm: null });
  req.resolve(ok);
}

/** Максимальное число хранимых строк лога на один профиль. */
const MAX_LOG_LINES = 2000;

/** Добавляет строку в лог профиля, обрезая историю до последних 2000 строк. */
export function pushLog(id: string, line: string): void {
  const st = useStore.getState();
  const prev = st.logs[id] ?? [];
  const next = prev.length >= MAX_LOG_LINES ? prev.slice(prev.length - MAX_LOG_LINES + 1) : prev.slice();
  next.push(line);
  st.set({ logs: { ...st.logs, [id]: next } });
}

/** Текущий выбранный профиль (или первый). */
export const curInstance = (s: StoreState): Instance | null =>
  s.instances.find((i) => i.id === s.sel) ?? s.instances[0] ?? null;

/** Активный аккаунт (или первый). */
export const curAccount = (s: StoreState): Account | null =>
  s.accounts.find((a) => a.id === s.config.activeAccount) ?? s.accounts[0] ?? null;

/** Тема оформления в виде, который нужен генераторам арта и иконок. */
export function useArtTheme(): ArtTheme {
  const style = useStore((s) => s.config.style);
  const dark = useStore((s) => s.config.dark);
  const accent = useStore((s) => s.config.accent);
  const custom = useStore((s) => s.config.custom);
  return useMemo<ArtTheme>(
    () => ({ style: normStyle(style), dark, accent, custom }),
    [style, dark, accent, custom],
  );
}

const instTimers = new Map<string, number>();

/** Мгновенно обновляет профиль в состоянии и сохраняет его с задержкой 300 мс. */
export function saveInstance(next: Instance): void {
  const st = useStore.getState();
  st.set({ instances: st.instances.map((i) => (i.id === next.id ? next : i)) });
  const prev = instTimers.get(next.id);
  if (prev) window.clearTimeout(prev);
  instTimers.set(
    next.id,
    window.setTimeout(() => {
      instTimers.delete(next.id);
      api.updateInstance(next).catch((e) => useStore.getState().toastMsg(String(e)));
    }, 300),
  );
}

/** Отменяет отложенную запись профиля (перед удалением профиля). */
export function cancelSave(id: string): void {
  const t = instTimers.get(id);
  if (t) {
    window.clearTimeout(t);
    instTimers.delete(id);
  }
}

/** Записывает профиль, возвращённый командой, и сбрасывает кэш его иконки. */
export function applyInstance(next: Instance): void {
  const st = useStore.getState();
  st.set({
    instances: st.instances.map((i) => (i.id === next.id ? next : i)),
    iconBust: { ...st.iconBust, [next.id]: (st.iconBust[next.id] ?? 0) + 1 },
  });
}

/** Абсолютный путь к файлу иконки профиля, пригодный для <img>. */
export function iconUrl(root: string, id: string, file: string | null, bust?: number): string | null {
  if (!file || !root) return null;
  const sep = root.includes('\\') ? '\\' : '/';
  const url = convertFileSrc(root + sep + 'instances' + sep + id + sep + file);
  return bust ? `${url}?v=${bust}` : url;
}

/** Перечитывает контент и миры профиля в состояние. */
export async function loadProfileContent(id: string): Promise<void> {
  const [content, worlds] = await Promise.all([api.listContent(id), api.listWorlds(id)]);
  const st = useStore.getState();
  st.set({ content, worlds, contentCounts: { ...st.contentCounts, [id]: content.length } });
}

/** Профили с учётом настройки «Скрывать пустые профили» (ТЗ §12-b). */
export const visibleInstances = (s: StoreState): Instance[] =>
  s.config.hideEmpty
    ? s.instances.filter((i) => (s.contentCounts[i.id] ?? 0) > 0 || i.playtimeSec > 0)
    : s.instances;

/** Считает контент всех профилей — нужно фильтру `hide_empty`. */
export async function loadContentCounts(): Promise<void> {
  const list = useStore.getState().instances;
  const pairs = await Promise.all(
    list.map(async (i): Promise<[string, number]> => {
      try {
        const items = await api.listContent(i.id);
        return [i.id, items.length];
      } catch {
        return [i.id, 0];
      }
    }),
  );
  useStore.getState().set({ contentCounts: Object.fromEntries(pairs) });
}

/** Загружает скины аккаунтов для аватаров (ответы кэширует бэкенд). */
export async function loadSkins(list: Account[]): Promise<void> {
  const pairs = await Promise.all(
    list.map(async (a): Promise<[string, Skin | null]> => {
      try {
        return [a.id, await api.accountSkin(a.id)];
      } catch {
        return [a.id, null];
      }
    }),
  );
  useStore.getState().set({ skins: Object.fromEntries(pairs) });
}
