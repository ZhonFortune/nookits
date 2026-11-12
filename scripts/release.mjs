import { appendFile, copyFile, mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const tag = process.env.GITHUB_REF_NAME ?? '';
const pattern = /^(?:v\.)?((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))$/;
const match = pattern.exec(tag);
const valid = Boolean(match) && match[2].split('.').every((part) => !/^\d+$/.test(part) || part === '0' || !part.startsWith('0'));
const mode = process.argv[2];
const releaseCategories = new Map([
  ['feat', 'Features'],
  ['fix', 'Fixes'],
  ['perf', 'Performance'],
  ['refactor', 'Refactoring'],
  ['docs', 'Documentation'],
  ['style', 'Style'],
  ['test', 'Tests'],
  ['build', 'Build'],
  ['ci', 'CI'],
  ['chore', 'Chore'],
  ['revert', 'Reverts'],
]);

function categoryFor(subject) {
  const parsed = /^([a-z][a-z0-9-]*)(?:\([^()\r\n]+\))?!?\s*[:：]\s*\S/i.exec(subject);
  if (!parsed) return 'Other Changes';
  const type = parsed[1].toLowerCase();
  return releaseCategories.get(type) ?? `${type[0].toUpperCase()}${type.slice(1)}`;
}

function git(...args) {
  return execFileSync('git', args, {
    cwd: root, encoding: 'utf8', timeout: 10000, maxBuffer: 16 * 1024 * 1024,
  }).trim();
}

function escapeMarkdown(value) {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
    .replace(/([\\`*_{}\[\]()!|])/g, '\\$1');
}

async function writeReleaseNotes() {
  const candidates = git('tag', '--merged', tag).split('\n').filter((candidate) => {
    if (candidate === tag) return false;
    const parsed = pattern.exec(candidate);
    return parsed && parsed[2].split('.').every((part) => !/^\d+$/.test(part) || part === '0' || !part.startsWith('0'));
  });
  const previous = candidates.length > 0
    ? git('describe', '--tags', '--abbrev=0', ...candidates.map((candidate) => `--match=${candidate}`), tag)
    : null;
  const range = previous ? `${previous}..${tag}` : tag;
  const records = git('log', '--reverse', '--format=%H%x00%s%x00', range, '--').split('\0');
  const repository = `${process.env.GITHUB_SERVER_URL ?? 'https://github.com'}/${process.env.GITHUB_REPOSITORY ?? ''}`;
  if (!process.env.GITHUB_REPOSITORY) throw new Error('GITHUB_REPOSITORY is required for release notes.');
  const groups = new Map();
  for (let index = 0; index + 1 < records.length; index += 2) {
    const hash = records[index].trim();
    const subject = records[index + 1].trim();
    if (!/^[a-f0-9]{40,64}$/.test(hash)) throw new Error('Invalid Git commit record.');
    const category = categoryFor(subject);
    if (!groups.has(category)) groups.set(category, []);
    groups.get(category).push(`- ${escapeMarkdown(subject)} [${hash.slice(0, 7)}](${repository}/commit/${hash})`);
  }
  const english = await readJson('locales/en.json');
  const chinese = await readJson('locales/zh-CN.json');
  const metadata = await readJson('package.json');
  const categoryOrder = [...releaseCategories.values()];
  const rank = (category) => {
    if (category === 'Other Changes') return categoryOrder.length + 1;
    const index = categoryOrder.indexOf(category);
    return index === -1 ? categoryOrder.length : index;
  };
  const changes = groups.size > 0
    ? [...groups.entries()]
      .sort(([left], [right]) => rank(left) - rank(right) || left.localeCompare(right))
      .map(([category, items]) => `**${category}:**\n\n${items.join('\n')}`)
      .join('\n\n')
    : '**Other Changes:**\n\n- No new commits.';
  await writeFile(join(root, 'release-notes.md'),
    `${chinese.aboutDescription}\n\n${english.aboutDescription}\n\n${changes}\n\n${metadata.author}\n`);
}

async function readJson(path) {
  return JSON.parse(await readFile(join(root, path), 'utf8'));
}

async function writeJson(path, value) {
  await writeFile(join(root, path), `${JSON.stringify(value, null, 2)}\n`);
}

async function replaceVersion(path, pattern, version) {
  const file = join(root, path);
  const source = await readFile(file, 'utf8');
  if (!pattern.test(source)) throw new Error(`Version field not found in ${path}`);
  await writeFile(file, source.replace(pattern, (_match, prefix, suffix) => `${prefix}${version}${suffix}`));
}

if (mode === 'validate') {
  if (!process.env.GITHUB_OUTPUT) throw new Error('GITHUB_OUTPUT is required.');
  await appendFile(process.env.GITHUB_OUTPUT, `publish=${valid}\n`);
} else {
  if (!valid) throw new Error('Expected N.N.N-STR or v.N.N.N-STR release tag.');
  const version = match[1];
  if (mode === 'prepare') {
    const metadata = await readJson('package.json');
    const lock = await readJson('package-lock.json');
    const config = await readJson('src-tauri/tauri.conf.json');
    const english = await readJson('locales/en.json');
    metadata.version = version;
    metadata.description = english.aboutDescription;
    lock.version = version;
    lock.packages[''].version = version;
    config.version = version;
    config.bundle.shortDescription = english.aboutDescription;
    const installerVersion = version.split('-')[0];
    const numbers = installerVersion.split('.').map(Number);
    if (numbers[0] > 255 || numbers[1] > 255 || numbers[2] > 65535) {
      throw new Error('MSI requires major/minor <= 255 and patch <= 65535.');
    }
    config.bundle.windows ??= {};
    config.bundle.windows.wix ??= {};
    config.bundle.windows.wix.version = installerVersion;
    await writeJson('package.json', metadata);
    await writeJson('package-lock.json', lock);
    await writeJson('src-tauri/tauri.conf.json', config);
    await replaceVersion('src-tauri/Cargo.toml', /(^\[package\][\s\S]*?\nversion = ")[^"]+("\r?$)/m, version);
    await replaceVersion('src-tauri/Cargo.lock', /(^name = "nookits"\r?\nversion = ")[^"]+("\r?$)/m, version);
  } else if (mode === 'notes') {
    await writeReleaseNotes();
  } else if (mode === 'collect') {
    const { RELEASE_PLATFORM: platform, RELEASE_CPU: cpu } = process.env;
    if (![platform, cpu].every((part) => typeof part === 'string' && /^[A-Za-z0-9_-]+$/.test(part))) {
      throw new Error('RELEASE_PLATFORM and RELEASE_CPU must be filename-safe values.');
    }
    const bundle = join(root, 'src-tauri', 'target', 'release', 'bundle', 'msi');
    const installers = (await readdir(bundle)).filter((name) => name.endsWith('.msi'));
    if (installers.length !== 1) throw new Error('Expected exactly one MSI installer.');
    const destination = join(root, 'release-assets');
    await mkdir(destination, { recursive: true });
    await copyFile(join(bundle, installers[0]), join(destination, `Nookits-${platform}-${cpu}-${tag}.msi`));
  } else {
    throw new Error('Expected validate, prepare, notes, or collect.');
  }
}
