import { test, expect } from '@playwright/test';

const SOURCE = [
  "Recognize $f'(a)=\\lim_{h\\to0}\\frac{f(a+h)-f(a)}{h}$:",
  '',
  '1. $f=\\sin$ at $x$: $=\\cos x$',
  '2. $f=e^x$ at $0$: $=1$ since $e^0=1$',
  '',
  '$$\\int_0^1 x\\,dx = \\frac12$$',
  '',
  'Done.',
].join('\n');
const COPIED = [
  "Recognize $f'(a)=\\lim_{h\\to0}\\frac{f(a+h)-f(a)}{h}$:",
  '',
  '1. $f=\\sin$ at $x$: $=\\cos x$',
  '2. $f=e^x$ at $0$: $=1$ since $e^0=1$',
  '',
  '$$\\int_0^1 x\\,dx = \\frac12$$',
  '',
  'Done.',
].join('\n');
// A solid selection colour, so painted pixels can be told apart from ink and page.
const SELECTED = [0, 200, 0];

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#status')).toContainText('Ready', { timeout: 20000 });
  await page.addStyleTag({ content: `::selection{background:rgb(${SELECTED})} .markit{--mk-selection:rgb(${SELECTED})}` });
  await page.evaluate(source => window.renderer.render(source), SOURCE);
  await expect(page.locator('#preview .mk-math')).toHaveCount(9);
});

/** Drags from just before the first word to just past the last one, as a person would. */
async function selectAll(page) {
  const box = await page.evaluate(() => {
    const preview = document.querySelector('#preview');
    const first = preview.querySelector('p').firstChild, last = [...preview.querySelectorAll('p')].at(-1).firstChild;
    const start = document.createRange(); start.setStart(first, 0); start.setEnd(first, 1);
    const end = document.createRange(); end.setStart(last, last.length - 1); end.setEnd(last, last.length);
    const a = start.getBoundingClientRect(), b = end.getBoundingClientRect();
    return { from: [a.left + 1, a.top + a.height / 2], to: [b.right + 2, b.top + b.height / 2] };
  });
  await page.mouse.move(...box.from);
  await page.mouse.down();
  await page.mouse.move(...box.to, { steps: 8 });
  await page.mouse.up();
}

/** Share of pixels in each box painted exactly the selection colour. */
async function painted(page, boxes) {
  const shot = (await page.screenshot()).toString('base64');
  return page.evaluate(async ({ shot, boxes, selected }) => {
    const bitmap = await createImageBitmap(await (await fetch(`data:image/png;base64,${shot}`)).blob());
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height), context = canvas.getContext('2d');
    context.drawImage(bitmap, 0, 0);
    const scale = bitmap.width / innerWidth;
    return boxes.map(([x, y, width, height]) => {
      const data = context.getImageData(Math.round(x * scale), Math.round(y * scale), Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))).data;
      let hits = 0;
      for (let i = 0; i < data.length; i += 4) if (Math.abs(data[i] - selected[0]) + Math.abs(data[i + 1] - selected[1]) + Math.abs(data[i + 2] - selected[2]) < 12) hits++;
      return hits / (data.length / 4);
    });
  }, { shot, boxes, selected: SELECTED });
}

test('selecting across math paints the formulas and keeps word highlights text-sized', async ({ page }) => {
  await selectAll(page);
  const layout = await page.evaluate(() => {
    const preview = document.querySelector('#preview');
    const rect = node => { const r = node.getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; };
    const word = document.createRange(); const text = preview.querySelector('p').firstChild;
    word.setStart(text, 0); word.setEnd(text, 'Recognize'.length);
    const w = word.getBoundingClientRect(), line = preview.querySelector('p').getBoundingClientRect();
    return {
      formulas: [...preview.querySelectorAll('.mk-math')].map(rect),
      word: [w.left, w.top, w.width, w.height],
      // Inside the tall line box but above the word's own text box.
      aboveWord: [w.left, line.top, w.width, Math.max(1, w.top - line.top - 2)],
      tallLine: w.top - line.top > 6,
    };
  });
  expect(layout.tallLine).toBe(true);
  const [word, aboveWord, ...formulas] = await painted(page, [layout.word, layout.aboveWord, ...layout.formulas]);
  expect(word).toBeGreaterThan(0.4);
  expect(aboveWord).toBe(0);
  for (const share of formulas) expect(share).toBeGreaterThan(0.4);
});

test('copying a selection across math gives the TeX between the words and list markers', async ({ page }) => {
  await selectAll(page);
  await page.keyboard.press('ControlOrMeta+C');
  const copied = await page.evaluate(async () => {
    const [item] = await navigator.clipboard.read();
    const text = type => (item.types.includes(type) ? item.getType(type).then(blob => blob.text()) : '');
    return { plain: await text('text/plain'), html: await text('text/html') };
  });
  expect(copied.plain).toBe(COPIED);
  expect(copied.html.match(/<svg/g) ?? []).toHaveLength(9);
});

test('double-clicking a formula selects all of it; a word still selects the word', async ({ page }) => {
  const formula = page.locator('#preview .mk-math').first();
  await formula.dblclick();
  const whole = await page.evaluate(() => {
    const range = getSelection().getRangeAt(0), math = document.querySelector('#preview .mk-math');
    const around = document.createRange(); around.selectNode(math);
    return range.compareBoundaryPoints(Range.START_TO_START, around) === 0 && range.compareBoundaryPoints(Range.END_TO_END, around) === 0;
  });
  expect(whole).toBe(true);
  await page.keyboard.press('ControlOrMeta+C');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("$f'(a)=\\lim_{h\\to0}\\frac{f(a+h)-f(a)}{h}$");

  const word = await page.evaluate(() => {
    const range = document.createRange(), text = document.querySelector('#preview p').firstChild;
    range.setStart(text, 2); range.setEnd(text, 3);
    const r = range.getBoundingClientRect();
    return [r.left + r.width / 2, r.top + r.height / 2];
  });
  await page.mouse.dblclick(...word);
  expect(await page.evaluate(() => getSelection().toString().trim())).toBe('Recognize');
});
