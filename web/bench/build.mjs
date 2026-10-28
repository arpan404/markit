// Builds both harnesses: node bench/build.mjs [ace checkout]
import { build } from 'esbuild';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ace = path.resolve(process.argv[2] ?? path.join(here, '.ace'));
const dist = path.join(here, 'dist');
const webRoot = path.resolve(here, '..');
fs.rmSync(dist, { recursive: true, force: true });
// Markit is bundled the way a host bundler would ship it: the app (React,
// the binding and index.js) and the worker (worker.js and the wasm-bindgen glue)
// are minified separately; the Wasm module and full math font stay files next
// to the worker, where `new URL(..., import.meta.url)` finds them.
const markit = path.join(dist, 'markit');
const common = { bundle: true, format: 'esm', minify: true, target: 'es2022', logLevel: 'error' };
// Code splitting, like Vite: the WebGPU code painter is its own chunk and loads
// only when a page has WebGPU.
await build({
  ...common, splitting: true, entryPoints: { app: path.join(here, 'markit/app.jsx') }, outdir: markit, jsx: 'automatic',
  define: { 'process.env.NODE_ENV': '"production"' },
});
await build({ ...common, entryPoints: { worker: path.join(webRoot, 'worker.js') }, outdir: markit });
await build({ ...common, entryPoints: { style: path.join(webRoot, 'style.css') }, outdir: markit });
fs.copyFileSync(path.join(webRoot, 'pkg/markit_wasm_bg.wasm'), path.join(markit, 'markit_wasm_bg.wasm'));
fs.mkdirSync(path.join(markit, 'pkg'));
fs.copyFileSync(path.join(webRoot, 'pkg/STIXTwoMath-Regular.otf'), path.join(markit, 'pkg/STIXTwoMath-Regular.otf'));
execFileSync(process.execPath, [path.join(here, 'ace/build.mjs'), ace, path.join(dist, 'ace')], { stdio: 'inherit' });
// esbuild writes the same KaTeX and Markdown CSS once per chunk that imports
// it; a Vite build emits it once, so the page links each distinct sheet once.
const sheets = new Map();
for (const f of fs.readdirSync(path.join(dist, 'ace')).filter(f => f.endsWith('.css')).sort()) {
  const text = fs.readFileSync(path.join(dist, 'ace', f), 'utf8');
  if (!sheets.has(text)) sheets.set(text, f);
}
const css = [...sheets.values()].map(f => `<link rel="stylesheet" href="./ace/${f}">`).join('');
const page = (title, head, script) => `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title}</title><link rel="icon" href="data:,">${head}<style>body{margin:0;font:16px/1.6 system-ui,sans-serif}#root{max-width:860px;margin:auto;padding:16px}.bench-message{margin:0 0 24px}</style></head><body><div id="root"></div><script type="module" src="${script}"></script></body></html>`;
fs.writeFileSync(path.join(dist, 'markit.html'), page('Markit bench', '<link rel="stylesheet" href="./markit/style.css">', './markit/app.js'));
fs.writeFileSync(path.join(dist, 'ace.html'), page('Ace bench', css, './ace/ace.js'));
console.log('built', dist);
