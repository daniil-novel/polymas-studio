import { readFile, writeFile, mkdir } from 'node:fs/promises';
import process from 'node:process';
import { fileURLToPath, URL } from 'node:url';
import Ajv2020 from 'ajv/dist/2020.js';
import standalone from 'ajv/dist/standalone/index.js';
import { build } from 'esbuild';

const root = new URL('../', import.meta.url);
const schema = JSON.parse(
  await readFile(new URL('contracts/configuration-export.schema.json', root), 'utf8'),
);
const ajv = new Ajv2020({ strict: true, code: { source: true } });
const validator = ajv.compile(schema);
// Bundle the official standalone CJS output and its helpers into CSP-safe ESM.
// The browser receives no compiler, dynamic evaluation, or runtime dependency on Ajv.
const result = await build({
  stdin: {
    contents: standalone(ajv, validator),
    resolveDir: fileURLToPath(root),
    sourcefile: 'configuration-export-validator.cjs',
    loader: 'js',
  },
  bundle: true,
  write: false,
  platform: 'browser',
  format: 'esm',
  target: 'es2022',
  minify: false,
  legalComments: 'inline',
});
const output =
  '// Generated from contracts/configuration-export.schema.json; run npm run generate:contracts.\n' +
  result.outputFiles[0].text;
const licenseParts = [];
for (const [name, path] of [
  ['Ajv 8.20.0', 'ajv/LICENSE'],
  ['fast-deep-equal', 'fast-deep-equal/LICENSE'],
  ['esbuild 0.28.2 generated helpers', 'esbuild/LICENSE.md'],
]) {
  licenseParts.push(
    `${name}\n\n${(await readFile(new URL(`node_modules/${path}`, root), 'utf8')).trim()}\n`,
  );
}
const licenses =
  'Third-party code in the generated PolyMAS configuration export validator\n\n' +
  licenseParts.join('\n');
const target = new URL('apps/desktop/src/gen/configuration-export-validator.mjs', root);
const licenseTarget = new URL('apps/desktop/public/export-validator.LICENSE.txt', root);
if (process.argv.includes('--check')) {
  if ((await readFile(target, 'utf8')) !== output)
    throw new Error('Export validator is stale; run npm run generate:contracts');
  if ((await readFile(licenseTarget, 'utf8')) !== licenses)
    throw new Error('Export validator license notice is stale');
} else {
  await mkdir(new URL('./', target), { recursive: true });
  await writeFile(target, output);
  await mkdir(new URL('./', licenseTarget), { recursive: true });
  await writeFile(licenseTarget, licenses);
}
