import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.locator('#status')).toContainText('Ready'); });

test('renderers share one worker and keep separate streams', async ({ page }) => {
  const texts = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const views = Array.from({ length: 24 }, () => {
      const element = document.createElement('div'); document.body.append(element);
      return new MarkitRenderer(element);
    });
    // Interleave growing sources so each stream sends appends to the same worker.
    for (let step = 0; step < 6; step++) {
      await Promise.all(views.map((view, i) => view.render(`# Message ${i}\n\n` + 'word '.repeat(step * 3) + `step ${step}`, { finalized: false })));
    }
    await Promise.all(views.map((view, i) => view.render(`# Message ${i}\n\n` + 'word '.repeat(15) + 'step 5\n\n```ts\nconst x = 1;\n```\n')));
    window.views = views;
    return [...document.querySelectorAll('body > div.markit')].map(el => el.textContent);
  });
  expect(texts).toHaveLength(24);
  texts.forEach((text, i) => { expect(text).toContain(`Message ${i}`); expect(text).toContain('step 5'); expect(text).toContain('const x = 1;'); });
  expect(page.workers()).toHaveLength(1);
});

test('edits that are not appends resend the source', async ({ page }) => {
  const html = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const element = document.createElement('div'); document.body.append(element);
    const view = new MarkitRenderer(element);
    await view.render('Hello **wor', { finalized: false });
    await view.render('Hello **world**', { finalized: false });
    await view.render('Goodbye *moon*');
    return element.innerHTML;
  });
  expect(html).toContain('<em>moon</em>');
  expect(html).not.toContain('Hello');
});

test('destroying one renderer cancels only its work and frees its stream', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const make = () => { const element = document.createElement('div'); document.body.append(element); return [element, new MarkitRenderer(element)]; };
    const [, doomed] = make(); const [kept, survivor] = make();
    const big = '```rust\n' + 'let value = 1;\n'.repeat(20000) + '```\n';
    const pending = doomed.render(big).then(() => 'resolved', error => error.message);
    const other = survivor.render('Still **here**');
    doomed.destroy(); doomed.destroy();
    const late = await doomed.render('x').then(() => 'resolved', error => error.message);
    await other;
    return { pending: await pending, late, kept: kept.innerHTML };
  });
  expect(result.pending).toBe('Renderer disposed');
  expect(result.late).toBe('Renderer disposed');
  expect(result.kept).toContain('<strong>here</strong>');
  expect(page.workers()).toHaveLength(1);
});

test('dedicated workers opt out and terminate on destroy; shared workers restart', async ({ page }) => {
  const closed = new Promise(resolve => page.on('worker', worker => worker.on('close', resolve)));
  await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const element = document.createElement('div'); document.body.append(element);
    window.dedicated = new MarkitRenderer(element, { worker: 'dedicated' });
    await window.dedicated.render('Own *worker*');
  });
  expect(page.workers()).toHaveLength(2);
  await page.evaluate(() => window.dedicated.destroy());
  await closed;
  await expect.poll(() => page.workers().length).toBe(1);
  const html = await page.evaluate(async () => {
    const { terminateMarkitWorkers } = await import('/index.js');
    await window.renderer.render('Before restart', { finalized: false });
    terminateMarkitWorkers();
    await window.renderer.render('Before restart, after **restart**');
    return document.querySelector('#preview').innerHTML;
  });
  expect(html).toContain('<strong>restart</strong>');
  expect(html.match(/Before restart/g)).toHaveLength(1);
});

test('streamed code fences grow in place and match a fresh render', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const make = () => { const element = document.createElement('div'); document.body.append(element); return [element, new MarkitRenderer(element)]; };
    const source = 'Intro\n\n```ts title="a.ts"\n' + Array.from({ length: 40 }, (_, i) => i === 20 ? '/* opens a comment' : i === 25 ? 'closes it */ const s = "😀";' : `const v${i} = ${i}; // line`).join('\n') +
      '\n```\n\n```diff\n--- a\n+++ b\n-old line\n+new line\n context\n```\n\nAfter.\n';
    const [streamed, live] = make();
    let kept = 0, fence;
    for (let at = 3; at < source.length; at += 3) {
      await live.render(source.slice(0, at), { finalized: false });
      const current = streamed.querySelector('.mk-codeblock code');
      if (current && fence === current) kept++;
      fence = current;
    }
    await live.render(source);
    const [fresh, once] = make();
    await once.render(source);
    return { same: streamed.innerHTML === fresh.innerHTML, kept };
  });
  expect(result.same).toBe(true);
  expect(result.kept).toBeGreaterThan(100);
});

test('preloading starts the shared worker that the first renderer reuses', async ({ page }) => {
  const spawned = [];
  page.on('worker', worker => spawned.push(worker));
  await page.evaluate(async () => {
    const { preloadMarkit, terminateMarkitWorkers } = await import('/index.js');
    terminateMarkitWorkers();
    preloadMarkit(); preloadMarkit();
  });
  await expect.poll(() => spawned.length).toBe(1);
  await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const views = [1, 2, 3].map(() => new MarkitRenderer(document.body.appendChild(document.createElement('div'))));
    await Promise.all(views.map((view, i) => view.render(`# Preloaded ${i}\n`)));
  });
  await expect(page.locator('body > div.markit h1')).toHaveText(['Preloaded 0', 'Preloaded 1', 'Preloaded 2']);
  expect(spawned).toHaveLength(1);
});
