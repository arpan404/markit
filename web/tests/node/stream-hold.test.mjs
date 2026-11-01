// A streaming message must only grow on screen: text markit showed is never taken back when
// the markup around it completes (a backtick that becomes a code span, `[text](url` that
// becomes a link). Run with `npm run test:node`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JSDOM } from 'jsdom';

const wasm = await readFile(new URL('../../pkg/markit_wasm_bg.wasm', import.meta.url));
const { MarkitRenderer, preloadMarkit } = await import('../../index.js');
preloadMarkit({ wasm });

function mount() {
  const { window } = new JSDOM('<!doctype html><body></body>');
  const element = window.document.createElement('div');
  window.document.body.append(element);
  return { element, renderer: new MarkitRenderer(element, { wasm, flavor: 'agent' }) };
}

/** What a reader sees, as text: whitespace runs are one space, trailing space is not yet text. */
const shown = element => element.textContent.replace(/\s+/g, ' ').trimEnd();

/** An agent's answer with every inline and block form that changes once it is closed. */
const answer = `Starting the thread now. The cleanup just found: \`rm -Rf\` with an empty **\`$TMPDIR\`** would delete from the root, see [the cleanup notes](https://example.com/cleanup) and <https://example.com/rm>.

What changed:

1. \`scripts/clean.ts\` checks the variable before it deletes *anything*;
2. the sweep skips ~~every path~~ paths outside \`/tmp/ace-*\`, costing $5 less, since $n^2$ grows fast.

| Case | Result |
| --- | --- |
| empty \`TMPDIR\` | refused |

\`\`\`ts
export function cleanRoot(env) {
  return env.TMPDIR;
}
\`\`\`

Next I'll run \`bun run test\` and report back in [the thread](https://example.com/thread).`;

/** Streams `source` in chunks of the given sizes and returns what was on screen after each. */
async function stream(source, sizes) {
  const { element, renderer } = mount();
  const screens = [];
  for (let at = 0, index = 0; at < source.length; index++) {
    at = Math.min(source.length, at + sizes[index % sizes.length]);
    await renderer.render(source.slice(0, at), { finalized: false });
    screens.push(shown(element));
  }
  await renderer.render(source);
  screens.push(shown(element));
  renderer.destroy();
  return screens;
}

for (const [name, sizes] of [['a character at a time', [1]], ['uneven chunks', [7, 3, 11, 5, 9, 4]]]) {
  test(`a streamed answer never takes back text it showed (${name})`, async () => {
    const screens = await stream(answer, sizes);
    for (let index = 1; index < screens.length; index++) {
      const before = screens[index - 1], after = screens[index];
      assert.ok(after.startsWith(before), `update ${index} took back text:\n  showed …${before.slice(-60)}\n  then   …${after.slice(-60)}`);
    }
    const fresh = mount();
    await fresh.renderer.render(answer);
    assert.equal(screens.at(-1), shown(fresh.element));
    fresh.renderer.destroy();
  });
}

test('unfinished inline markup waits until it closes, then shows whole', async () => {
  const { element, renderer } = mount();
  await renderer.render('The cleanup found: `rm -R', { finalized: false });
  assert.equal(shown(element), 'The cleanup found:');
  await renderer.render('The cleanup found: `rm -Rf` with', { finalized: false });
  assert.equal(element.querySelector('code')?.textContent, 'rm -Rf');
  assert.equal(shown(element), 'The cleanup found: rm -Rf with');
  renderer.destroy();
});

test('held text shows once its paragraph ends, and a finished message shows everything', async () => {
  const { element, renderer } = mount();
  await renderer.render('It is 5` tall and [unsure', { finalized: false });
  assert.equal(shown(element), 'It is 5');
  await renderer.render('It is 5` tall and [unsure\n\nNext', { finalized: false });
  assert.equal(shown(element), 'It is 5` tall and [unsure Next');
  await renderer.render('Ends with `open');
  assert.equal(shown(element), 'Ends with `open');
  renderer.destroy();
});

test('code in an open fence streams as it arrives', async () => {
  const { element, renderer } = mount();
  await renderer.render('```ts\nconst a = `templ', { finalized: false });
  assert.match(element.querySelector('code').textContent, /const a = `templ/);
  renderer.destroy();
});

test('a span left open at a soft line break stays held until it closes', async () => {
  const { element, renderer } = mount();
  await renderer.render('Run `hello\n', { finalized: false });
  assert.equal(shown(element), 'Run');
  await renderer.render('Run `hello\nworld` done', { finalized: false });
  assert.equal(element.querySelector('code')?.textContent, 'hello world');
  assert.equal(shown(element), 'Run hello world done');
  renderer.destroy();
});

test('spans across soft line breaks never take back text, and a paragraph break ends them', async () => {
  const source = 'Run `hello\nworld` and **bold\nacross** lines, see [the\nnotes](https://example.com/n).\n\n'
    + 'A `stray backtick\nthat never closes.\n\nNext *one\nends* here.';
  const screens = await stream(source, [1]);
  for (let index = 1; index < screens.length; index++)
    assert.ok(screens[index].startsWith(screens[index - 1]), `update ${index} took back text:\n  showed …${screens[index - 1].slice(-60)}\n  then   …${screens[index].slice(-60)}`);
  assert.match(screens.at(-1), /A `stray backtick that never closes\. Next one ends here\.$/);
});
