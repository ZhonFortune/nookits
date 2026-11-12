import { execFileSync } from 'node:child_process';
import { cp, mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
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
  await writeFile(join(runtime, 'runner.cjs'), `require(${JSON.stringify(join(root, 'runtime', 'runner.cjs'))});\n`);
} else {
  await cp(join(root, 'runtime', 'runner.cjs'), join(runtime, 'runner.cjs'));
}
const jitiEntry = require.resolve('jiti');
const jitiRoot = resolve(dirname(jitiEntry), '..');
await cp(jitiRoot, join(runtime, 'node_modules', 'jiti'), { recursive: true });
// Generate an original, dependency-free RGBA tray icon as a Windows ICO.
const size = 32;
const pixels = Buffer.alloc(size * size * 4);
for (let y = 0; y < size; y++) {
  for (let x = 0; x < size; x++) {
    const i = ((size - 1 - y) * size + x) * 4;
    const mark = x >= 8 && x <= 23 && y >= 7 && y <= 24 &&
      (x <= 11 || x >= 20 || Math.abs(x - y) < 3);
    pixels.set(mark ? [245, 245, 245, 255] : [46, 38, 31, 255], i);
  }
}
const mask = Buffer.alloc(size * size / 8);
const header = Buffer.alloc(22);
header.writeUInt16LE(1, 2); header.writeUInt16LE(1, 4);
header[6] = size; header[7] = size;
header.writeUInt16LE(1, 10); header.writeUInt16LE(32, 12);
header.writeUInt32LE(40 + pixels.length + mask.length, 14);
header.writeUInt32LE(22, 18);
const dib = Buffer.alloc(40);
dib.writeUInt32LE(40); dib.writeInt32LE(size, 4); dib.writeInt32LE(size * 2, 8);
dib.writeUInt16LE(1, 12); dib.writeUInt16LE(32, 14);
dib.writeUInt32LE(pixels.length + mask.length, 20);
await writeFile(join(root, 'src-tauri', 'generated', 'icon.ico'), Buffer.concat([header, dib, pixels, mask]));
