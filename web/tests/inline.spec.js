import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.locator('#status')).toContainText('Ready'); });

test('inline renderers run the same Wasm on the page and spawn no worker', async ({ page }) => {
  const workers = page.workers().length;
  const result = await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    const bytes = await (await fetch('/pkg/markit_wasm_bg.wasm')).arrayBuffer();
    const make = options => { const element = document.createElement('div'); document.body.append(element); return [element, new MarkitRenderer(element, options)]; };
    const source = '# Inline\n\nText with $x^2$ and `src/a.ts:3`.\n\n```diff\n@@ -1 +1 @@\n-a\n+b\n```\n\n| a | b |\n| - | - |\n| 1 | 2 |\n';
    const [inline, local] = make({ worker: 'inline', wasm: new WebAssembly.Module(bytes) });
    for (let at = 4; at < source.length; at += 4) await local.render(source.slice(0, at), { finalized: false });
    await local.render(source);
    const [worker, remote] = make({});
    await remote.render(source);
    const tokens = await local.highlight('const x = "😀";\n', 'ts');
    return { same: inline.innerHTML.replace(/mk-code-\d+/g, '') === worker.innerHTML.replace(/mk-code-\d+/g, ''), text: tokens.tokens.map(t => t.text).join(''), heading: inline.querySelector('h1')?.textContent };
  });
  expect(result).toEqual({ same: true, text: 'const x = "😀";\n', heading: 'Inline' });
  expect(page.workers()).toHaveLength(workers);
});
