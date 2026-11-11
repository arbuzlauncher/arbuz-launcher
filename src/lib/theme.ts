import { monet, ink, mix } from './color';
export type Style = 'flat' | 'monet' | 'amoled' | 'catppuccin' | 'dracula' | 'custom';
export type Roles = Record<string, string>;

/** Полностью своя тема: цвета, размеры текста/кнопок и ширина сайдбара. */
export interface CustomTheme {
  bg: string;
  sbg: string;
  s1: string;
  s2: string;
  s3: string;
  on: string;
  onv: string;
  ln: string;
  pri: string;
  sec: string;
  ter: string;
  font: string; // пусто — стандартный шрифт темы
  fontScale: number; // 0.85..1.3
  radius: number; // 0..26 px
  control: number; // 0.8..1.3 — высота и отступы кнопок
  sidebar: number; // 190..320 px
  density: number; // 0.85..1.3 — отступы экранов
  soft: boolean; // мягкие формы и скругления
}

export const CUSTOM_COLORS = [
  'bg',
  'sbg',
  's1',
  's2',
  's3',
  'on',
  'onv',
  'ln',
  'pri',
  'sec',
  'ter',
] as const;

export type CustomColor = (typeof CUSTOM_COLORS)[number];

export const DEFAULT_CUSTOM: CustomTheme = {
  bg: '#1a1b1e',
  sbg: '#141517',
  s1: '#212225',
  s2: '#2a2c30',
  s3: '#34363b',
  on: '#e8e9eb',
  onv: '#8f949c',
  ln: '#2c2e33',
  pri: '#5fb339',
  sec: '#2a2c30',
  ter: '#5fb339',
  font: '',
  fontScale: 1,
  radius: 8,
  control: 1,
  sidebar: 230,
  density: 1,
  soft: false,
};

const HEX = /^#[0-9a-f]{6}$/i;
const num = (v: unknown, lo: number, hi: number, d: number): number => {
  const n = typeof v === 'number' ? v : Number(v);
  return Number.isFinite(n) ? Math.min(hi, Math.max(lo, n)) : d;
};

/** Приводит свою тему из конфига к безопасным значениям. */
export function normCustom(v?: Partial<CustomTheme> | null): CustomTheme {
  const src = (v ?? {}) as Partial<CustomTheme>;
  const c: CustomTheme = { ...DEFAULT_CUSTOM };
  for (const k of CUSTOM_COLORS) {
    const val = src[k];
    c[k] = typeof val === 'string' && HEX.test(val) ? val.toLowerCase() : DEFAULT_CUSTOM[k];
  }
  c.font = typeof src.font === 'string'
    ? src.font.replace(/[^\p{L}\p{N}\s,'"-]/gu, '').slice(0, 80).trim()
    : '';
  c.fontScale = num(src.fontScale, 0.85, 1.3, 1);
  c.radius = num(src.radius, 0, 26, 8);
  c.control = num(src.control, 0.8, 1.3, 1);
  c.sidebar = num(src.sidebar, 190, 320, 230);
  c.density = num(src.density, 0.85, 1.3, 1);
  c.soft = !!src.soft;
  return c;
}

const FLAT_DARK = { bg: '#1a1b1e', sbg: '#141517', s1: '#212225', s2: '#2a2c30', s3: '#34363b', on: '#e8e9eb', onv: '#8f949c', ln: '#2c2e33' };
const FLAT_LIGHT = { bg: '#f6f6f7', sbg: '#ececee', s1: '#ffffff', s2: '#efeff1', s3: '#e1e1e5', on: '#1c1d20', onv: '#6b7078', ln: '#e3e3e6' };

/** AMOLED: настоящий чёрный фон, чтобы пиксели OLED выключались полностью. */
const AMOLED = { bg: '#000000', sbg: '#000000', s1: '#0b0b0d', s2: '#131316', s3: '#1c1c20', on: '#f2f3f5', onv: '#8d9096', ln: '#202024' };

export interface CatTokens {
  base: string;
  mantle: string;
  crust: string;
  surface0: string;
  surface1: string;
  surface2: string;
  overlay1: string;
  text: string;
  subtext0: string;
  mauve: string;
  blue: string;
  pink: string;
  teal: string;
  yellow: string;
  green: string;
}

/** Официальные токены Catppuccin (github.com/catppuccin/palette). */
const MOCHA: CatTokens = {
  base: '#1e1e2e', mantle: '#181825', crust: '#11111b',
  surface0: '#313244', surface1: '#45475a', surface2: '#585b70',
  overlay1: '#7f849c', text: '#cdd6f4', subtext0: '#a6adc8',
  mauve: '#cba6f7', blue: '#89b4fa', pink: '#f5c2e7', teal: '#94e2d5', yellow: '#f9e2af', green: '#a6e3a1',
};

const LATTE: CatTokens = {
  base: '#eff1f5', mantle: '#e6e9ef', crust: '#dce0e8',
  surface0: '#ccd0da', surface1: '#bcc0cc', surface2: '#acb0be',
  overlay1: '#8c8fa1', text: '#4c4f69', subtext0: '#6c6f85',
  mauve: '#8839ef', blue: '#1e66f5', pink: '#ea76cb', teal: '#179299', yellow: '#df8e1d', green: '#40a02b',
};

export const catTokens = (dark: boolean): CatTokens => (dark ? MOCHA : LATTE);

/** Официальные токены Dracula (draculatheme.com/about). */
export interface DraculaTokens {
  background: string;
  currentLine: string;
  foreground: string;
  comment: string;
  cyan: string;
  green: string;
  orange: string;
  pink: string;
  purple: string;
  red: string;
  yellow: string;
}

const DRACULA: DraculaTokens = {
  background: '#282a36', currentLine: '#44475a', foreground: '#f8f8f2', comment: '#6272a4',
  cyan: '#8be9fd', green: '#50fa7b', orange: '#ffb86c', pink: '#ff79c6',
  purple: '#bd93f9', red: '#ff5555', yellow: '#f1fa8c',
};

/** Alucard — официальная светлая версия Dracula. */
const ALUCARD: DraculaTokens = {
  background: '#fffbeb', currentLine: '#cfcfde', foreground: '#1f1f1f', comment: '#6c664b',
  cyan: '#036a96', green: '#14710a', orange: '#a34d14', pink: '#a3144d',
  purple: '#644ac9', red: '#cb3a2a', yellow: '#846e15',
};

export const draculaTokens = (dark: boolean): DraculaTokens => (dark ? DRACULA : ALUCARD);

/** Monet, Catppuccin и Dracula — «мягкий» вариант: скругления и другая типографика. */
export const isSoft = (style: Style): boolean =>
  style === 'monet' || style === 'catppuccin' || style === 'dracula';

/** Monet-подобные темы: та же вёрстка и Material-иконки. */
export const monetLike = (style: string): boolean =>
  style === 'monet' || style === 'catppuccin' || style === 'dracula';

/** Мягкая ли тема с учётом своей (у неё есть отдельный переключатель). */
export const softOf = (style: Style, custom?: CustomTheme | null): boolean =>
  style === 'custom' ? !!custom?.soft : isSoft(style);

/** Приводит строку из конфига к известной теме. */
export const normStyle = (v: string): Style =>
  v === 'monet' || v === 'amoled' || v === 'catppuccin' || v === 'dracula' || v === 'custom'
    ? v
    : 'flat';

/** Плоские темы: базовая палитра + акцент пользователя. */
function plain(base: Roles, accent: string): Roles {
  const c: Roles = { ...base };
  c.pri = accent; c.onpri = ink(accent); c.sec = c.s2; c.onsec = c.on;
  c.pric = c.s2; c.onpric = c.on; c.ter = accent; c.terc = c.s1; c.onterc = c.on; c.outl = c.onv;
  return c;
}

/** Catppuccin: фиксированная палитра Mocha/Latte, акцент из настроек не применяется. */
function catppuccin(dark: boolean): Roles {
  const c = catTokens(dark);
  if (dark) {
    return {
      bg: c.base, sbg: c.mantle, s1: c.surface0, s2: c.surface1, s3: c.surface2,
      on: c.text, onv: c.subtext0, ln: c.surface1, outl: c.overlay1,
      pri: c.mauve, onpri: c.crust,
      sec: c.surface1, onsec: c.text,
      pric: '#494060', onpric: c.text,
      ter: c.teal, terc: '#384953', onterc: c.text,
    };
  }
  return {
    bg: c.base, sbg: c.mantle, s1: c.surface0, s2: c.surface1, s3: c.surface2,
    on: c.text, onv: c.subtext0, ln: c.surface1, outl: c.overlay1,
    pri: c.mauve, onpri: c.base,
    sec: c.surface0, onsec: c.text,
    pric: '#d8c8f4', onpric: c.text,
    ter: c.teal, terc: '#c8e0e4', onterc: c.text,
  };
}

/** Dracula: официальная палитра Dracula / Alucard, акцент из настроек не применяется. */
function dracula(dark: boolean): Roles {
  if (dark) {
    const c = DRACULA;
    return {
      bg: c.background, sbg: '#21222c', s1: '#343746', s2: c.currentLine, s3: '#565970',
      // Прозрачный комментарий слишком тусклый для подсказок — подмешиваем светлый.
      on: c.foreground, onv: mix(c.comment, c.foreground, 0.35),
      ln: c.currentLine, outl: c.comment,
      pri: c.purple, onpri: c.background,
      sec: c.currentLine, onsec: c.foreground,
      pric: mix(c.purple, '#343746', 0.55), onpric: c.foreground,
      ter: c.cyan, terc: mix(c.cyan, '#343746', 0.62), onterc: c.foreground,
    };
  }
  const c = ALUCARD;
  return {
    bg: c.background, sbg: '#f7f2e0', s1: '#ffffff', s2: '#ece7d5', s3: '#dedac8',
    on: c.foreground, onv: c.comment, ln: '#dedac8', outl: '#b3ab93',
    pri: c.purple, onpri: '#fffbeb',
    sec: '#ece7d5', onsec: c.foreground,
    pric: mix(c.purple, '#ffffff', 0.62), onpric: c.foreground,
    ter: c.cyan, terc: mix(c.cyan, '#ffffff', 0.72), onterc: c.foreground,
  };
}

/** Своя тема: роли собираются из цветов пользователя. */
function customRoles(c: CustomTheme): Roles {
  const pri = c.pri;
  const ter = c.ter;
  return {
    bg: c.bg,
    sbg: c.sbg,
    s1: c.s1,
    s2: c.s2,
    s3: c.s3,
    on: c.on,
    onv: c.onv,
    ln: c.ln,
    outl: c.onv,
    pri,
    onpri: ink(pri),
    sec: c.sec,
    onsec: ink(c.sec),
    // Мягкий вариант использует приглушённые заливки, плоский — чистый акцент.
    pric: c.soft ? mix(pri, c.s1, 0.45) : pri,
    onpric: c.soft ? c.on : ink(pri),
    ter,
    terc: c.soft ? mix(ter, c.s1, 0.55) : ter,
    onterc: c.soft ? c.on : ink(ter),
  };
}

export function getRoles(
  style: Style,
  dark: boolean,
  accent: string,
  custom?: CustomTheme | null,
): Roles {
  if (style === 'custom') return customRoles(normCustom(custom));
  if (style === 'monet') return monet(dark, accent);
  if (style === 'catppuccin') return catppuccin(dark);
  if (style === 'dracula') return dracula(dark);
  if (style === 'amoled') return plain(AMOLED, accent);
  return plain(dark ? FLAT_DARK : FLAT_LIGHT, accent);
}

export function applyTheme(
  el: HTMLElement,
  style: Style,
  dark: boolean,
  accent: string,
  custom?: CustomTheme | null,
) {
  const r = getRoles(style, dark, accent, custom);
  for (const k in r) el.style.setProperty('--' + k, r[k]);
  el.dataset.v = softOf(style, custom) ? 'b' : 'a';
  // Стиль темы отдельным атрибутом: Monet и классика отличаются не только цветом.
  el.dataset.style = style === 'catppuccin' || style === 'dracula' ? 'monet' : style;
  // Нативные поля, datalist и скроллбары должны следовать тёмной/светлой теме,
  // иначе WebView2 рисует их светлыми независимо от палитры.
  el.style.colorScheme = dark ? 'dark' : 'light';
  if (style === 'custom') {
    const c = normCustom(custom);
    el.dataset.custom = '1';
    el.style.setProperty('--r', `${Math.round(c.radius)}px`);
    el.style.setProperty('--rb', `${Math.max(2, Math.round(c.radius * 0.78))}px`);
    el.style.setProperty('--rp', `${Math.round(c.radius + 2)}px`);
    el.style.setProperty('--sbw', `${Math.round(c.sidebar)}px`);
    el.style.setProperty('--fs', String(c.fontScale));
    el.style.setProperty('--ctl', String(c.control));
    el.style.setProperty('--den', String(c.density));
    if (c.font) el.style.setProperty('--font', c.font);
    else el.style.removeProperty('--font');
  } else {
    delete el.dataset.custom;
    for (const k of ['--r', '--rb', '--rp', '--sbw', '--fs', '--ctl', '--den', '--font']) {
      el.style.removeProperty(k);
    }
  }
}
