// Node + jsdom, as in a host's unit tests: no Worker, no requestAnimationFrame.
// Run with `npm run test:node`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';

const wasm = await readFile(new URL('../../pkg/markit_wasm_bg.wasm', import.meta.url));
const { MarkitRenderer, preloadMarkit } = await import('../../index.js');
preloadMarkit({ wasm });

function mount(options = {}) {
  const { window } = new JSDOM('<!doctype html><body></body>');
  const element = window.document.createElement('div');
  window.document.body.append(element);
  return { element, renderer: new MarkitRenderer(element, { wasm, ...options }) };
}

test('renders Markdown in-process when the host has no Worker', async () => {
  assert.equal(typeof Worker, 'undefined');
  const { element, renderer } = mount();
  const patch = await renderer.render('# Hello\n\nSome **bold** text.\n\n```ts title="a.ts"\nconst x = 1;\n```\n');
  assert.equal(patch.superseded, false);
  assert.equal(element.querySelector('h1').textContent, 'Hello');
  assert.equal(element.querySelector('strong').textContent, 'bold');
  assert.equal(element.querySelector('.mk-code-title').textContent, 'a.ts');
  assert.equal(element.querySelector('code').textContent, 'const x = 1;\n');
  renderer.destroy();
});

test('streamed appends keep settled blocks and finish like a fresh render', async () => {
  const source = 'First paragraph.\n\n- one\n- two\n\n```diff\n@@ -1,2 +1,2 @@\n-old\n+new\n same\n```\n\nLast words.\n';
  const streamed = mount(), fresh = mount();
  let first;
  for (let at = 1; at <= source.length; at += 3) {
    await streamed.renderer.render(source.slice(0, at), { finalized: false });
    first ??= streamed.element.firstElementChild;
  }
  await streamed.renderer.render(source);
  await fresh.renderer.render(source);
  assert.equal(streamed.element.firstElementChild, first);
  assert.equal(streamed.element.innerHTML, fresh.element.innerHTML);
  assert.equal(fresh.element.querySelector('.mk-diff-stat').textContent, '+1 −1');
});

test('a newer render supersedes a queued one and highlighting answers asynchronously', async () => {
  const { element, renderer } = mount();
  const jobs = [renderer.render('one'), renderer.render('two'), renderer.render('three')];
  const results = await Promise.all(jobs);
  assert.equal(results[1].superseded, true);
  assert.equal(results[2].superseded, false);
  assert.equal(element.textContent.trim(), 'three');
  const pending = renderer.highlight('let a = 1;\n', 'rust');
  assert.ok(pending instanceof Promise);
  assert.equal((await pending).tokens.map(token => token.text).join(''), 'let a = 1;\n');
});

test('destroying an inline renderer rejects its pending work', async () => {
  const { renderer } = mount();
  const pending = renderer.render('# gone');
  renderer.destroy();
  await assert.rejects(pending, /Renderer disposed/);
  await assert.rejects(renderer.render('x'), /Renderer disposed/);
});

test('the inline engine renders the GitHub flavour and reports ticked task boxes', async () => {
  const tasks = [];
  const { element, renderer } = mount({ flavor: 'github', onTask: task => tasks.push(task) });
  const source = '> [!WARNING]\n> Careful.\n\n- [ ] ship it\n';
  await renderer.render(source);
  assert.equal(element.querySelector('.mk-alert-title').textContent, 'Warning');
  const box = element.querySelector('input[data-markit-task]');
  assert.equal(box.disabled, false);
  box.checked = true;
  box.dispatchEvent(new element.ownerDocument.defaultView.Event('change', { bubbles: true }));
  assert.deepEqual(tasks, [{ index: 0, offset: source.indexOf('[ ]') + 1, checked: true }]);
  const plain = mount();
  await plain.renderer.render(source);
  assert.equal(plain.element.querySelector('.mk-alert'), null);
  renderer.destroy(); plain.renderer.destroy();
});

test('the inline engine passes the agent flavour through: footnotes drawn, issue numbers left as text', async () => {
  const { element, renderer } = mount({ flavor: 'agent' });
  await renderer.render('Step #12 for @octocat[^1]\n\n[^1]: The note.\n');
  assert.equal(element.querySelectorAll('[data-markit-footnote="1"]').length, 1);
  assert.equal(element.querySelector('[data-markit-reference]'), null);
  assert.match(element.querySelector('p').textContent, /^Step #12 for @octocat/);
  renderer.destroy();
});
