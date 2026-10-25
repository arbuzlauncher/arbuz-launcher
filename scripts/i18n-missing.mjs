// Ищет строки, которые интерфейс просит перевести, но которых нет в EN.
import fs from 'node:fs';
import path from 'node:path';

const text = fs.readFileSync('src/lib/i18n.ts', 'utf8');
const start = text.indexOf('const EN: Table = {');
const end = text.indexOf('\n};', start);
const block = text.slice(start, end);
const re =
  /(?:^|\n)\s*(?:'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)")\s*:\s*(?:'((?:[^'\\]|\\.)*)'|"((?:[^"\\]|\\.)*)")/g;
const en = new Set();
let m;
while ((m = re.exec(block))) en.add(m[1] ?? m[2]);

const un = (s) => s.replace(/\\(\\|'|"|n)/g, (_, c) => (c === 'n' ? '\n' : c));
const used = new Map();
const files = [];
function walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p);
    else if (/\.(tsx?|jsx?)$/.test(e.name) && !/i18n/.test(e.name)) files.push(p);
  }
}
walk('src');

for (const f of files) {
  const src = fs.readFileSync(f, 'utf8');
  const res = [
    /\bt\(\s*'((?:[^'\\]|\\.)*)'/g,
    /\bt\(\s*"((?:[^"\\]|\\.)*)"/g,
    /\btp\(\s*'((?:[^'\\]|\\.)*)'/g,
  ];
  for (const r of res) {
    let x;
    while ((x = r.exec(src))) used.set(un(x[1]), f);
  }
}

const missing = [...used].filter(([k]) => !en.has(k));
console.log(`missing: ${missing.length}`);
for (const [k, f] of missing) console.log(`${f}\t${k}`);
