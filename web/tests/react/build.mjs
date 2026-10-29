// Bundles the React test page. React is a dev dependency only; the published
// binding imports it as a peer. Markit's own modules stay unbundled at /index.js
// so the worker and Wasm URLs resolve exactly as they do for consumers.
import { build } from 'esbuild';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
export const markitExternal = {
  name: 'markit-external',
  setup(b) { b.onResolve({ filter: /\/index\.js$/ }, args => args.importer.endsWith(`${path.sep}react.js`) ? { path: '/index.js', external: true } : undefined); },
};
export default async function buildReactTests() {
  await build({
    entryPoints: [path.join(here, 'app.jsx')], outfile: path.join(here, 'dist/app.js'), bundle: true, format: 'esm',
    jsx: 'automatic', minify: true, define: { 'process.env.NODE_ENV': '"production"' }, plugins: [markitExternal], logLevel: 'error',
  });
}
if (process.argv[1] === fileURLToPath(import.meta.url)) await buildReactTests();
