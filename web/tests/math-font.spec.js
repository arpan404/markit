import { test, expect } from '@playwright/test';

// Same documents and hashes as `web_math_patches_match_recorded_output` in
// crates/markit/tests/golden.rs, which renders them natively with the full font.
const documents = [
  'Core: $x^2$, $\\alpha_i \\to \\beta^{n+1}$, $\\mathbb{R}^n$ and\n\n$$\n\\sum_{i=1}^{n} \\frac{1}{i^2} = \\frac{\\pi^2}{6} \\quad \\left( \\begin{matrix} a & b \\\\ c & d \\end{matrix} \\right)\n$$\n\nDone.\n',
  'Rare: $\\mathfrak{g} \\oplus \\mathscr{H}$, $\\text{café ☃}$, $\\phantom{\\mathfrak{X}}x$ and\n\n$$\n\\left\\langle \\begin{matrix} 1\\\\2\\\\3\\\\4\\\\5\\\\6\\\\7\\\\8\\\\9 \\end{matrix} \\right\\rangle \\oint_C \\omega\n$$\n',
];
const nativeHashes = ['9f34057f1d5643fb', 'b6d1d30230ed5bb2'];

test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready'); });

test('Wasm math with the core font and the on-demand full font matches native output', async ({ page }) => {
  const result = await page.evaluate(async documents => {
    const wasm = await import('/pkg/markit_wasm.js');
    await wasm.default();
    const fnv = bytes => {
      let hash = 0xcbf29ce484222325n;
      for (const byte of [...bytes, 10]) hash = BigInt.asUintN(64, (hash ^ BigInt(byte)) * 0x100000001b3n);
      return hash.toString(16).padStart(16, '0');
    };
    const out = [];
    for (const document of documents) {
      const stream = wasm.MarkdownStream.web();
      let packed = stream.appendPacked(document, true);
      const missing = wasm.mathFontMissing();
      if (missing) {
        const font = await (await fetch('/pkg/STIXTwoMath-Regular.otf')).arrayBuffer();
        wasm.installMathFont(new Uint8Array(font));
        stream.reset();
        packed = stream.appendPacked(document, true);
      }
      out.push({ missing, hash: fnv(packed) });
    }
    return out;
  }, documents);
  expect(result).toEqual([{ missing: false, hash: nativeHashes[0] }, { missing: true, hash: nativeHashes[1] }]);
});

test('the worker loads the full math font once, only when a formula needs it', async ({ page }) => {
  const fonts = [];
  page.on('request', request => { if (request.url().endsWith('.otf')) fonts.push(request.url()); });
  const before = await page.evaluate(async documents => {
    const { MarkitRenderer } = await import('/index.js');
    const views = documents.map(() => new MarkitRenderer(document.body.appendChild(document.createElement('div'))));
    await views[0].render(documents[0]);
    const before = performance.getEntriesByType('resource').filter(entry => entry.name.endsWith('.otf')).length;
    await Promise.all([views[1].render(documents[1]), views[0].render(documents[0] + '\nMore $\\mathfrak{a}$.\n')]);
    return before;
  }, documents);
  expect(before).toBe(0);
  expect(fonts).toHaveLength(1);
  // Every formula drew; none fell back to its TeX source.
  await expect(page.locator('body > div.markit .mk-math svg')).toHaveCount(4 + 1 + 4);
  await expect(page.locator('body > div.markit', { hasText: 'mathfrak' })).toHaveCount(0);
});
