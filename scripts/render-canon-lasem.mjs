// Renders every reference-corpus manifest case through the built incumbent
// Lasem into corpus/canon/lasem/<flattened id>.svg. Deterministic: pinned
// vendored Lasem revision, sorted cases, no timestamps.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('../', import.meta.url).pathname;
const canon = join(root, 'corpus/canon');
const outDir = join(canon, 'lasem');
const gemDir = process.env.LASEM_RUBY_DIR;
if (!gemDir) throw new Error('set LASEM_RUBY_DIR to the lasem-ruby checkout');
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const flat = (id) => id.replaceAll('/', '__');

const corporaRoot = join(root, 'corpus/reference');
const only = process.argv[2];
const corpora = readdirSync(corporaRoot, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name)
  .filter((name) => !only || name === only)
  .sort();

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });
const plan = [];
for (const corpus of corpora) {
  const manifest = JSON.parse(readFileSync(join(corporaRoot, corpus, 'manifest.json'), 'utf8'));
  for (const item of manifest.cases ?? []) {
    if (item.input_kind !== 'mathml') continue;
    const bytes = readFileSync(join(corporaRoot, corpus, item.path));
    if (sha256(bytes) !== item.sha256) throw new Error(`${corpus}/${item.id}: corpus hash mismatch`);
    const source = join(outDir, `${corpus}__${flat(item.id)}.mml`);
    writeFileSync(source, bytes);
    plan.push(`${source}\t${join(outDir, `${corpus}__${flat(item.id)}.svg`)}`);
  }
}

// The incumbent segfaults on malformed input; chunks isolate the blast
// radius so one crash records its remaining cases as render errors and the
// rest of the corpus still renders.
const CHUNK = 256;
let rendered = 0;
let errored = 0;
for (let start = 0; start < plan.length; start += CHUNK) {
  const chunk = plan.slice(start, start + CHUNK);
  const planPath = join(outDir, `plan-${start}.tsv`);
  writeFileSync(planPath, chunk.join('\n') + '\n');
  try {
    execFileSync('bundle', ['exec', 'ruby', '-Ilib', join(root, 'scripts/canon/lasem/LasemRender.rb'), planPath], {
      cwd: gemDir,
      stdio: ['ignore', 'ignore', 'ignore'],
      env: {
        ...process.env,
        DYLD_LIBRARY_PATH: `${gemDir}/vendor/lasem/install/lib`,
      },
    });
  } catch {
    // A crashed chunk leaves its remaining cases unrecorded; the driver
    // fills them in below.
  }
  for (const line of chunk) {
    const target = line.split('\t')[1];
    if (existsSync(`${target}.error`)) {
      errored += 1;
    } else if (existsSync(target)) {
      rendered += 1;
    } else {
      writeFileSync(`${target}.error`, 'the renderer process crashed on this chunk\n');
      errored += 1;
    }
  }
}
console.log(`rendered ${rendered}, errored ${errored}`);
