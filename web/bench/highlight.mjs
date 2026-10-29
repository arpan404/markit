// Whole-file highlight timings through the Wasm build, in Node.
//
//   ./scripts/build-web.sh && npm --prefix web ci
//   node web/bench/highlight.mjs [file.ts]
//
// Builds ~2 KB, ~60 KB, ~490 KB and ~4 MB TypeScript inputs by repeating the
// file (or a built-in sample). Each sample prepends a distinct comment line so
// markit's result cache cannot answer it. Prism (a dev dependency) is timed on
// the same inputs for reference.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import init, { highlightCodePacked } from '../pkg/markit_wasm.js';
import { unpackHighlight } from '../highlight.js';
import { codeLines } from '../code-lines.js';

await init({ module_or_path: readFileSync(new URL('../pkg/markit_wasm_bg.wasm', import.meta.url)) });

const sample = `import { createServer, type Server } from "node:http";
// Fake daemon: answers JSON-RPC requests with canned events.
export interface Session { id: string; title?: string; createdAt: number }
const sessions = new Map<string, Session>();
/** Create a session and return its identifier. */
export async function createSession(title: string): Promise<string> {
  const id = \`s-\${sessions.size + 1}\`;
  sessions.set(id, { id, title, createdAt: Date.now() });
  return id;
}
`;
const seed = process.argv[2] ? readFileSync(process.argv[2], 'utf8') : sample;

function sized(target) {
  let out = '';
  while (out.length < target) out += seed;
  return out.slice(0, out.lastIndexOf('\n', target) + 1);
}

function median(code, run) {
  const samples = [];
  const budget = code.length > 1 << 20 ? 3000 : 600;
  const started = performance.now();
  for (let i = 0; samples.length < 5 || (performance.now() - started < budget && samples.length < 200); i++) {
    const source = `// run ${i}\n${code}`;
    const start = performance.now();
    run(source);
    samples.push(performance.now() - start);
  }
  samples.sort((a, b) => a - b);
  return samples[samples.length >> 1];
}

let prism;
try {
  const require = createRequire(import.meta.url);
  prism = require('prismjs');
  require('prismjs/components/prism-typescript.js');
} catch {
  prism = undefined;
}

console.log(['bytes', 'runs', 'packed ms', '+unpack ms', '+lines ms', 'prism ms'].map(s => s.padStart(11)).join(''));
for (const target of [2 * 1024, 60 * 1024, 490 * 1024, 4 * 1024 * 1024]) {
  const code = sized(target);
  const words = highlightCodePacked(code, 'ts');
  const runs = (words.length - 3 - words[2]) / 3;
  const packed = median(code, s => highlightCodePacked(s, 'ts'));
  const unpacked = median(code, s => unpackHighlight(s, highlightCodePacked(s, 'ts')));
  const lines = median(code, s => codeLines(s, highlightCodePacked(s, 'ts')));
  const reference = prism ? median(code, s => prism.tokenize(s, prism.languages.typescript)) : NaN;
  console.log([code.length, runs, packed, unpacked, lines, reference]
    .map(n => (Number.isInteger(n) ? String(n) : n.toFixed(3)).padStart(11)).join(''));
}
