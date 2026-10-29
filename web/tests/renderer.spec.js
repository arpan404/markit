import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready'); });
test('Wasm worker renders code, math, tables, tasks, and reference actions', async ({ page }) => {
  await expect(page.locator('#preview h1')).toHaveText('One shared engine');
  await expect(page.locator('#preview .mk-math')).toHaveCount(2);
  await expect(page.locator('#preview table')).toHaveCount(1);
  await expect(page.locator('#preview input[type=checkbox]')).toHaveCount(3);
  await expect(page.locator('#preview [style*="--mk-dark"]')).not.toHaveCount(0);
  await page.locator('[data-markit-reference="src/hello.ts:12"]').press('Enter');
  await expect(page.getByRole('status')).toContainText('file: src/hello.ts:12');
});
test('raw HTML stays inert and unauthorized images never make network requests', async ({ page }) => {
  const requests = [];
  page.on('request', request => requests.push(request.url()));
  await page.evaluate(() => window.renderer.render('<script>window.injected=1</script>\n\n[bad](javascript:alert(1)) ![remote](https://images.invalid/tracker.png)'));
  await expect(page.locator('#preview script')).toHaveCount(0);
  await expect(page.locator('#preview')).toContainText('<script>');
  expect(await page.evaluate(() => window.injected)).toBeUndefined();
  expect(requests.some(url => url.includes('images.invalid'))).toBe(false);
});
test('streaming preserves settled DOM and resolves references arriving late', async ({ page }) => {
  await page.evaluate(() => window.renderer.render('First paragraph.\n\nOpen tail',{finalized:false}));
  await page.evaluate(() => { window.settled = document.querySelector('#preview .mk-block'); });
  await page.evaluate(() => window.renderer.render('First paragraph.\n\nOpen tail continues\n',{finalized:false}));
  expect(await page.evaluate(() => window.settled === document.querySelector('#preview .mk-block'))).toBe(true);
  await page.evaluate(() => window.renderer.render('[late][ref]\n\nTail\n\n[ref]: https://example.com\n'));
  await expect(page.locator('#preview a')).toHaveAttribute('href','https://example.com');
});
test('large source files create a bounded viewport and exact highlighting', async ({ page }) => {
  await page.getByRole('button',{name:'Load 5,000 lines'}).click();
  await expect(page.locator('.mk-virtual-code')).toBeVisible();
  expect(await page.locator('#preview .mk-line').count()).toBeLessThan(80);
  await page.locator('.mk-virtual-code').evaluate(element => { element.scrollTop = element.scrollHeight; });
  await expect(page.locator('#preview')).toContainText('value_4999');
  const result = await page.evaluate(() => window.renderer.highlight('const x = "😀";\n','ts'));
  expect(result.tokens.map(token=>token.text).join('')).toBe('const x = "😀";\n');
});
test('replacement edits and coalesced jobs show the latest result', async ({ page }) => {
  await page.evaluate(async () => { await Promise.all(Array.from({length:30},(_,i)=>window.renderer.render(`# Revision ${i}`))); });
  await expect(page.locator('#preview h1')).toHaveText('Revision 29');
  await page.evaluate(() => window.renderer.render('Replacement **text**'));
  await expect(page.locator('#preview strong')).toHaveText('text');
});
test('native-sized viewport does not overflow the page', async ({ page }) => {
  await page.setViewportSize({width:390,height:844});
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test('copy controls retain the full source behind virtualized code', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await page.getByRole('button',{name:'Load 5,000 lines'}).click();
  await expect(page.locator('.mk-virtual-code')).toBeVisible();
  await page.getByRole('button',{name:'Copy code'}).click();
  await expect(page.getByRole('button',{name:'Copied'})).toBeVisible();
  const copied = await page.evaluate(() => navigator.clipboard.readText());
  expect(copied.startsWith('let value_0 = 0;\n')).toBe(true);
  expect(copied.endsWith('let value_4999 = 4999;\n')).toBe(true);
});

test('image bytes are resolved by the host and math copies its TeX source', async ({ page }) => {
  await page.evaluate(async () => {
    const { MarkitRenderer } = await import('/index.js');
    window.renderer.destroy();
    window.renderer = new MarkitRenderer(document.querySelector('#preview'), {
      resolveImage: () => new Blob([Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aWQ0AAAAASUVORK5CYII='), c=>c.charCodeAt(0))],{type:'image/png'})
    });
    await window.renderer.render('![dot](attachment.png)\n\nMath \\(x^2\\).');
  });
  await expect(page.locator('#preview img')).toHaveAttribute('src',/^blob:/);
  const copied = await page.evaluate(() => {
    const math = document.querySelector('.mk-math');
    const range = document.createRange(); range.selectNode(math);
    const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range);
    const transfer = new DataTransfer();
    document.querySelector('#preview').dispatchEvent(new ClipboardEvent('copy',{clipboardData:transfer,bubbles:true,cancelable:true}));
    return transfer.getData('text/plain');
  });
  expect(copied).toBe('\\(x^2\\)');
});

test('compact worker highlighting exactly matches legacy tokens and colors', async ({ page }) => {
  const results = await page.evaluate(async () => {
    const wasm = await import('/pkg/markit_wasm.js'); await wasm.default();
    const pairs = [
      ['const café = "😀";\n/* multi\nline */\nfn()', 'ts'],
      ['let x = r###"😀"###;\n', 'rs'],
      ['SELECT name FROM records WHERE x IS NOT NULL', 'sql'],
      ["x = '''multi\nline 😀''' # comment", 'python'],
      ['<div title="😀">hello</div>', 'tsx'],
      ['', 'rust'], ['', 'unknown'], ['😀', 'unknown-😀'],
      ['hello', 'x'.repeat(5000)],
      ['let x = 1;\n'.repeat(3000), 'rust'],
    ];
    const results = [];
    for (const [source, language] of pairs) {
      const expected = JSON.parse(wasm.highlightCode(source, language));
      const actual = await window.renderer.highlight(source, language);
      results.push({ equal: JSON.stringify(actual) === JSON.stringify(expected), preserved: actual.tokens.map(t => t.text).join('') === source });
    }
    return results;
  });
  expect(results.every(result => result.equal && result.preserved)).toBe(true);
});

test('worker highlighting colours a whole large file and unknown languages', async ({ page }) => {
  await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready');
  const result = await page.evaluate(async () => {
    // About 680 KB and 160k runs: far past the 8,192-run fence budget.
    const source = 'const x = 1; // note\n'.repeat(32000) + 'return "end";\n';
    const large = await window.renderer.highlight(source, 'ts');
    const unknown = await window.renderer.highlight('# note\nvalue = "text" 42\n', 'conf-unknown');
    const colour = (code, text) => code.tokens.find(token => token.text === text)?.light;
    return {
      plain: large.plain,
      runs: large.tokens.length,
      preserved: large.tokens.map(t => t.text).join('') === source,
      last: large.tokens.at(-2).text + '|' + large.tokens.at(-2).light,
      unknownPlain: unknown.plain,
      comment: colour(unknown, '# note'),
      string: colour(unknown, '"text"'),
    };
  });
  expect(result.plain).toBe(false);
  expect(result.runs).toBeGreaterThan(100000);
  expect(result.preserved).toBe(true);
  expect(result.last).toBe('"end"|#0a3069');
  expect(result.unknownPlain).toBe(false);
  expect(result.comment).toBe('#6e7781');
  expect(result.string).toBe('#0a3069');
});

test('unavailable WebGPU keeps source, syntax colors, and copying on the text backend', async ({page}) => {
  await page.evaluate(async()=>{
    Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>null},configurable:true});
    const {MarkitRenderer}=await import('/index.js');window.renderer.destroy();
    window.renderer=new MarkitRenderer(document.querySelector('#preview'),{codeRendering:'webgpu'});
    await window.renderer.render('```rust\n'+Array.from({length:500},(_,i)=>`let value_${i} = ${i};`).join('\n')+'\n```');
  });
  const pre=page.locator('pre');await expect(pre).toHaveAttribute('data-markit-backend','dom');
  await expect(page.locator('.mk-gpu-canvas')).toHaveCount(0);
  expect(await pre.locator('span[style*="--mk-light"]').count()).toBeGreaterThan(20);
  await pre.evaluate(element=>{element.scrollTop=element.scrollHeight;});
  await expect(pre).toContainText('value_499');
});

test('WebGPU requests retain Unicode shaping and diff backgrounds through text rendering', async ({page}) => {
  const result=await page.evaluate(async()=>{
    let requests=0;Object.defineProperty(navigator,'gpu',{value:{requestAdapter:async()=>{requests++;return null;}},configurable:true});
    const {MarkitRenderer}=await import('/index.js');window.renderer.destroy();
    window.renderer=new MarkitRenderer(document.querySelector('#preview'),{codeRendering:'webgpu'});
    await window.renderer.render('```rust\n'+('let café = "😀";\n'.repeat(500))+'```');
    const unicode=document.querySelector('pre').textContent.includes('café = "😀"');
    await window.renderer.render('```diff\n'+('-old\n+new\n'.repeat(300))+'```');
    return {unicode,diff:!!document.querySelector('.mk-diff-addition'),requests,canvas:!!document.querySelector('.mk-gpu-canvas')};
  });
  expect(result).toEqual({unicode:true,diff:true,requests:0,canvas:false});
});
