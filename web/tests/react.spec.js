import { test, expect } from '@playwright/test';

const messages = n => Array.from({ length: n }, (_, i) => ({ id: `m${i}`, text: `## Message ${i}\n\nSome **bold** text and \`code\`.\n\n\`\`\`ts\nconst value${i} = ${i};\n\`\`\`\n\nMath \\(x^${i % 9}\\).\n` }));

test.beforeEach(async ({ page }) => {
  page.on('pageerror', error => { throw error; });
  await page.goto('/tests/react/index.html');
  await page.waitForFunction(() => window.ready);
});

for (const strict of [false, true]) {
  test(`mounting many messages uses one worker${strict ? ' in StrictMode' : ''}`, async ({ page }) => {
    await page.evaluate(([list, strict]) => window.app.mountTranscript(list, { strict }), [messages(120), strict]);
    await expect(page.locator('[data-id] h2')).toHaveCount(120);
    await expect(page.locator('[data-id="m119"] .mk-math')).toHaveCount(1);
    await expect(page.locator('[data-id="m7"] pre')).toContainText('const value7 = 7;');
    expect(page.workers()).toHaveLength(1);
    // Each message rendered its text once even though StrictMode mounts twice.
    for (const id of ['m0', 'm60', 'm119']) expect(await page.evaluate(id => window.app.renders(id), id)).toBe(1);
    expect(await page.locator('[data-id] .mk-block').count()).toBeGreaterThan(120 * 3);
  });
}

test('streaming a message through props keeps settled blocks and finishes exactly', async ({ page }) => {
  await page.evaluate(() => window.app.mountTranscript([{ id: 'live', text: '', done: false }], { strict: true }));
  const full = 'First paragraph.\n\n```diff\n-old\n+new\n```\n\n' + Array.from({ length: 60 }, (_, i) => `Line ${i} with $x_${i}$ math.`).join('\n\n') + '\n\nThe end.';
  let text = '';
  for (let i = 0; i < full.length; i += 7) {
    text = full.slice(0, i + 7);
    await page.evaluate(text => window.app.setMessages([{ id: 'live', text, done: false }]), text);
    if (i === 70) {
      await expect(page.locator('[data-id="live"] pre')).toHaveCount(1);
      await page.evaluate(() => { window.first = document.querySelector('[data-id="live"] .mk-block'); });
    }
  }
  await page.evaluate(text => window.app.setMessages([{ id: 'live', text, done: true }]), full);
  await expect(page.locator('[data-id="live"]')).toContainText('The end.');
  await expect(page.locator('[data-id="live"] .mk-math')).toHaveCount(60);
  expect(await page.evaluate(() => !!window.first && window.first === document.querySelector('[data-id="live"] .mk-block'))).toBe(true);
  // The streamed DOM matches a fresh render of the final text.
  const streamed = await page.locator('[data-id="live"]').innerHTML();
  await page.evaluate(text => window.app.mountTranscript([{ id: 'fresh', text }]), full);
  await expect(page.locator('[data-id="fresh"]')).toContainText('The end.');
  expect(await page.locator('[data-id="fresh"]').innerHTML()).toBe(streamed);
});

test('imperative appends stream without React re-renders', async ({ page }) => {
  await page.evaluate(() => window.app.mountImperative(true));
  await page.evaluate(async () => {
    for (const token of ['# Live', ' title\n\n', 'Some ', '**bold**', ' text.\n\n```rs\n', 'fn main() {}\n', '```\n']) window.stream.append(token);
    await window.stream.finish();
  });
  await expect(page.locator('[data-id="imperative"] h1')).toHaveText('Live title');
  await expect(page.locator('[data-id="imperative"] pre')).toContainText('fn main() {}');
});

test('unmount destroys renderers, frees streams and keeps the shared worker', async ({ page }) => {
  await page.evaluate(list => window.app.mountTranscript(list), messages(30));
  await expect(page.locator('[data-id] h2')).toHaveCount(30);
  // Unmount while a large render is in flight: nothing may throw or render afterwards.
  await page.evaluate(() => {
    window.app.setMessages([{ id: 'big', text: '```rust\n' + 'let a = 1;\n'.repeat(30000) + '```\n' }]);
    window.app.unmount();
  });
  await page.waitForTimeout(300);
  expect(await page.locator('#root').innerHTML()).toBe('');
  expect(page.workers()).toHaveLength(1);
  await page.evaluate(list => window.app.mountTranscript(list), messages(3));
  await expect(page.locator('[data-id] h2')).toHaveCount(3);
});

test('lazy messages render as plain text until scrolled near', async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 600 });
  await page.evaluate(list => window.app.mountTranscript(list, { lazy: '0px' }), messages(80));
  await expect(page.locator('[data-id="m0"] h2')).toHaveCount(1);
  await expect(page.locator('[data-id="m79"]')).toHaveClass(/mk-pending/);
  await expect(page.locator('[data-id="m79"]')).toContainText('## Message 79');
  await page.locator('[data-id="m79"]').scrollIntoViewIfNeeded();
  await expect(page.locator('[data-id="m79"] h2')).toHaveText('Message 79');
  await expect(page.locator('[data-id="m79"]')).not.toHaveClass(/mk-pending/);
});
