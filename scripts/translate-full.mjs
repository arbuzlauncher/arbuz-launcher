import fs from 'node:fs';
import path from 'node:path';

const ROOT = process.cwd();
const SRC = path.join(ROOT, 'src', 'lib', 'i18n.ts');
const JSON_OUT = path.join(ROOT, 'src', 'lib', 'i18n-auto.json');
const TS_OUT = path.join(ROOT, 'src', 'lib', 'i18n-auto.ts');

const LANGS = {
  fa: 'fa',
  ar: 'ar',
  he: 'he',
  az: 'az',
  hy: 'hy',
  pt: 'pt',
  pl: 'pl',
  uk: 'uk',
  eo: 'eo',
  zh: 'zh-CN',
  et: 'et',
  fi: 'fi',
  fr: 'fr',
  de: 'de',
  be: 'be',
  sr: 'sr',
  sk: 'sk',
  ja: 'ja',
  tg: 'tg',
  uz: 'uz',
  ky: 'ky',
  lt: 'lt',
  lv: 'lv',
  kk: 'kk',
  ka: 'ka',
  iu: 'iu',
};

function readEntries() {
  const text = fs.readFileSync(SRC, 'utf8');
  const start = text.indexOf('const EN: Table = {');
  if (start < 0) throw new Error('EN table not found');
  const end = text.indexOf('\n};', start);
  const block = text.slice(start, end);
  const re = /(?:^|\n)\s*'((?:[^'\\]|\\.)*)'\s*:\s*'((?:[^'\\]|\\.)*)',?/g;
  const out = [];
  let m;
  while ((m = re.exec(block))) {
    const un = (s) => s.replace(/\\'/g, "'").replace(/\\\\/g, '\\').replace(/\\n/g, '\n');
    out.push([un(m[1]), un(m[2])]);
  }
  return out;
}

const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);

function protect(s) {
  const vars = [];
  const text = s.replace(/\{[a-zA-Z0-9_]+\}/g, (v) => {
    const token = PUA(vars.length);
    vars.push([token, v]);
    return token;
  });
  return { text, vars };
}

function restore(s, vars) {
  let out = s;
  for (const [token, value] of vars) out = out.split(token).join(value);
  return out;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function translateBatch(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=en&tl=' +
    encodeURIComponent(tl) +
    list.map((s) => '&q=' + encodeURIComponent(s)).join('');
  for (let attempt = 0; attempt < 6; attempt++) {
    try {
      const r = await fetch(url, {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
        },
      });
      if (r.status === 429) {
        await sleep(2000 * (attempt + 1));
        continue;
      }
      if (!r.ok) return null;
      const data = await r.json();
      if (Array.isArray(data) && data.length === list.length) return data;
      if (typeof data === 'string' && list.length === 1) return [data];
      return null;
    } catch {
      await sleep(1000 * (attempt + 1));
    }
  }
  return null;
}

const entries = readEntries();
let progress = {};
if (fs.existsSync(JSON_OUT)) {
  try {
    progress = JSON.parse(fs.readFileSync(JSON_OUT, 'utf8'));
  } catch {
    progress = {};
  }
}

const BATCH = 25;
// ONLY=fa,ar — перевести только эти языки (остальные берутся из кэша).
const only = (process.env.ONLY || '')
  .split(',')
  .map((s) => s.trim())
  .filter(Boolean);
let requests = 0;

for (const [lang, tl] of Object.entries(LANGS)) {
  if (only.length && !only.includes(lang)) continue;
  progress[lang] ??= {};
  const done = progress[lang];
  const pending = entries.filter(([key]) => !done[key]);
  if (pending.length === 0) {
    console.log(`${lang}: already complete`);
    continue;
  }
  for (let i = 0; i < pending.length; i += BATCH) {
    const chunk = pending.slice(i, i + BATCH);
    const protectedChunk = chunk.map(([, value]) => protect(value));
    const texts = protectedChunk.map((p) => p.text);
    const res = await translateBatch(texts, tl);
    requests++;
    if (!res) {
      console.log(`${lang}: batch ${i / BATCH} failed, skipping`);
      await sleep(1500);
      continue;
    }
    chunk.forEach(([key], n) => {
      const translated = restore(String(res[n] ?? ''), protectedChunk[n].vars);
      if (translated.trim()) done[key] = translated;
    });
    fs.writeFileSync(JSON_OUT, JSON.stringify(progress, null, 1), 'utf8');
    const pct = Math.min(100, Math.round(((i + chunk.length) / pending.length) * 100));
    console.log(`${lang}: ${pct}% (${requests} requests)`);
    await sleep(350);
  }
}

const esc = (s) => s.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n');
const lines = [];
lines.push('export const AUTO: Record<string, Record<string, string>> = {');
for (const [lang, table] of Object.entries(progress)) {
  lines.push(`  ${lang}: {`);
  for (const [key, value] of Object.entries(table)) {
    lines.push(`    '${esc(key)}': '${esc(value)}',`);
  }
  lines.push('  },');
}
lines.push('};');
lines.push('');
fs.writeFileSync(TS_OUT, lines.join('\n'), 'utf8');

for (const [lang, table] of Object.entries(progress)) {
  console.log(`${lang}: ${Object.keys(table).length}/${entries.length}`);
}
