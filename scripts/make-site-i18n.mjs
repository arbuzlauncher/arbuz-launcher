// Мультиязычный сайт лаунчера: site/source.html → site/index.html.
//
// Русский текст берётся из разметки ([data-i18n]), английский — из словаря EN,
// который уже был в исходнике; остальные языки переводятся сами
// (Google-эндпоинт, как в переводе лаунчера) и складываются в site/i18n.json.
// Олбанский — тем же албанизатором, что и интерфейс (javer.kiev.ua/alban.php).
//
//   node scripts/make-site-i18n.mjs          — добрать переводы
//   node scripts/make-site-i18n.mjs --write  — плюс собрать site/index.html
import fs from 'node:fs';

const SRC = 'site/source.html';
const OUT = 'site/index.html';
const CACHE = 'site/i18n.json';

/** Наши языки: код, самоназвание и код для переводчика. */
const LANGS = [
  ['ru', 'Русский', null],
  ['en', 'English', null],
  ['fa', 'فارسی', 'fa'],
  ['ar', 'العربية', 'ar'],
  ['he', 'עברית', 'he'],
  ['az', 'Azərbaycanca', 'az'],
  ['hy', 'Հայերեն', 'hy'],
  ['pt', 'Português', 'pt'],
  ['pl', 'Polski', 'pl'],
  ['uk', 'Українська', 'uk'],
  ['be', 'Беларуская', 'be'],
  ['sr', 'Српски', 'sr'],
  ['sk', 'Slovenčina', 'sk'],
  ['eo', 'Esperanto', 'eo'],
  ['et', 'Eesti', 'et'],
  ['fi', 'Suomi', 'fi'],
  ['fr', 'Français', 'fr'],
  ['de', 'Deutsch', 'de'],
  ['lv', 'Latviešu', 'lv'],
  ['lt', 'Lietuvių', 'lt'],
  ['zh', '中文', 'zh-CN'],
  ['ja', '日本語', 'ja'],
  ['kk', 'Қазақша', 'kk'],
  ['ky', 'Кыргызча', 'ky'],
  ['uz', 'Oʻzbekcha', 'uz'],
  ['tg', 'Тоҷикӣ', 'tg'],
  ['iu', 'ᐃᓄᒃᑎᑐᑦ', 'iu'],
  ['olb', 'Олбанский', 'olb'],
  ['ka', 'ქართული', 'ka'],
];

const html = fs.readFileSync(SRC, 'utf8');

// --- тексты: значение data-i18n и содержимое элемента ----------------------
const keys = [];
const ru = {};
for (const m of html.matchAll(/<([a-z0-9]+)([^>]*?)data-i18n="([^"]+)"[^>]*>([\s\S]*?)<\/\1>/gi)) {
  const key = m[3];
  if (!ru[key]) keys.push(key);
  ru[key] = m[4].trim();
}

// --- английский уже есть в исходнике --------------------------------------
const en = {};
const enBlock = html.match(/var EN = \{([\s\S]*?)\n  \};/);
if (enBlock) {
  for (const m of enBlock[1].matchAll(/(\w+):\s*"((?:[^"\\]|\\.)*)"/g)) {
    en[m[1]] = m[2].replace(/\\"/g, '"');
  }
}

// --- названия акцентов ----------------------------------------------------
const seeds = {};
for (const m of html.matchAll(/(\d+):\s*\{\s*ru:\s*"([^"]+)",\s*en:\s*"([^"]+)"\s*\}/g)) {
  seeds[m[1]] = { ru: m[2], en: m[3] };
}

const origin = { ...ru };
for (const h of Object.keys(seeds)) origin[`seed.${h}`] = seeds[h].ru;
const english = { ...en };
for (const h of Object.keys(seeds)) english[`seed.${h}`] = seeds[h].en;

// --- переводчики ----------------------------------------------------------
const PUA = (i) => String.fromCharCode(0xe000 + i) + String.fromCharCode(0xe100 + i);

function protect(s) {
  const vars = [];
  const body = s.replace(/<[^>]+>/g, (v) => {
    const token = PUA(vars.length);
    vars.push(v);
    return token;
  });
  return { body, vars };
}

function restore(s, vars) {
  let out = s;
  vars.forEach((v, i) => {
    out = out.split(PUA(i)).join(v);
    out = out.split(`${PUA(i)} `.trim()).join(v);
  });
  return out;
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function google(list, tl) {
  const url =
    'https://clients5.google.com/translate_a/t?client=dict-chrome-ex&sl=ru&tl=' +
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

// Албанизатор отдаёт windows-1251 — гоняем байты вручную.
const dec1251 = new TextDecoder('windows-1251');
const CP = {};
for (let b = 0x80; b <= 0xff; b++) {
  const c = dec1251.decode(new Uint8Array([b]));
  if (c && c !== '\uFFFD') CP[c] = b;
}
for (let b = 0x20; b < 0x7f; b++) CP[String.fromCharCode(b)] = b;
const encQ = (s) =>
  [...s].map((ch) => '%' + (CP[ch] ?? 0x3f).toString(16).toUpperCase().padStart(2, '0')).join('');

async function alban(text) {
  for (let i = 0; i < 3; i++) {
    try {
      const r = await fetch('https://javer.kiev.ua/alban.php?input=' + encQ(text), {
        headers: { 'User-Agent': 'Mozilla/5.0' },
      });
      if (!r.ok) continue;
      const page = dec1251.decode(Buffer.from(await r.arrayBuffer()));
      const areas = page
        .split('<textarea')
        .slice(1)
        .map((x) => x.split('</textarea>')[0].replace(/^[^>]*>/, '').trim());
      if (areas.length) return areas[areas.length - 1];
    } catch {
      /* повтор */
    }
    await sleep(400);
  }
  return null;
}

// --- прогон ---------------------------------------------------------------
let cache = {};
if (fs.existsSync(CACHE)) {
  try {
    cache = JSON.parse(fs.readFileSync(CACHE, 'utf8'));
  } catch {
    cache = {};
  }
}
cache.ru = origin;
cache.en = { ...english, ...(cache.en ?? {}) };

const BATCH = 20;
for (const [code, , tl] of LANGS) {
  if (code === 'ru' || code === 'en') continue;
  cache[code] ??= {};
  const done = cache[code];
  const todo = keys.filter((k) => !done[k]);
  if (todo.length) {
    if (tl === 'olb') {
      for (const k of todo) {
        const out = await alban(origin[k]);
        if (out) done[k] = out;
        await sleep(250);
      }
    } else {
      for (let i = 0; i < todo.length; i += BATCH) {
        const chunk = todo.slice(i, i + BATCH);
        const prot = chunk.map((k) => protect(origin[k]));
        const res = await google(
          prot.map((p) => p.body),
          tl,
        );
        if (!res) {
          console.log(`${code}: пакет не перевёлся, повтор позже`);
          continue;
        }
        chunk.forEach((k, n) => {
          const t = restore(String(res[n] ?? ''), prot[n].vars);
          if (t.trim()) done[k] = t;
        });
        fs.writeFileSync(CACHE, JSON.stringify(cache, null, 1), 'utf8');
        await sleep(300);
      }
    }
  }
  const have = keys.filter((k) => done[k]).length;
  console.log(`${code}: ${have}/${keys.length}`);
}

// Названия акцентов переводим теми же дорожками.
for (const h of Object.keys(seeds)) {
  const key = `seed.${h}`;
  for (const [code, , tl] of LANGS) {
    cache[code] ??= {};
    if (cache[code][key]) continue;
    if (code === 'ru') cache[code][key] = seeds[h].ru;
    else if (code === 'en') cache[code][key] = seeds[h].en;
    else if (tl === 'olb') {
      const out = await alban(seeds[h].ru);
      cache[code][key] = out || seeds[h].ru;
      await sleep(250);
    } else {
      const res = await google([seeds[h].ru], tl);
      cache[code][key] = res?.[0] ?? seeds[h].ru;
      await sleep(200);
    }
    fs.writeFileSync(CACHE, JSON.stringify(cache, null, 1), 'utf8');
  }
}

if (!process.argv.includes('--write')) {
  console.log('переводы собраны; запустите с --write для сборки страницы');
  process.exit(0);
}

// --- сборка страницы ------------------------------------------------------
const q = (s) => JSON.stringify(s);

const js = `(function () {
  var I18N = ${JSON.stringify(cache, null, 2)};
  var NAMES = ${JSON.stringify(LANGS.map(([c, n]) => [c, n]))};
  var SPREAD = Object.keys(I18N);
  var RTL = ["fa", "ar", "he"];

  var texts = Array.prototype.slice.call(document.querySelectorAll("[data-i18n]"));
  texts.forEach(function (el) { el.setAttribute("data-ru", el.innerHTML); });
  var dots = Array.prototype.slice.call(document.querySelectorAll(".dot"));
  var nameEl = document.getElementById("seed-name");
  var select = document.getElementById("lang");

  NAMES.forEach(function (pair) {
    var o = document.createElement("option");
    o.value = pair[0];
    o.textContent = pair[1];
    select.appendChild(o);
  });

  var lang = "ru";
  var hue = 15;
  var SEED_HUES = [15, 150, 245, 295, 65];
  try {
    var asked = new URLSearchParams(location.search).get("l");
    var saved = localStorage.getItem("arbuz-lang");
    var probe = asked || saved;
    if (probe && I18N[probe]) lang = probe;
    else {
      var sys = (navigator.language || "ru").slice(0, 2).toLowerCase();
      if (I18N[sys]) lang = sys;
    }
    var sh = Number(localStorage.getItem("arbuz-hue"));
    if (SEED_HUES.indexOf(sh) >= 0) hue = sh;
  } catch (e) {}

  function words(code) {
    var table = I18N[code] || {};
    return function (key, fallback) { return table[key] !== undefined ? table[key] : fallback; };
  }

  function render() {
    var w = words(lang);
    document.documentElement.lang = lang;
    document.documentElement.dir = RTL.indexOf(lang) >= 0 ? "rtl" : "ltr";
    document.documentElement.style.setProperty("--h", String(hue));
    select.value = lang;
    texts.forEach(function (el) {
      var key = el.getAttribute("data-i18n");
      el.innerHTML = w(key, el.getAttribute("data-ru"));
    });
    dots.forEach(function (d) {
      var h = Number(d.getAttribute("data-h"));
      var name = w("seed." + h, String(h));
      d.setAttribute("aria-pressed", h === hue ? "true" : "false");
      d.setAttribute("aria-label", name);
      d.setAttribute("title", name);
    });
    nameEl.textContent = w("seed." + hue, "");
  }

  function remember(k, v) { try { localStorage.setItem(k, String(v)); } catch (e) {} }

  dots.forEach(function (d) {
    d.addEventListener("click", function () {
      hue = Number(d.getAttribute("data-h"));
      remember("arbuz-hue", hue);
      render();
    });
  });
  select.addEventListener("change", function () {
    lang = select.value;
    remember("arbuz-lang", lang);
    try {
      var u = new URL(location.href);
      u.searchParams.set("l", lang);
      history.replaceState(null, "", u);
    } catch (e) {}
    render();
  });

  render();
})();`;

let out = html;

// 1. Переключатель языка: пилюли RU/EN → список всех наших языков.
out = out.replace(
  /<div class="seg" role="group" aria-label="Language">[\s\S]*?<\/div>/,
  '<select class="lang-select" id="lang" aria-label="Language"></select>',
);

// 2. Стиль переключателя.
out = out.replace(
  /<\/style>/,
  `.lang-select{appearance:none;background:var(--surface-2);color:var(--fg);border:1px solid var(--outline);border-radius:999px;padding:7px 30px 7px 14px;font:500 13px var(--font-body);cursor:pointer;background-image:linear-gradient(45deg,transparent 50%,var(--fg-muted) 50%),linear-gradient(135deg,var(--fg-muted) 50%,transparent 50%);background-position:calc(100% - 16px) 52%,calc(100% - 11px) 52%;background-size:5px 5px,5px 5px;background-repeat:no-repeat}
</style>`,
);

// 3. Скрипт целиком.
out = out.replace(/<script>[\s\S]*?<\/script>/, `<script>\n${js}\n</script>`);

// 4. Заголовок страницы и описание — тоже на языке браузера оставим русскими,
//    но с пометкой о языках в <html>.
out = out.replace('<html>', '<html lang="ru">');

fs.writeFileSync(OUT, out, 'utf8');
console.log('готово:', OUT, out.length, 'байт,', keys.length, 'строк ×', LANGS.length, 'языков');
