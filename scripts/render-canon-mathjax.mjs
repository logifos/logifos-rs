// Renders every reference-corpus manifest case through the pinned MathJax
// into corpus/canon/mathjax/<flattened id>.svg, recording per-case render
// errors as .error sidecars. Deterministic: pinned package, sorted cases,
// no timestamps.
import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { readdirSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join } from 'node:path';

const root = new URL('../', import.meta.url).pathname;
const canon = join(root, 'corpus/canon');
const require = createRequire(join(canon, 'mathjax-env/node_modules/mathjax/package.json'));
const mathjaxModule = require('mathjax');

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const flat = (id) => id.replaceAll('/', '__');

const startup = await new Promise((resolve, reject) => {
  mathjaxModule.init({
    loader: { load: ['adaptors/liteDOM', 'input/mml', 'input/mml/entities', 'output/svg'] },
    startup: { typeset: false },
  }).then(() => resolve(mathjaxModule.startup)).catch(reject);
});

const corporaRoot = join(root, 'corpus/reference');
const only = process.argv[2];
const corpora = readdirSync(corporaRoot, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name)
  .filter((name) => !only || name === only)
  .sort();

mkdirSync(join(canon, 'mathjax'), { recursive: true });
let rendered = 0;
let errored = 0;
for (const corpus of corpora) {
  const manifest = JSON.parse(readFileSync(join(corporaRoot, corpus, 'manifest.json'), 'utf8'));
  for (const item of manifest.cases ?? []) {
    const bytes = readFileSync(join(corporaRoot, corpus, item.path));
    if (sha256(bytes) !== item.sha256) throw new Error(`${corpus}/${item.id}: corpus hash mismatch`);
    if (item.input_kind !== 'mathml') continue;
    // liteDOM treats an XML declaration as a second root; the canon
    // renders the MathML content, so the prolog is stripped up front.
    const source = bytes.toString('utf8').replace(/^\s*<\?xml[^?]*\?>/, '').trim();
    let svg = null;
    try {
      const node = startup.document.convert(source);
      svg = startup.adaptor.outerHTML(node);
    } catch (error) {
      errored += 1;
      writeFileSync(join(canon, 'mathjax', `${corpus}__${flat(item.id)}.error`), String(error.message ?? error) + '\n');
      continue;
    }
    rendered += 1;
    writeFileSync(join(canon, 'mathjax', `${corpus}__${flat(item.id)}.svg`), svg);
  }
}
console.log(`rendered ${rendered}, errored ${errored}`);
