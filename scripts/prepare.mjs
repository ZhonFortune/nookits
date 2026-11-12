import { execFileSync } from 'node:child_process';
import { cp, mkdir, writeFile } from 'node:fs/promises';
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
