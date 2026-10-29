import { test, expect } from '@playwright/test';

// Opt-in benchmark: MARKIT_BENCH=1 npx playwright test stream-bench
// MARKIT_BENCH_RUNTIME picks the runtime module (default /index.js), so another
// build of the runtime served from this origin can be compared on the same answer.
// Streams a ~40 KB answer in 20-character deltas at 200 deltas a second, as the
// host does, and reports DOM work and main-thread time.
const runtime = process.env.MARKIT_BENCH_RUNTIME ?? '/index.js';
const rate = Number(process.env.MARKIT_BENCH_RATE ?? 200);

test.skip(!process.env.MARKIT_BENCH, 'benchmark runs only with MARKIT_BENCH=1');
test.setTimeout(120_000);

test('streaming a 40 KB answer: DOM applies, mutations and main-thread time', async ({ page }) => {
  await page.goto('/tests/bench/stream.html');
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Performance.enable');
  const metrics = async () => Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m => [m.name, m.value]));
  if (process.env.MARKIT_BENCH_PROFILE) { await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval', { interval: 100 }); await cdp.send('Profiler.start'); }
  const before = await metrics();
  const result = await page.evaluate(async ({ runtime, rate }) => {
    const { MarkitRenderer } = await import(runtime);
    const { longAnswer } = await import('/tests/bench/answer.js');
    const answer = longAnswer();
    const root = document.querySelector('#answer');
    const renderer = new MarkitRenderer(root);
    await renderer.render('', { finalized: false });
    const counts = { applies: 0, records: 0, added: 0, removed: 0, remounts: 0, frames: 0, applied: 0 };
    const settled = new WeakSet();
    new MutationObserver(records => {
      counts.applies++;
      counts.records += records.length;
      for (const record of records) {
        counts.added += record.addedNodes.length; counts.removed += record.removedNodes.length;
        if (record.target === root) for (const node of record.removedNodes) if (settled.has(node)) counts.remounts++;
      }
      for (const block of [...root.children].slice(0, -1)) settled.add(block);
    }).observe(root, { childList: true, subtree: true, characterData: true, attributes: true });
    let ticking = true;
    const tick = () => { counts.frames++; if (ticking) requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
    const started = performance.now();
    await new Promise(resolve => {
      let at = 0;
      const timer = setInterval(() => {
        at = Math.min(answer.length, at + 20);
        renderer.render(answer.slice(0, at), { finalized: false }).then(patch => { if (!patch.superseded) counts.applied++; });
        if (at === answer.length) { clearInterval(timer); resolve(); }
      }, 1000 / rate);
    });
    await renderer.render(answer);
    ticking = false;
    return { ...counts, chars: answer.length, ms: performance.now() - started, text: root.textContent.length };
  }, { runtime, rate });
  const after = await metrics();
  if (process.env.MARKIT_BENCH_PROFILE) {
    const { profile } = await cdp.send('Profiler.stop');
    const self = new Map(), dt = new Map(profile.nodes.map(n => [n.id, n]));
    profile.samples.forEach((id, i) => { const n = dt.get(id); const key = `${n.callFrame.functionName || '(anon)'} ${n.callFrame.url.split('/').pop()}:${n.callFrame.lineNumber}`; self.set(key, (self.get(key) ?? 0) + (profile.timeDeltas[i] ?? 0) / 1000); });
    console.log([...self].filter(([k]) => !k.startsWith('(idle)') && !k.startsWith('(program)')).sort((a, b) => b[1] - a[1]).slice(0, 25).map(([k, v]) => `${v.toFixed(1)}ms ${k}`).join('\n'));
  }
  const script = (after.ScriptDuration - before.ScriptDuration) * 1000;
  const task = (after.TaskDuration - before.TaskDuration) * 1000;
  const layout = (after.LayoutDuration - before.LayoutDuration) * 1000;
  const style = (after.RecalcStyleDuration - before.RecalcStyleDuration) * 1000;
  const report = {
    runtime, chars: result.chars, seconds: +(result.ms / 1000).toFixed(1), frames: result.frames,
    domApplies: result.applies, resolvedApplied: result.applied, mutationRecords: result.records,
    nodesAdded: result.added, nodesRemoved: result.removed, settledRemounts: result.remounts,
    scriptMs: Math.round(script), layoutMs: Math.round(layout), styleMs: Math.round(style), taskMs: Math.round(task),
    taskMsPerFrame: +(task / result.frames).toFixed(2),
  };
  console.log(JSON.stringify(report));
  expect(result.remounts).toBe(0);
  expect(result.text).toBeGreaterThan(30_000);
});
