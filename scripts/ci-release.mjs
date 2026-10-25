// Решает, что делать с версией и релизом в CI.
//
//   pull_request  — ничего не трогаем, только проверки;
//   push main     — поднимаем patch, коммитим, ставим тег, отдаём его сборкам;
//   tag v*        — собираем как есть;
//   dispatch      — release=<версия> или bump=patch|minor|major.
//
// Пишет version/tag/ref/release в $GITHUB_OUTPUT.
import { appendFileSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const env = process.env;
const emit = (key, value) => {
  if (env.GITHUB_OUTPUT) appendFileSync(env.GITHUB_OUTPUT, `${key}=${value}\n`);
  console.log(`${key}=${value}`);
};
const versionNow = () => JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version;
const run = (file, args) => execFileSync(file, args, { stdio: 'inherit' });
const setVersion = (version) => run(process.execPath, ['scripts/set-version.mjs', version]);

const event = env.GITHUB_EVENT_NAME || '';
const refType = env.GITHUB_REF_TYPE || '';
const refName = env.GITHUB_REF_NAME || '';
const sha = env.GITHUB_SHA || '';
const input = (env.RELEASE_INPUT || '').trim().replace(/^v/i, '');
const bump = (env.BUMP || '').trim().toLowerCase();
const message = env.COMMIT_MSG || '';

const skip = (why) => {
  emit('version', versionNow());
  emit('tag', '');
  emit('ref', sha);
  emit('release', 'false');
  console.log(`релиз не нужен: ${why}`);
  process.exit(0);
};

const tagExists = (tag) => {
  try {
    execFileSync('git', ['rev-parse', '--verify', `refs/tags/${tag}`], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
};

/**
 * Ставит версию, коммитит её и ставит тег, если такого ещё нет.
 * Возвращает имя тега — по нему сборки делают checkout.
 */
const publishTag = (version) => {
  const tag = `v${version}`;
  if (tagExists(tag)) {
    console.log(`тег ${tag} уже есть — собираем его`);
    return tag;
  }

  setVersion(version);
  run('git', ['config', 'user.name', 'github-actions[bot]']);
  run('git', ['config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com']);
  run('git', ['add', 'package.json', 'src-tauri/tauri.conf.json', 'src-tauri/Cargo.toml', 'src-tauri/Cargo.lock']);
  try {
    run('git', ['commit', '-m', `chore(release): ${tag}`]);
  } catch {
    // Файлы уже с этой версией — коммитить нечего.
  }
  run('git', ['tag', tag]);
  run('git', ['push', 'origin', `HEAD:${refName}`]);
  run('git', ['push', 'origin', tag]);
  console.log(`новая версия: ${tag}`);
  return tag;
};

if (event === 'pull_request') skip('pull request');
if (message.includes('[skip release]')) skip('в коммите [skip release]');

// Готовый тег: версия и файлы уже на месте.
if (refType === 'tag') {
  emit('version', versionNow());
  emit('tag', refName);
  emit('ref', refName);
  emit('release', 'true');
  process.exit(0);
}

// Ручной запуск с указанной версией.
if (input) {
  const tag = publishTag(input);
  emit('version', input);
  emit('tag', tag);
  emit('ref', tag);
  emit('release', 'true');
  process.exit(0);
}

const onMain = event === 'push' && (refName === 'main' || refName === 'master');
const kind = bump && bump !== 'none' ? bump : onMain ? 'patch' : '';
if (!kind) skip('версию не поднимали');
if (onMain && message.startsWith('chore(release):')) skip('это коммит самого релиза');

const next = execFileSync(process.execPath, ['scripts/bump-version.mjs', kind], {
  encoding: 'utf8',
}).trim();
const tag = publishTag(next);

emit('version', next);
emit('tag', tag);
emit('ref', tag);
emit('release', 'true');
