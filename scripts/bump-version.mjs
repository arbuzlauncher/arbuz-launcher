// Следующая версия: node scripts/bump-version.mjs patch|minor|major
// Печатает только номер, файлы не трогает — их обновляет set-version.mjs.
import { readFileSync } from 'node:fs';

const kind = (process.argv[2] || 'patch').toLowerCase();
const current = JSON.parse(readFileSync('package.json', 'utf8')).version.replace(/^v/i, '');
const [major, minor, patch] = current.split('.').map((n) => parseInt(n, 10) || 0);

let next;
if (kind === 'major') next = `${major + 1}.0.0`;
else if (kind === 'minor') next = `${major}.${minor + 1}.0`;
else if (kind === 'patch') next = `${major}.${minor}.${patch + 1}`;
else {
  console.error('usage: node scripts/bump-version.mjs patch|minor|major');
  process.exit(1);
}

console.log(next);
