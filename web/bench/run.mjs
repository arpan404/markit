// Markit vs Ace web Markdown benchmark in Chromium (Playwright).
// Usage: node bench/run.mjs [--runs 5] [--out bench/results/latest.json]
// Build first: bench/ace/fetch.sh && node bench/build.mjs
import { chromium } from '@playwright/test';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { transcript, message, longAnswer } from './fixtures.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(here, '..');
const args = process.argv.slice(2);
const flag = (name, fallback) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : fallback; };
const runs = Number(flag('--runs', 5));
// CPU throttling applies Chromium's CDP slowdown to the page, e.g. 4 for a mid-range phone.
const throttle = Number(flag('--throttle', 1));
const out = path.resolve(flag('--out', path.join(here, 'results/latest.json')));

const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.html': 'text/html', '.wasm': 'application/wasm', '.woff2': 'font/woff2', '.woff': 'font/woff', '.ttf': 'font/ttf', '.json': 'application/json' };
const served = [];
const server = http.createServer((req, res) => {
  const file = path.join(webRoot, decodeURIComponent(new URL(req.url, 'http://x').pathname));
  if (!file.startsWith(webRoot) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); res.end(); return; }
  const body = fs.readFileSync(file);
  served.push(file);
  res.writeHead(200, { 'content-type': types[path.extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' });
  res.end(body);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const gzipSizes = new Map();
const gz = file => { if (!gzipSizes.has(file)) gzipSizes.set(file, zlib.gzipSync(fs.readFileSync(file), { level: 9 }).length); return gzipSizes.get(file); };
function transfer(files) {
  const unique = [...new Set(files)].filter(f => !f.endsWith('.html'));
  return { files: unique.length, raw: unique.reduce((s, f) => s + fs.statSync(f).size, 0), gzip: unique.reduce((s, f) => s + gz(f), 0) };
}

const browser = await chromium.launch({ args: ['--enable-precise-memory-info', '--js-flags=--expose-gc'] });
async function open(impl, setup) {
  const context = await browser.newContext({ viewport: { width: 1280, height: 900 } });
  const page = await context.newPage();
  page.on('pageerror', error => console.error(impl, 'page error:', error.message));
  served.length = 0;
  if (setup) await page.addInitScript(setup);
  const cdp = await context.newCDPSession(page);
  await cdp.send('Performance.enable');
  if (throttle > 1) await cdp.send('Emulation.setCPUThrottlingRate', { rate: throttle });
  await page.goto(`${base}/bench/dist/${impl}.html`);
  await page.waitForFunction(() => window.benchReady);
  return { context, page, cdp };
}
/** Main-thread task and script time (ms) spent inside `run`. */
async function busy(cdp, run) {
  const read = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
  const before = await read();
  const value = await run();
  const after = await read();
  return { ...value, mainTaskMs: (after.TaskDuration - before.TaskDuration) * 1e3, mainScriptMs: (after.ScriptDuration - before.ScriptDuration) * 1e3 };
}
/** JS heap plus ArrayBuffer/Wasm backing stores of every dedicated worker. */
async function workerMemory() {
  const session = await browser.newBrowserCDPSession();
  const { targetInfos } = await session.send('Target.getTargets');
  let used = 0, backing = 0;
  for (const target of targetInfos.filter(t => t.type === 'worker')) {
    const { sessionId } = await session.send('Target.attachToTarget', { targetId: target.targetId, flatten: false });
    const reply = new Promise(resolve => {
      const listener = event => {
        if (event.sessionId !== sessionId) return;
        const message = JSON.parse(event.message);
        if (message.id === 1) { session.off('Target.receivedMessageFromTarget', listener); resolve(message.result); }
      };
      session.on('Target.receivedMessageFromTarget', listener);
    });
    await session.send('Target.sendMessageToTarget', { sessionId, message: JSON.stringify({ id: 1, method: 'Runtime.getHeapUsage' }) });
    const heap = await Promise.race([reply, new Promise(resolve => setTimeout(() => resolve(null), 5000))]);
    if (heap) { used += heap.usedSize; backing += heap.backingStorageSize ?? 0; }
    await session.send('Target.detachFromTarget', { sessionId }).catch(() => {});
  }
  await session.detach();
  return { workerHeap: used, workerBacking: backing };
}

const impls = ['markit', 'ace'];
const fixtures = { one: message(3), other: message(8), t50: transcript(50), t200: transcript(200), long: longAnswer() };
const results = { meta: { date: new Date().toISOString(), runs, throttle, chromium: browser.version(), fixtures: { oneBytes: fixtures.one.length, t50Bytes: fixtures.t50.reduce((s, m) => s + m.text.length, 0), t200Bytes: fixtures.t200.reduce((s, m) => s + m.text.length, 0), longBytes: fixtures.long.length } }, samples: {} };
const record = (key, value) => { process.stderr.write(`${key} ${JSON.stringify(value)}\n`); (results.samples[key] ??= []).push(value); };
const only = flag('--only', '');

for (let run = 0; run < runs; run++) {
  for (const impl of impls.filter(i => !only || i === only)) {
    // Cold: page load, then one message; includes worker, Wasm and lazy chunks.
    let { context, page, cdp } = await open(impl);
    const loadMs = await page.evaluate(() => performance.now());
    const cold = await busy(cdp, () => page.evaluate(m => window.bench.mount([{ id: 'one', role: 'assistant', text: m }]), fixtures.one));
    record(`${impl}.first_message_cold`, { ...cold, sinceNavigation: loadMs + cold.ms });
    record(`${impl}.transfer_first_message`, transfer(served));
    const warm = await busy(cdp, () => page.evaluate(m => window.bench.mount([{ id: 'two', role: 'assistant', text: m }]), fixtures.other));
    record(`${impl}.first_message_warm`, warm);
    await context.close();
    if (impl === 'markit') {
      ({ context, page, cdp } = await open(impl, () => { window.markitPreload = true; }));
      const preloadMs = await page.evaluate(() => performance.now());
      const preloaded = await busy(cdp, () => page.evaluate(m => window.bench.mount([{ id: 'one', role: 'assistant', text: m }]), fixtures.one));
      record('markit.first_message_cold_preload', { ...preloaded, sinceNavigation: preloadMs + preloaded.ms });
      await context.close();
    }

    for (const [name, list, lazy] of [['transcript_50', fixtures.t50], ['transcript_200', fixtures.t200], ...(impl === 'markit' ? [['transcript_200_lazy', fixtures.t200, true]] : [])]) {
      ({ context, page, cdp } = await open(impl, lazy ? () => { window.markitLazy = '1200px 0px'; } : undefined));
      const mounted = await busy(cdp, () => page.evaluate(list => window.bench.mount(list), list));
      const memory = await page.evaluate(() => window.bench.memory());
      const wasm = impl === 'markit' ? await page.workers()[0].evaluate(() => self.markitStats().wasmMemory) : 0;
      record(`${impl}.${name}`, { ...mounted, ...memory, ...(await workerMemory()), wasmMemory: wasm, workers: page.workers().length });
      if (name === 'transcript_200' && !lazy) record(`${impl}.transfer_transcript`, transfer(served));
      await context.close();
    }

    ({ context, page, cdp } = await open(impl));
    await page.evaluate(m => window.bench.mount([{ id: 'warm', role: 'assistant', text: m }]), fixtures.other);
    const streamed = await busy(cdp, () => page.evaluate(text => window.bench.stream(text, { chunk: 24 }), fixtures.long));
    record(`${impl}.stream_long`, { ...streamed, lagMs: streamed.ms - streamed.fedMs, workers: page.workers().length });
    await context.close();
    process.stderr.write(`run ${run + 1}/${runs} ${impl} done\n`);
  }
}
await browser.close();
server.close();

const median = values => { const v = values.filter(x => typeof x === 'number').sort((a, b) => a - b); return v.length ? (v.length % 2 ? v[(v.length - 1) / 2] : (v[v.length / 2 - 1] + v[v.length / 2]) / 2) : null; };
results.medians = {};
for (const [key, samples] of Object.entries(results.samples)) {
  results.medians[key] = {};
  for (const field of Object.keys(samples[0])) results.medians[key][field] = median(samples.map(s => s[field]));
}
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify(results, null, 2));
console.log(JSON.stringify(results.medians, null, 2));
