// Пресеты производительности (опц. A): подбор RAM по числу модов.

/** Рекомендуемый максимум RAM профиля, МБ.
 *
 * Эвристика: 2 ГБ базе + ~48 МБ на включённый мод, с запасом 2 ГБ системе,
 * потолок 12 ГБ и округлением к 512 МБ. Мир и шейдеры едят из базы.
 */
export function recommendedRamMb(enabledMods: number, systemRamMb: number): number {
  const base = 2048;
  const perMod = 48;
  const systemReserve = 2048;
  const hardCap = 12288;
  const byMods = base + Math.max(0, enabledMods) * perMod;
  const cap = Math.min(hardCap, Math.max(2048, systemRamMb - systemReserve));
  const raw = Math.min(byMods, cap);
  return Math.max(512, Math.round(raw / 512) * 512);
}
