// Генерирует полные языковые файлы NSIS (.nlf/.nsh) для языков, которых нет
// в стандартном наборе NSIS (азербайджанский) или чей стандартный перевод
// недостаточно полон (казахский, киргизский, таджикский, инуктитут).
// Запускать вручную при добавлении новых языков: node scripts/make-nsis-langs.mjs
import fs from 'node:fs';
import path from 'node:path';

const NSIS = path.join(process.env.LOCALAPPDATA, 'tauri', 'NSIS', 'Contrib', 'Language files');
const OUT = path.join('installer', 'lang');
const read = (name) => fs.readFileSync(path.join(NSIS, name), 'utf8');

const LANGS = {
  Kazakh: { tl: 'kk', id: 1087, native: 'Қазақша', ascii: 'Kazaksha' },
  Kyrgyz: { tl: 'ky', id: 1088, native: 'Кыргызча', ascii: 'Kyrgyzcha' },
  Tajik: { tl: 'tg', id: 1064, native: 'Тоҷикӣ', ascii: 'Tojiki' },
  Inuktitut: { tl: 'iu', id: 1117, native: 'ᐃᓄᒃᑎᑐᑦ', ascii: 'Inuktitut' },
  Azerbaijani: { tl: 'az', id: 1068, native: 'Azərbaycanca', ascii: 'Azerbaycanca' },
};

const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);
function protect(s) {
  const vars = [];
  const text = String(s).replace(
    /\$\([^)]*\)|\$_[A-Za-z]+|\$\\r\$\\n|\$[0-9A-Za-z_]+/g,
    (v) => {
      const token = PUA(vars.length);
      vars.push([token, v]);
      return token;
    },
  );
  return { text, vars };
}
const restore = (s, vars) => vars.reduce((a, [t, v]) => a.split(t).join(v), String(s));
const keepsTokens = (translated, vars) => vars.every(([token]) => String(translated).includes(token));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function trBatch(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    tl +
    list.map((s) => '&q=' + encodeURIComponent(s)).join('');
  for (let a = 0; a < 8; a++) {
    try {
      const r = await fetch(url, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      });
      if (r.status === 429) {
        await sleep(2500 * (a + 1));
        continue;
      }
      if (!r.ok) return null;
      const data = await r.json();
      if (Array.isArray(data) && data.length === list.length) return data;
      return null;
    } catch {
      await sleep(1200 * (a + 1));
    }
  }
  return null;
}

/** Переводит строки и восстанавливает служебные токены, если Google их потерял. */
async function translateChecked(values, tl, label) {
  const out = [];
  for (let i = 0; i < values.length; i += 20) {
    const chunk = values.slice(i, i + 20).map(protect);
    const res = await trBatch(
      chunk.map((c) => c.text),
      tl,
    );
    if (!res) throw new Error(`batch failed at ${i} (${label})`);
    chunk.forEach((c, n) => out.push({ value: String(res[n] ?? ''), vars: c.vars }));
    await sleep(300);
  }

  for (let i = 0; i < out.length; i++) {
    if (keepsTokens(out[i].value, out[i].vars)) continue;
    for (let attempt = 0; attempt < 3 && !keepsTokens(out[i].value, out[i].vars); attempt++) {
      const single = await trBatch([protect(values[i]).text], tl);
      if (single?.[0]) out[i].value = String(single[0]);
      await sleep(500);
    }
    if (!keepsTokens(out[i].value, out[i].vars)) {
      const missing = out[i].vars.filter(([t]) => !out[i].value.includes(t));
      out[i].value += ' ' + missing.map(([, v]) => v).join('');
      console.log(`  ! ${label}[${i}]: восстановлено токенов — ${missing.length}`);
    }
  }
  return out.map((x) => restore(x.value, x.vars));
}

function parseNlf(text) {
  const lines = text.split(/\r?\n/);
  const brand = lines.findIndex((l) => l.startsWith('# ^Branding'));
  const head = lines.slice(0, brand);
  const items = [];
  let i = brand;
  while (i < lines.length) {
    const l = lines[i];
    if (l.startsWith('# ^')) {
      items.push({ name: l.slice(3).trim(), value: lines[i + 1] ?? '' });
      i += 2;
    } else {
      i++;
    }
  }
  return { head, items };
}

/** Ручные правки там, где машинный перевод теряет смысл. */
const NLF_OVERRIDES = {
  Azerbaijani: {
    SetupCaption: '$(^Name) Quraşdırma',
    UninstallCaption: '$(^Name) Silinmə',
    BrowseBtn: '&Gözdən keçir...',
    ClickNext: 'Davam etmək üçün «Next» düyməsini klikləyin.',
    ClickInstall: 'Quraşdırmaya başlamaq üçün «Install» düyməsini klikləyin.',
    ClickUninstall: 'Silinməyə başlamaq üçün «Uninstall» düyməsini klikləyin.',
    Name: 'Ad',
    FileError:
      'Yazmaq üçün faylı açarkən xəta baş verdi: \\r\\n\\r\\n$0\\r\\n\\r\\nQuraşdırmanı dayandırmaq üçün «Abort» düyməsini klikləyin,\\r\\nYenidən cəhd etmək üçün «Retry» düyməsini klikləyin və ya\\r\\nbu faylı ötürmək üçün «Ignore» düyməsini klikləyin.',
    FileError_NoIgnore:
      'Yazmaq üçün faylı açarkən xəta baş verdi: \\r\\n\\r\\n$0\\r\\n\\r\\nYenidən cəhd etmək üçün «Retry» düyməsini klikləyin və ya\\r\\nquraşdırmanı dayandırmaq üçün «Cancel» düyməsini klikləyin.',
    CreatedUninstaller: 'Silinmə proqramı yaradıldı: ',
    Skipped: 'Ötürüldü: ',
    LogInstall: 'Quraşdırma prosesini qeyd et',
  },
};

const NSH_OVERRIDES = {
  Kazakh: {
    MUI_TEXT_WELCOME_INFO_TEXT:
      'Орнату $(^NameDA) бағдарламасын орнату бойынша нұсқаулық береді.$\\r$\\n$\\r$\\nОрнатуды бастамас бұрын барлық басқа қолданбаларды жабу ұсынылады. Бұл компьютерді қайта жүктемей-ақ тиісті жүйелік файлдарды жаңартуға мүмкіндік береді.$\\r$\\n$\\r$\\n$_CLICK',
    MUI_TEXT_FINISH_INFO_TEXT:
      '$(^NameDA) компьютеріңізге орнатылды.$\\r$\\n$\\r$\\nОрнатуды жабу үшін «Аяқтау» түймесін басыңыз.',
    MUI_UNTEXT_FINISH_INFO_TEXT:
      '$(^NameDA) компьютеріңізден жойылды.$\\r$\\n$\\r$\\nОрнатуды жабу үшін «Аяқтау» түймесін басыңыз.',
  },
  Kyrgyz: {
    MUI_TEXT_WELCOME_INFO_TEXT:
      'Орнотуу программасы $(^NameDA) программасын орнотууга жетектейт.$\\r$\\n$\\r$\\nОрнотууну баштаардан мурун бардык башка колдонмолорду жабуу сунушталат. Бул компьютериңизди кайра жүктөбөстөн тиешелүү системалык файлдарды жаңыртууга мүмкүндүк берет.$\\r$\\n$\\r$\\n$_CLICK',
    MUI_TEXT_FINISH_INFO_TEXT:
      '$(^NameDA) компьютериңизге орнотулду.$\\r$\\n$\\r$\\nОрнотууну жабуу үчүн «Аяктоо» баскычын басыңыз.',
    MUI_UNTEXT_FINISH_INFO_TEXT:
      '$(^NameDA) компьютериңизден чыгарылды.$\\r$\\n$\\r$\\nОрнотууну жабуу үчүн «Аяктоо» баскычын басыңыз.',
  },
  Inuktitut: {
    MUI_TEXT_ABORTWARNING: 'ᓄᖅᑲᕈᒪᓪᓚᕆᒃᐲᑦ $(^Name) ᐋᖅᑭᒃᓱᐃᓂᕐᒥᒃ?',
  },
  Azerbaijani: {
    MUI_TEXT_WELCOME_INFO_TEXT:
      'Quraşdırma sizi $(^NameDA) quraşdırılması boyunca yönləndirəcək.$\\r$\\n$\\r$\\nQuraşdırmaya başlamazdan əvvəl bütün digər proqramları bağlamaq tövsiyə olunur. Bu, kompüterinizi yenidən yükləmədən müvafiq sistem fayllarını yeniləməyə imkan verəcək.$\\r$\\n$\\r$\\n$_CLICK',
    MUI_UNTEXT_WELCOME_INFO_TITLE: '$(^NameDA) Silinməyə xoş gəlmisiniz',
    MUI_TEXT_DIRECTORY_SUBTITLE: '$(^NameDA) quraşdırılacaq qovluğu seçin.',
    MUI_UNTEXT_DIRECTORY_TITLE: 'Silinmə yerini seçin',
    MUI_UNTEXT_DIRECTORY_SUBTITLE: '$(^NameDA) silinəcək qovluğu seçin.',
    MUI_TEXT_FINISH_INFO_TEXT:
      '$(^NameDA) kompüterinizə quraşdırılıb.$\\r$\\n$\\r$\\nQuraşdırmanı bağlamaq üçün «Finish» düyməsini klikləyin.',
    MUI_TEXT_FINISH_RUN: '&Çalışdır $(^NameDA)',
    MUI_UNTEXT_FINISH_INFO_TEXT:
      '$(^NameDA) kompüterinizdən silinib.$\\r$\\n$\\r$\\nQuraşdırmanı bağlamaq üçün «Finish» düyməsini klikləyin.',
    MUI_UNTEXT_CONFIRM_SUBTITLE: '$(^NameDA) proqramını kompüterinizdən silin.',
  },
};

const { head, items } = parseNlf(read('English.nlf'));
const nshSource = read('English.nsh').split(/\r?\n/);
const only = process.env.ONLY?.split(',').map((s) => s.trim()).filter(Boolean);

for (const [name, cfg] of Object.entries(LANGS)) {
  if (only && !only.includes(name)) continue;
  console.log(`${name} (${cfg.tl})…`);

  // --- .nlf ----------------------------------------------------------------
  const translated = await translateChecked(
    items.map((x) => x.value),
    cfg.tl,
    `${name}.nlf`,
  );
  for (const [key, value] of Object.entries(NLF_OVERRIDES[name] ?? {})) {
    const index = items.findIndex((x) => x.name === key);
    if (index >= 0) translated[index] = value;
  }

  const headOut = head.map((l) => {
    if (l === '# Translation by ..... (any credits should go here)') {
      return '# Translation by Google Translate (generated by scripts/make-nsis-langs.mjs)';
    }
    return l;
  });
  const idIndex = headOut.findIndex(
    (l, i) =>
      i > 1 && /^\d+$/.test(l.trim()) && headOut.slice(0, i).join('\n').includes('# Language ID'),
  );
  if (idIndex >= 0) headOut[idIndex] = String(cfg.id);

  const nlf = [...headOut];
  items.forEach((x, i) => {
    nlf.push(`# ^${x.name}`);
    nlf.push(
      String(translated[i])
        .replace(/[\uE000-\uF8FF]/g, '')
        .replace(/\r?\n/g, ' ')
        .trim(),
    );
  });
  // head[0] из English.nlf уже содержит UTF-8 BOM — второй добавлять нельзя.
  fs.writeFileSync(path.join(OUT, `${name}.nlf`), nlf.join('\r\n') + '\r\n', 'utf8');

  // --- .nsh ----------------------------------------------------------------
  const nshLines = nshSource.slice();
  nshLines[0] = `;Language: ${name} (${cfg.id})`;
  nshLines[1] = ';By Google Translate (generated by scripts/make-nsis-langs.mjs)';

  const targets = [];
  for (let i = 0; i < nshLines.length; i++) {
    const line = nshLines[i];
    if (!line.includes('${LangFileString}')) continue;
    const m = line.match(/^(.*?)\s\$\{LangFileString\}\s+(\w+)\s+"([\s\S]*)"(\s*)$/);
    if (!m) throw new Error(`bad line in English.nsh: ${line}`);
    targets.push({ index: i, prefix: m[1], name: m[2], text: m[3], suffix: m[4] });
  }
  const nshTranslated = await translateChecked(
    targets.map((t) => t.text),
    cfg.tl,
    `${name}.nsh`,
  );
  targets.forEach((t, n) => {
    const value = (NSH_OVERRIDES[name]?.[t.name] ?? nshTranslated[n])
      .replace(/[\uE000-\uF8FF]/g, '')
      .replace(/\r?\n/g, ' ')
      .trim()
      .replace(/"/g, "'");
    nshLines[t.index] = `${t.prefix} \${LangFileString} ${t.name} "${value}"${t.suffix}`;
  });
  const langIndex = nshLines.findIndex((l) => l.startsWith('!insertmacro LANGFILE'));
  nshLines[langIndex] =
    `!insertmacro LANGFILE "${name}" "${name}" "${cfg.native}" "${cfg.ascii}"`;
  fs.writeFileSync(path.join(OUT, `${name}.nsh`), '\ufeff' + nshLines.join('\r\n'), 'utf8');
}

console.log('готово');
