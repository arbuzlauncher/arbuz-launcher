// Направление интерфейса: арабский, персидский и иврит пишутся справа налево.
// Флаг ставится на <html>, поэтому вся раскладка (сетка сайдбара, меню,
// отступы) разворачивается без отдельных классов на компонентах.
import { RTL_LANGS, normLang, type Lang } from './i18n';

/** Язык пишется справа налево? */
export function isRtl(lang: Lang | string): boolean {
  return RTL_LANGS.includes(normLang(lang));
}

/** Применяет язык и направление к документу. */
export function applyDirection(lang: Lang | string): void {
  const l = normLang(lang);
  const root = document.documentElement;
  root.lang = l;
  root.dir = isRtl(l) ? 'rtl' : 'ltr';
}
