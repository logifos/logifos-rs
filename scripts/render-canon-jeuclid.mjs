// Renders every reference-corpus manifest case through the built JEuclid
// fork into corpus/canon/jeuclid/<flattened id>.svg (one JVM for all
// cases). Deterministic: pinned fork commit, sorted cases, no timestamps.
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('../', import.meta.url).pathname;
const canon = join(root, 'corpus/canon');
const env = join(canon, 'jeuclid-env');
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const flat = (id) => id.replaceAll('/', '__');

const corporaRoot = join(root, 'corpus/reference');
const only = process.argv[2];
const corpora = readdirSync(corporaRoot, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name)
  .filter((name) => !only || name === only)
  .sort();

const outDir = join(canon, 'jeuclid');
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
const planPath = join(outDir, 'plan.tsv');
writeFileSync(planPath, plan.join('\n') + '\n');
const classpath = `${join(env, 'classes')}:${join(env, 'src/jeuclid-core.jar')}:${join(env, 'src/jeuclid-core/libs/*')}`;
execFileSync('java', ['-cp', classpath, 'CanonRender', planPath], { stdio: 'inherit' });
