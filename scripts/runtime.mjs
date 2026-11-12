import { readFileSync } from 'node:fs';
import { createJiti } from 'jiti';
import { createRequire } from 'node:module';
import path from 'node:path';

async function run() {
  const [script, dataDir] = process.argv.slice(2);
  if (!script || !dataDir || !path.isAbsolute(script) || !path.isAbsolute(dataDir)) {
    throw new Error('An absolute script path and data directory are required.');
  }
  const jiti = createJiti(script, { fsCache: false, moduleCache: false });
  const source = readFileSync(script, 'utf8').replace(/^\uFEFF/, '').trim().replace(/;\s*$/, '');
  const entry = jiti.evalModule(`module.exports = (${source}\n);`, { filename: script });
  if (typeof entry !== 'function') throw new TypeError('The script must contain a function.');
  await entry(Object.freeze({
    dataDir,
    scriptsDir: path.dirname(script),
    scriptPath: script,
    require: createRequire(script),
  }));
}

run().catch((error) => {
  console.error(error instanceof Error ? error.stack : String(error));
  process.exitCode = 1;
});
