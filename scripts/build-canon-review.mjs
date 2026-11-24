// Builds the visible-review set: corpus/canon/review/index.html pairs
// logifos's render of every shared case with each reference renderer's
// artifact, inlined so the page opens standalone in a browser.
import { existsSync, readFileSync, readdirSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const root = new URL('../', import.meta.url).pathname;
const canon = join(root, 'corpus/canon');

const renderers = readdirSync(canon, { withFileTypes: true })
  .filter((e) => e.isDirectory() && e.name !== 'logifos' && e.name !== 'review')
  .map((e) => e.name)
  .filter((name) => existsSyncPinned(name))
  .sort();

function existsSyncPinned(renderer) {
  try {
    readFileSync(join(canon, `${renderer}.json`), 'utf8');
    return true;
  } catch {
    return false;
  }
}

const ours = readdirSync(join(canon, 'logifos'))
  .filter((name) => name.endsWith('.svg'))
  .sort();

const sections = [];
for (const name of ours) {
  const id = name.replace(/\.svg$/, '').replaceAll('__', '/');
  const corpus = id.split('/')[0];
  const columns = [`<figure><figcaption>logifos</figcaption>${readFileSync(join(canon, 'logifos', name), 'utf8')}</figure>`];
  for (const renderer of renderers) {
    const svg = join(canon, renderer, name);
    const error = join(canon, renderer, `${name.slice(0, -4)}.error`);
    if (existsSync(svg)) {
      columns.push(`<figure><figcaption>${renderer}</figcaption>${readFileSync(svg, 'utf8')}</figure>`);
    } else if (existsSync(error)) {
      columns.push(`<figure class="error"><figcaption>${renderer}</figcaption><pre>${readFileSync(error, 'utf8').replace(/[<>&]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' }[c]))}</pre></figure>`);
    }
  }
  sections.push(`<section id="${corpus}/${id.slice(corpus.length + 1)}"><h2>${id}</h2><div class="row">${columns.join('')}</div></section>`);
}

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Logifos canon review set</title>
<style>
body { font-family: system-ui, sans-serif; margin: 2rem; }
h1 { font-size: 1.4rem; }
section { border-top: 1px solid #ddd; padding: 1rem 0; }
h2 { font-size: 0.85rem; font-family: monospace; font-weight: normal; }
.row { display: flex; gap: 2rem; align-items: flex-start; flex-wrap: wrap; }
figure { margin: 0; }
figcaption { font-size: 0.75rem; color: #555; margin-bottom: 0.25rem; }
figure svg { max-width: 320px; max-height: 160px; }
figure.error pre { font-size: 0.7rem; color: #a00; max-width: 320px; white-space: pre-wrap; }
</style>
</head>
<body>
<h1>Logifos canon review set — ${ours.length} shared cases, renderers: ${renderers.join(', ')}</h1>
${sections.join('\n')}
</body>
</html>
`;
mkdirSync(join(canon, 'review'), { recursive: true });
writeFileSync(join(canon, 'review', 'index.html'), html);
console.log(`review set: ${ours.length} cases, renderers ${renderers.join(', ')}`);
