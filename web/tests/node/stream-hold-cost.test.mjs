// An append to a streaming message costs what the new text costs, not what came before it:
// the hold never rescans the history. Run with `npm run test:node`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import { StreamHold } from '../../stream-hold.js';

const paragraph = n => `Paragraph ${n} has \`code\`, **bold**, a [link](https://example.com/${n}) and\na second line.\n\n`;
function history(bytes) {
  let text = '';
  for (let n = 0; text.length < bytes; n++) text += paragraph(n);
  return text;
}
/** Deltas a provider streams into the open paragraph: words, spans, line and paragraph breaks. */
const deltas = Array.from({ length: 3000 }, (_, i) => [' word', ' `co', 'de`', ' **b', 'old**', '\n', ' more', '\n\nNew'][i % 8]);

let sink = 0;

/** Milliseconds `append` spends on every delta after `before`, best of three. */
function streamAfter(before) {
  let best = Infinity;
  for (let round = 0; round < 3; round++) {
    const hold = new StreamHold();
    let source = before, spent = 0;
    hold.replace(source);
    for (const delta of deltas) {
      source += delta;
      // Building the string is the caller's cost: V8 flattens a concatenation on its first
      // full read, so read it once here (kept, so the read is not optimized away).
      sink += source.indexOf('\u0000');
      const started = performance.now();
      hold.append(source);
      spent += performance.now() - started;
    }
    best = Math.min(best, spent);
  }
  return best;
}

test('an append costs the same after 20 KB of history as after 2 MB', () => {
  streamAfter(history(20_000)); // warm up
  const short = streamAfter(history(20_000));
  const long = streamAfter(history(2_000_000));
  // Rescanning the history would make the long stream about a hundred times slower.
  assert.equal(sink < 0, true);
  assert.ok(long < short * 4 + 5, `3,000 appends took ${short.toFixed(1)} ms after 20 KB and ${long.toFixed(1)} ms after 2 MB`);
});

test('a replacement still reads the whole source', () => {
  const hold = new StreamHold();
  hold.append('Old text with `code');
  assert.equal(hold.replace('New `text'), 'New '.length);
  assert.equal(hold.append('New `text` and more'), 'New `text` and more'.length);
});
