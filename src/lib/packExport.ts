// Экспорт сборки в один из трёх форматов. Общий код для «Сборки» профиля
// и меню «Экспорт…» на экране «Установки».
import { save } from '@tauri-apps/plugin-dialog';
import { api, type Instance, type PackExport } from './api';

export type PackFormat = 'mrpack' | 'zip' | 'mmc';

const EXT: Record<PackFormat, string> = { mrpack: 'mrpack', zip: 'zip', mmc: 'zip' };

/** Имя файла по умолчанию: без символов, запрещённых в путях. */
function fileName(name: string, version: string, format: PackFormat): string {
  const base =
    name
      .replace(/[\\/:*?"<>|]+/g, '_')
      .trim()
      .slice(0, 60) || 'pack';
  const suffix = format === 'mmc' ? '-mmc' : '';
  return `${base}-${version || '1.0.0'}${suffix}.${EXT[format]}`;
}

/** Спрашивает путь и экспортирует. `null` — пользователь отменил. */
export async function exportPackAs(
  inst: Instance,
  format: PackFormat,
  opts: { name?: string; version?: string; dir?: string } = {},
): Promise<PackExport | null> {
  const file = fileName(opts.name || inst.name, opts.version || '1.0.0', format);
  const dir = opts.dir?.replace(/[\\/]+$/, '');
  // В Windows данные лежат на диске C:, на Unix — с прямыми слэшами.
  const sep = dir ? (dir.includes('\\') ? '\\' : '/') : '';
  const dest = await save({
    defaultPath: `${dir ?? ''}${sep}${file}`,
    filters: [
      {
        name: format === 'mrpack' ? 'Modrinth pack' : 'ZIP',
        extensions: [EXT[format]],
      },
    ],
  });
  if (!dest) return null;
  return api.exportPack(inst.id, dest, format);
}
