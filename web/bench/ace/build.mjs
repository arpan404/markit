// Bundles Ace's web Markdown stack (apps/web/src/components/markdown at a
// pinned commit) into a production React page for the comparison benchmark.
// Usage: node bench/ace/build.mjs <ace checkout> <out dir>
import { build } from 'esbuild';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const [ace, out] = process.argv.slice(2).map(p => path.resolve(p));
const web = path.join(ace, 'apps/web/src');
const ui = path.join(ace, 'packages/ui-core/src');
const stubs = path.join(here, 'stubs.jsx');
const stubbed = ['components/thread-reference.tsx', 'components/attachment-markdown.tsx', 'components/file-reference.tsx',
  'components/pull-request-link.tsx', 'components/web-link.tsx', 'components/ui/skeleton.tsx'].map(f => path.join(web, f));
const uiShim = ['content-hash', 'diff', 'motion', 'inline-markdown', 'lru', 'checkout-files']
  .map(f => `export * from ${JSON.stringify(path.join(ui, `${f}.ts`))};`).join('\n');

const ace_plugin = {
  name: 'ace',
  setup(b) {
    b.onResolve({ filter: /^@ace\/ui-core$/ }, () => ({ path: 'ui-core-shim', namespace: 'ace-shim' }));
    b.onLoad({ filter: /.*/, namespace: 'ace-shim' }, () => ({ contents: uiShim, resolveDir: ui, loader: 'ts' }));
    b.onResolve({ filter: /^@ace\/ui-core\// }, args => ({ path: path.join(ui, `${{ 'file-references': 'file-references', 'math-render': 'math-render' }[args.path.slice(12)] ?? args.path.slice(12)}.ts`) }));
    b.onResolve({ filter: /^@\// }, args => {
      const target = path.join(web, args.path.slice(2));
      return stubbed.includes(target) ? { path: stubs } : { path: target };
    });
    b.onResolve({ filter: /^\.\.?\// }, args => {
      const target = path.resolve(args.resolveDir, args.path);
      return stubbed.includes(target) ? { path: stubs } : undefined;
    });
    // Vite bundles the worker from `new URL("./markdown.worker.ts", import.meta.url)`;
    // esbuild builds it as a separate entry next to the chunks.
    b.onLoad({ filter: /components[\\/]markdown[\\/]worker\.ts$/ }, async args => {
      const fs = await import('node:fs');
      return { contents: fs.readFileSync(args.path, 'utf8').replace('"./markdown.worker.ts"', '"./markdown.worker.js"'), loader: 'ts' };
    });
    // Vite `?url` imports: emit the font as a file and return its URL.
    b.onResolve({ filter: /\?url$/ }, args => ({ path: path.resolve(args.resolveDir, args.path.startsWith('.') ? args.path.slice(0, -4) : require_resolve(args.path.slice(0, -4))), namespace: 'url' }));
    b.onLoad({ filter: /.*/, namespace: 'url' }, async args => ({ contents: `export default new URL(${JSON.stringify('./fonts/' + path.basename(args.path))}, import.meta.url).href;`, loader: 'js' }));
  },
};
function require_resolve(spec) { return path.join(here, '../../node_modules', spec); }

const common = {
  bundle: true, format: 'esm', minify: true, splitting: true, outdir: out, metafile: true,
  nodePaths: [path.join(here, '../../node_modules')], jsx: 'automatic', target: 'es2022',
  loader: { '.woff2': 'file', '.woff': 'file', '.ttf': 'file' }, assetNames: 'fonts/[name]',
  define: { 'import.meta.env.MODE': '"production"', 'import.meta.env.DEV': 'false', 'process.env.NODE_ENV': '"production"' },
  plugins: [ace_plugin], logLevel: 'error',
};
const result = await build({ ...common, entryPoints: { ace: path.join(here, 'app.jsx') } });
const workers = await build({ ...common, splitting: false, entryPoints: { 'markdown.worker': path.join(web, 'components/markdown/markdown.worker.ts') } });
const fs = await import('node:fs');
fs.writeFileSync(path.join(out, 'meta.json'), JSON.stringify({ app: result.metafile, worker: workers.metafile }));
