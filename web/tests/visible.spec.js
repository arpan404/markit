import { test, expect } from '@playwright/test';

test('packed visible rows match Rust HTML for Unicode, multiline tokens, diff and nested fences', async ({ page }) => {
  await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready');
  const failures = await page.evaluate(async () => {
    const wasm = await import('/pkg/markit_wasm.js'); await wasm.default();
    const { codeLines } = await import('/code-lines.js');
    const fixtures = [
      ['const café = "😀";\n/* multi\nline */\nfn()', 'ts'],
      ['-old\n+new\n@@ header\n context', 'diff'],
      ['', 'rust'], ['\n\n', 'unknown'],
      ['<x a="&">\r\ntext\n', 'html'],
      // 8,000 runs: the largest fence that renderHtml still colours (its budget is 8,192).
      ['let x = 1;\n'.repeat(2000), 'rust'],
    ];
    const failures = [];
    for (const [source, language] of fixtures) {
      const markdown = '```' + language + '\n' + source + '\n```';
      const baseline = document.createElement('div'); baseline.innerHTML = wasm.renderHtml(markdown);
      const fullSource = baseline.querySelector('code').textContent;
      const expected = [...baseline.querySelectorAll('.mk-line')];
      const lines = codeLines(fullSource, wasm.highlightCodePacked(fullSource, language), language === 'diff');
      for (let i = 0; i < expected.length; i++) {
        if (lines.row(i).outerHTML !== expected[i].outerHTML) { failures.push({language, line: i}); break; }
      }
    }
    return failures;
  });
  expect(failures).toEqual([]);
});

test('initial DOM never constructs offscreen code rows and scrolling reconstructs exact last line', async ({ page }) => {
  await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready');
  const count = await page.evaluate(async () => {
    let built = 0;
    const create = document.createElement.bind(document);
    document.createElement = (...args) => { const element = create(...args); if (args[0] === 'span') built++; return element; };
    try { await window.renderer.render('```rust\n' + 'let café = "😀";\n'.repeat(5000) + 'last();\n```'); }
    finally { document.createElement = create; }
    return built;
  });
  expect(count).toBeLessThan(400);
  await page.locator('pre').evaluate(pre => { pre.scrollTop = pre.scrollHeight; });
  await expect(page.locator('pre')).toContainText('last();');
});

test('nested code payloads render exactly like the existing Rust HTML API', async ({ page }) => {
  await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready');
  const equal = await page.evaluate(async () => {
    const wasm = await import('/pkg/markit_wasm.js'); await wasm.default();
    const markdown = '> quote\n>\n> ```ts title="nested.ts"\n> const café = "😀";\n> /* multi\n> line */\n> ```\n\n- item\n\n  ```diff\n  -old\n  +new\n  ```';
    const expected = document.createElement('div'); expected.innerHTML = wasm.renderHtml(markdown);
    await window.renderer.render(markdown);
    return JSON.stringify([...document.querySelectorAll('#preview .mk-codeblock')].map(s => s.innerHTML)) === JSON.stringify([...expected.querySelectorAll('.mk-codeblock')].map(s => s.innerHTML));
  });
  expect(equal).toBe(true);
});
