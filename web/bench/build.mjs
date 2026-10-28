// Builds both harnesses: node bench/build.mjs [ace checkout]
import { build } from 'esbuild';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { markitExternal } from '../tests/react/build.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ace = path.resolve(process.argv[2] ?? path.join(here, '.ace'));
const dist = path.join(here, 'dist');
fs.rmSync(dist, { recursive: true, force: true });
await build({
  entryPoints: { app: path.join(here, 'markit/app.jsx') }, outdir: path.join(dist, 'markit'), bundle: true, format: 'esm', jsx: 'automatic',
  minify: true, target: 'es2022', define: { 'process.env.NODE_ENV': '"production"' }, plugins: [markitExternal], logLevel: 'error',
});
execFileSync(process.execPath, [path.join(here, 'ace/build.mjs'), ace, path.join(dist, 'ace')], { stdio: 'inherit' });
const css = fs.readdirSync(path.join(dist, 'ace')).filter(f => f.endsWith('.css')).map(f => `<link rel="stylesheet" href="./ace/${f}">`).join('');
const page = (title, head, script) => `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title}</title><link rel="icon" href="data:,">${head}<style>body{margin:0;font:16px/1.6 system-ui,sans-serif}#root{max-width:860px;margin:auto;padding:16px}.bench-message{margin:0 0 24px}</style></head><body><div id="root"></div><script type="module" src="${script}"></script></body></html>`;
fs.writeFileSync(path.join(dist, 'markit.html'), page('Markit bench', '<link rel="stylesheet" href="/style.css">', './markit/app.js'));
fs.writeFileSync(path.join(dist, 'ace.html'), page('Ace bench', css, './ace/ace.js'));
console.log('built', dist);
