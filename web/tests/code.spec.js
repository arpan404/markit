import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.locator('#status')).toContainText('Ready'); });

/** Renders into a fresh element with `options`; returns nothing, leaves `window.view`. */
async function mount(page, options) {
  await page.evaluate(async options => {
    const { MarkitRenderer } = await import('/index.js');
    const element = document.createElement('div'); element.id = 'view'; document.body.append(element);
    window.view = new MarkitRenderer(element, options);
  }, options);
}
const fence = (lines, language = 'ts') => '```' + language + '\n' + Array.from({ length: lines }, (_, i) => `const v${i} = ${i};`).join('\n') + '\n```\n';
const gutter = page => page.locator('#view .mk-line').first().evaluate(line => getComputedStyle(line, '::before').content);

test('line numbers appear once a fence reaches lineNumbersFrom lines, on the same element', async ({ page }) => {
  await mount(page, { code: { lineNumbersFrom: 10 } });
  await page.evaluate(source => window.view.render(source, { finalized: false }), fence(9).slice(0, -4));
  const section = await page.locator('#view section').elementHandle();
  expect(await gutter(page)).toBe('none');
  await page.evaluate(source => window.view.render(source), fence(10));
  expect(await gutter(page)).toBe('"1"');
  expect(await page.evaluate(section => section.isConnected, section)).toBe(true);
});

test('long fences fold to collapseAfter lines and expand on request while streaming', async ({ page }) => {
  await mount(page, { code: { collapseAfter: 20 } });
  await page.evaluate(source => window.view.render(source), fence(22));
  await expect(page.locator('#view .mk-code-toggle')).toHaveCount(0);
  await page.evaluate(source => window.view.render(source, { finalized: false }), fence(30).slice(0, -4));
  const toggle = page.getByRole('button', { name: 'Show all 30 lines' });
  await expect(toggle).toHaveAttribute('aria-expanded', 'false');
  const folded = await page.locator('#view pre').evaluate(pre => pre.getBoundingClientRect().height);
  expect(folded).toBeLessThanOrEqual(20 * 22 + 10 + 1);
  expect(await page.locator('#view pre').evaluate(pre => pre.scrollHeight)).toBeGreaterThan(30 * 22);
  await toggle.click();
  const collapse = page.getByRole('button', { name: 'Collapse' });
  await expect(collapse).toHaveAttribute('aria-expanded', 'true');
  expect(await page.locator('#view pre').evaluate(pre => pre.getBoundingClientRect().height)).toBeGreaterThan(30 * 22);
  // More streamed lines keep the reader's choice.
  await page.evaluate(source => window.view.render(source), fence(40));
  await expect(collapse).toBeVisible();
  await collapse.click();
  await expect(page.getByRole('button', { name: 'Show all 40 lines' })).toBeVisible();
});

test('copying a fence announces Copied and then offers Copy again', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await mount(page, {});
  await page.evaluate(source => window.view.render(source), fence(3));
  await page.locator('#view').getByRole('button', { name: 'Copy code' }).click();
  await expect(page.locator('#view').getByRole('status')).toHaveText('Copied to clipboard');
  await expect(page.locator('#view').getByRole('button', { name: 'Copied' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(fence(3).slice(6, -4));
  await expect(page.locator('#view').getByRole('button', { name: 'Copy code' })).toBeVisible({ timeout: 3000 });
  await expect(page.locator('#view').getByRole('status')).toHaveText('');
});

const patch = 'diff --git a/src/a.ts b/src/a.ts\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -10,4 +10,5 @@ export function a() {\n   keep();\n-  old();\n+  next();\n+  more();\n   tail();\n--- removed comment line\n';

test('diff fences count changes and number old and new lines from hunk headers', async ({ page }) => {
  await mount(page, {});
  await page.evaluate(source => window.view.render(source), '```diff\n' + patch + '```\n');
  await expect(page.locator('#view .mk-diff-stat')).toHaveText('+2 −2');
  await expect(page.locator('#view ins')).toHaveText(['+  next();', '+  more();']);
  await expect(page.locator('#view del')).toHaveText(['-  old();', '--- removed comment line']);
  const rows = await page.locator('#view .mk-line').evaluateAll(lines => lines.map(line => [line.dataset.old ?? '', line.firstElementChild?.dataset.new ?? '']));
  expect(rows.slice(4, 10)).toEqual([['10', '10'], ['11', ''], ['', '11'], ['', '12'], ['12', '13'], ['13', '']]);
});

test('streamed diff fences grow in place and end exactly like a fresh render', async ({ page }) => {
  const result = await page.evaluate(async source => {
    const { MarkitRenderer } = await import('/index.js');
    const make = () => { const element = document.createElement('div'); document.body.append(element); return [element, new MarkitRenderer(element, { code: { lineNumbersFrom: 5, collapseAfter: 4 } })]; };
    const [streamed, live] = make();
    let replaced = 0, section;
    // Once the info string is complete the fence's kind is known; from then on it only grows.
    const body = source.indexOf('```diff\n') + 8;
    for (let at = 2; at < source.length; at += 2) {
      await live.render(source.slice(0, at), { finalized: false });
      const current = streamed.querySelector('section');
      if (at > body + 2 && section && current !== section) replaced++;
      section = current;
    }
    await live.render(source);
    const [fresh, once] = make();
    await once.render(source);
    return { same: streamed.innerHTML.replace(/mk-code-\d+/g, '') === fresh.innerHTML.replace(/mk-code-\d+/g, ''), replaced };
  }, 'Intro\n\n```diff\n' + patch + '```\n\nAfter.\n');
  expect(result).toEqual({ same: true, replaced: 0 });
});

test('the open block keeps its element and settled blocks are never rebuilt while streaming', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const element = document.createElement('div'); document.body.append(element);
    const view = new MarkitRenderer(element);
    const source = '# Title\n\nFirst paragraph is settled.\n\n- one\n- two with **bold** text\n- three and `code`\n\nA long closing paragraph that streams in, word by word, until the end.\n';
    const removed = [];
    new MutationObserver(records => { for (const r of records) if (r.target === element) removed.push(...r.removedNodes); }).observe(element, { childList: true });
    let reused = 0, last;
    for (let at = 1; at <= source.length; at++) {
      await view.render(source.slice(0, at), { finalized: false });
      const open = element.lastElementChild;
      if (open === last) reused++;
      last = open;
    }
    return { reused, removed: removed.length, blocks: element.children.length };
  });
  expect(result.removed).toBe(0);
  expect(result.blocks).toBe(4);
  expect(result.reused).toBeGreaterThan(120);
});

test('DOM updates land at most once per animation frame however fast patches arrive', async ({ page }) => {
  const result = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const element = document.createElement('div'); document.body.append(element);
    const view = new MarkitRenderer(element);
    let frame = 0, ticking = true;
    const tick = () => { frame++; if (ticking) requestAnimationFrame(tick); };
    requestAnimationFrame(tick);
    const frames = [];
    new MutationObserver(() => frames.push(frame)).observe(element, { childList: true, subtree: true, characterData: true });
    const text = 'word '.repeat(600);
    const jobs = [];
    for (let at = 5; at <= text.length; at += 5) {
      jobs.push(view.render(text.slice(0, at), { finalized: false }));
      if (at % 50 === 0) await new Promise(resolve => setTimeout(resolve, 0));
    }
    const results = await Promise.all(jobs);
    ticking = false;
    return { batches: frames.length, frames: new Set(frames).size, text: element.textContent.length, applied: results.filter(r => !r.superseded).length };
  });
  expect(result.batches).toBe(result.frames);
  expect(result.text).toBe(3000);
  expect(result.applied).toBeGreaterThan(0);
});
