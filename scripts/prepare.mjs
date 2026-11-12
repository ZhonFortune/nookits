import { execFileSync } from 'node:child_process';
import { cp, mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(import.meta.url);
const host = execFileSync('rustc', ['-vV'], { encoding: 'utf8', timeout: 10000 })
  .match(/^host: (.+)$/m)?.[1]?.trim();
if (!host) throw new Error('Unable to determine the Rust host target.');
const runtime = join(root, 'src-tauri', 'generated', 'runtime');
const binaries = join(root, 'src-tauri', 'binaries');
const installer = join(root, 'src-tauri', 'generated', 'installer');
const frontend = join(root, 'src-tauri', 'generated', 'frontend');
await mkdir(frontend, { recursive: true });
await cp(join(root, 'assets', 'index.html'), join(frontend, 'index.html'));
await mkdir(installer, { recursive: true });
const installerLocales = new Map([['en-US', 'en'], ['zh-CN', 'zh-CN']]);
const escapeXml = (value) => value.replace(/[&<>"']/g, (character) => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;',
})[character]);
for (const [culture, locale] of installerLocales) {
  const text = JSON.parse(await readFile(join(root, 'locales', `${locale}.json`), 'utf8'));
  const entries = Object.entries(text).filter(([key]) => key.startsWith('installer'));
  const strings = entries.map(([key, value]) => {
    if (typeof value !== 'string' || !value.trim()) throw new Error(`Invalid installer translation: ${locale}/${key}`);
    return `  <String Id="${key}">${escapeXml(value)}</String>`;
  });
  await writeFile(join(installer, `${culture}.wxl`),
    `<?xml version="1.0" encoding="utf-8"?>\n<WixLocalization Culture="${culture}" xmlns="http://schemas.microsoft.com/wix/2006/localization">\n${strings.join('\n')}\n</WixLocalization>\n`);
}
const config = JSON.parse(await readFile(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const fragment = await readFile(join(root, 'src-tauri', 'installer', 'uninstall.wxs'), 'utf8');
const runtimeName = config.bundle.externalBin.map((binary) => binary.split(/[\\/]/).at(-1))
  .find((name) => name.endsWith('-node'));
if (!runtimeName) throw new Error('The installer requires the bundled Node.js runtime.');
const installerValues = {
  manufacturer: config.bundle.publisher,
  product_name: config.productName,
  runtime_name: runtimeName,
};
await writeFile(join(installer, 'uninstall.wxs'), fragment.replace(/\{\{([a-z_]+)\}\}/g, (_token, key) => {
  const value = installerValues[key];
  if (typeof value !== 'string' || !value.trim()) throw new Error(`Missing installer value: ${key}`);
  return escapeXml(value);
}));
await mkdir(runtime, { recursive: true });
await mkdir(binaries, { recursive: true });
await cp(process.execPath, join(binaries, `nookits-node-${host}${process.platform === 'win32' ? '.exe' : ''}`));
if (process.argv.includes('--dev')) {
  await writeFile(join(runtime, 'runtime.mjs'), `import ${JSON.stringify(pathToFileURL(join(root, 'scripts', 'runtime.mjs')).href)};\n`);
} else {
  await cp(join(root, 'scripts', 'runtime.mjs'), join(runtime, 'runtime.mjs'));
}
const jitiEntry = require.resolve('jiti');
const jitiRoot = resolve(dirname(jitiEntry), '..');
await cp(jitiRoot, join(runtime, 'node_modules', 'jiti'), { recursive: true });
execFileSync(process.execPath, [
  require.resolve('@tauri-apps/cli/tauri.js'),
  'icon', join(root, 'assets', 'icon.svg'),
  '--output', join(root, 'src-tauri', 'generated', 'icons'),
], { cwd: root, stdio: 'inherit', timeout: 60000 });
