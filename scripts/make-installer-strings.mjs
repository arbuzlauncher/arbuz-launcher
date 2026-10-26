// Генерирует installer/lang/strings.nsh: переводы строк установщика.
// Запускать вручную при добавлении новых строк или языков.
import fs from 'node:fs';
import path from 'node:path';

const OUT = path.join(process.cwd(), 'installer', 'lang', 'strings.nsh');

// Все языки лаунчера, кроме olb. Русский и английский заданы в arbuz.nsi.
const LANGS = {
  LANG_POLISH: 'pl',
  LANG_UKRAINIAN: 'uk',
  LANG_BELARUSIAN: 'be',
  LANG_SERBIAN: 'sr',
  LANG_SLOVAK: 'sk',
  LANG_ESPERANTO: 'eo',
  LANG_ESTONIAN: 'et',
  LANG_FINNISH: 'fi',
  LANG_FRENCH: 'fr',
  LANG_GERMAN: 'de',
  LANG_LATVIAN: 'lv',
  LANG_LITHUANIAN: 'lt',
  LANG_SIMPCHINESE: 'zh-CN',
  LANG_JAPANESE: 'ja',
  LANG_UZBEK: 'uz',
  LANG_GEORGIAN: 'ka',
  LANG_KAZAKH: 'kk',
  LANG_KYRGYZ: 'ky',
  LANG_TAJIK: 'tg',
  LANG_INUKTITUT: 'iu',
  LANG_FARSI: 'fa',
  LANG_ARABIC: 'ar',
  LANG_HEBREW: 'he',
  LANG_AZERBAIJANI: 'az',
  LANG_ARMENIAN: 'hy',
  LANG_PORTUGUESE: 'pt',
};

const STRINGS = [
  ['RunApp', 'Run Arbuz Launcher'],
  ['Desktop', 'Create desktop shortcut'],
  ['StartMenuShortcut', 'Create a Start Menu shortcut'],
  ['ShortcutsTitle', 'Shortcuts'],
  ['ShortcutsSubtitle', 'Choose which shortcuts to create.'],
  ['AskData', 'Also delete settings, profiles, worlds and accounts?'],
  ['Busy', 'The launcher is running. Close it and try again.'],
  [
    'AlreadyInstalled',
    'Arbuz Launcher is already installed in $ExistingDir. Yes — reinstall and keep settings, No — uninstall, Cancel — exit.',
  ],
  [
    'NeedAdmin',
    'This version was installed for all users. Run the installer as administrator to remove it.',
  ],
  [
    'NeedAdminInstall',
    'Administrator rights are required to install for all users.',
  ],
  ['MULTIUSER_TEXT_INSTALLMODE_TITLE', 'Installation mode'],
  [
    'MULTIUSER_TEXT_INSTALLMODE_SUBTITLE',
    'Choose how to install Arbuz Launcher: for all users or for a single user.',
  ],
  ['MULTIUSER_INNERTEXT_INSTALLMODE_TOP', 'Select the installation mode:'],
  [
    'MULTIUSER_INNERTEXT_INSTALLMODE_ALLUSERS',
    'Install for all users (administrator rights)',
  ],
  [
    'MULTIUSER_INNERTEXT_INSTALLMODE_CURRENTUSER',
    'Install for a single user (recommended)',
  ],
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const TOKEN = String.fromCharCode(0xe000);
const KEEP = String.fromCharCode(0xe001);

function protect(s) {
  return s.replace(/\$[A-Za-z0-9_]+/g, TOKEN);
}

async function translateBatch(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    encodeURIComponent(tl) +
    list.map((s) => '&q=' + encodeURIComponent(s)).join('');
  for (let attempt = 0; attempt < 8; attempt++) {
    try {
      const r = await fetch(url, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      });
      if (r.status === 429) {
        await sleep(2500 * (attempt + 1));
        continue;
      }
      if (!r.ok) return null;
      const data = await r.json();
      if (Array.isArray(data) && data.length === list.length) return data;
      if (typeof data === 'string' && list.length === 1) return [data];
      return null;
    } catch {
      await sleep(1200 * (attempt + 1));
    }
  }
  return null;
}

/** Экранирует строку для NSIS, сохраняя переменную $ExistingDir. */
function esc(s) {
  return String(s)
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/"/g, "'")
    .split(TOKEN)
    .join(KEEP)
    .replace(/\$/g, '$$$$')
    .split(KEEP)
    .join('$ExistingDir');
}

const lines = [
  '; Сгенерировано scripts/make-installer-strings.mjs — правьте скрипт, а не этот файл.',
  '',
];

for (const [define, tl] of Object.entries(LANGS)) {
  const res = await translateBatch(STRINGS.map(([, en]) => protect(en)), tl);
  if (!res) {
    console.log(`${tl}: batch failed, using English`);
    continue;
  }
  lines.push(`!ifndef ${define}\n!define ${define} 0\n!endif`);
  STRINGS.forEach(([name], i) => {
    const value = String(res[i] ?? '').replace(/\s+/g, ' ').trim();
    lines.push(`LangString ${name} \${${define}} "${esc(value || STRINGS[i][1])}"`);
  });
  lines.push('');
  console.log(`${tl}: ok`);
  await sleep(350);
}

fs.writeFileSync(OUT, '\ufeff' + lines.join('\n'), 'utf8');
console.log('written:', OUT);
