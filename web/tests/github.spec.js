import { test, expect } from '@playwright/test';

test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.getByRole('status')).toContainText('Ready'); });

/** Renders `source` into a fresh GitHub-flavour renderer; `setup` names the host options. */
async function github(page, source, setup = 'plain') {
  await page.evaluate(async ({ source, setup }) => {
    const { MarkitRenderer } = await import('./index.js');
    window.gh?.destroy();
    document.querySelector('#gh')?.remove();
    const element = Object.assign(document.createElement('div'), { id: 'gh' });
    document.body.append(element);
    window.events = [];
    const png = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII='), c => c.charCodeAt(0));
    const options = {
      flavor: 'github',
      onReference: reference => window.events.push(['reference', reference]),
      onError: error => window.events.push(['error', error.message]),
    };
    if (setup === 'host') Object.assign(options, {
      onTask: task => window.events.push(['task', task]),
      resolveImage: async url => { window.events.push(['image', url]); return new Blob([png], { type: 'image/png' }); },
      resolveMedia: async (url, kind) => { window.events.push(['media', url, kind]); return new Blob(['x'], { type: 'video/mp4' }); },
      renderDiagram: async (language, source) => {
        window.events.push(['diagram', language, source]);
        if (source.includes('broken')) throw new Error('cannot draw');
        return '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><script>window.injected=1</script></svg>';
      },
    });
    window.gh = new MarkitRenderer(element, options);
    await window.gh.render(source);
  }, { source, setup });
}

test('GitHub flavour draws alerts, footnotes, references and only its HTML subset', async ({ page }) => {
  const requests = [];
  page.on('request', request => requests.push(request.url()));
  await github(page, [
    '> [!WARNING]', '> Careful @octocat', '',
    'Shipped :tada: in a1b2c3d, see #12[^1]', '',
    '<details><summary>More</summary>', '', 'hidden **bold**', '', '</details>', '',
    '<p align="center" onclick="window.injected=1" style="color:red">Hi <b>there</b></p>', '',
    '<script>window.injected=1</script>', '',
    '<img src="https://images.invalid/a.png" onerror="window.injected=1">', '',
    '<a href="javascript:window.injected=1">bad</a>', '',
    '[^1]: Since 2.0.',
  ].join('\n'));
  const root = page.locator('#gh');
  await expect(root.locator('.mk-alert-warning .mk-alert-title')).toHaveText('Warning');
  await expect(root).not.toContainText('[!WARNING]');
  await expect(root.locator('details summary')).toHaveText('More');
  await expect(root.locator('details strong')).toHaveText('bold');
  await expect(root.locator('p[align=center] b')).toHaveText('there');
  await expect(root.locator('script, [onclick], [onerror], [style]')).toHaveCount(0);
  await expect(root).toContainText('<script>window.injected=1</script>');
  await expect(root.getByRole('img', { name: 'tada' })).toHaveText('🎉');
  await expect(root.locator('.mk-footnotes li')).toContainText('Since 2.0.');
  await root.getByText('@octocat').click();
  await root.locator('a.mk-commit').click();
  await root.locator('a.mk-issue').click();
  expect(await page.evaluate(() => window.events)).toEqual([
    ['reference', { kind: 'mention', target: 'octocat' }],
    ['reference', { kind: 'commit', target: 'a1b2c3d' }],
    ['reference', { kind: 'issue', target: '#12' }],
  ]);
  await root.locator('[data-markit-footnote="1"]').click();
  await root.locator('[data-markit-footnote-back="1"]').click();
  await root.locator('a', { hasText: 'bad' }).click();
  expect(await page.evaluate(() => [window.injected, location.hash])).toEqual([undefined, '']);
  expect(requests.some(url => url.includes('images.invalid'))).toBe(false);
});

test('task boxes are enabled only for a host that ticks them, which learns the source offset', async ({ page }) => {
  const source = '- [ ] Rebase\n- [x] Lint\n\n```\n- [ ] not a task\n```\n';
  await github(page, source);
  await expect(page.locator('#gh input[type=checkbox]')).toHaveCount(2);
  await expect(page.locator('#gh input[type=checkbox]').first()).toBeDisabled();
  await github(page, source, 'host');
  const box = page.locator('#gh input[data-markit-task="0"]');
  await expect(box).toBeEnabled();
  await box.check();
  const [[kind, task]] = await page.evaluate(() => window.events);
  expect(kind).toBe('task');
  expect(task).toMatchObject({ index: 0, checked: true });
  const next = source.slice(0, task.offset) + 'x' + source.slice(task.offset + 1);
  expect(next).toBe('- [x] Rebase\n- [x] Lint\n\n```\n- [ ] not a task\n```\n');
});

test('diagrams show the host drawing as an inert image and keep the code when drawing fails', async ({ page }) => {
  await github(page, '```mermaid\ngraph TD; A-->B\n```\n\n```mermaid\nbroken\n```\n', 'host');
  const [drawn, failed] = [page.locator('#gh .mk-diagram').first(), page.locator('#gh .mk-diagram').nth(1)];
  await expect(drawn.locator('img.mk-diagram-drawn')).toHaveAttribute('src', /^data:image\/svg\+xml;base64,/);
  await expect(drawn.locator('.mk-codeblock')).toBeHidden();
  await expect(failed.locator('.mk-codeblock')).toBeVisible();
  await expect(failed.locator('img')).toHaveCount(0);
  expect(await page.evaluate(() => window.injected)).toBeUndefined();
  const events = await page.evaluate(() => window.events);
  expect(events.filter(event => event[0] === 'diagram')).toEqual([['diagram', 'mermaid', 'graph TD; A-->B\n'], ['diagram', 'mermaid', 'broken\n']]);
});

test('HTML images, picture sources and videos load only what the host resolves', async ({ page }) => {
  const requests = [];
  page.on('request', request => requests.push(request.url()));
  await github(page, [
    '<picture><source media="(prefers-color-scheme: dark)" srcset="https://media.invalid/dark.png"><img src="https://media.invalid/light.png" alt="Logo"></picture>',
    '',
    '<video src="https://media.invalid/clip.mp4"></video>',
  ].join('\n'), 'host');
  const root = page.locator('#gh');
  await expect(root.locator('img[alt=Logo]')).toHaveAttribute('src', /^blob:/);
  await expect(root.locator('source')).toHaveAttribute('srcset', /^blob:/);
  await expect(root.locator('video')).toHaveAttribute('src', /^blob:/);
  const asked = (await page.evaluate(() => window.events)).filter(event => event[0] !== 'error').map(event => event.slice(1).join(' ')).sort();
  expect(asked).toEqual(['https://media.invalid/clip.mp4 video', 'https://media.invalid/dark.png', 'https://media.invalid/light.png']);
  expect(requests.some(url => url.includes('media.invalid'))).toBe(false);
});

test('a GitHub stream keeps settled blocks and ends equal to a one-shot render', async ({ page }) => {
  const source = '> [!NOTE]\n> Streaming @octocat\n\n- [ ] one\n- [x] two\n\n<details><summary>Logs</summary>\n\n```\nlog\n```\n\n</details>\n\nTail[^n]\n\n[^n]: Note.\n';
  const streamed = await page.evaluate(async source => {
    const { MarkitRenderer } = await import('./index.js');
    const element = document.createElement('div'); document.body.append(element);
    const renderer = new MarkitRenderer(element, { flavor: 'github' });
    for (let at = 1; at < source.length; at += 7) await renderer.render(source.slice(0, at), { finalized: false });
    await renderer.render(source);
    const html = element.innerHTML; renderer.destroy(); return html;
  }, source);
  await github(page, source);
  expect(streamed).toBe(await page.locator('#gh').innerHTML());
});
