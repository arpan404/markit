import { color } from './highlight.js';

const KINDS = ['context', 'addition', 'deletion', 'hunk', 'meta'];
const WRAPPERS = ['span', 'ins', 'del', 'span', 'span'];

// Keep ranges packed. Only the visible window becomes DOM nodes. Line starts
// are UTF-16 offsets, just like the packed run endpoints from Rust. `diff` is
// the packed rows from Rust (`[additions, deletions, kind, old+1, new+1, ...]`)
// for a diff fence; it decides each row's tint, <ins>/<del> and line numbers.
export function codeLines(source, words, diff = null, doc = globalThis.document) {
  const starts = [0];
  for (let at = source.indexOf('\n'); at !== -1; at = source.indexOf('\n', at + 1)) starts.push(at + 1);
  const firstRun = 3 + words[2];
  const count = (words.length - firstRun) / 3;
  const diffRows = diff ? (diff.length - 2) / 3 : 0;
  const runAt = from => {
    let lo = 0, hi = count;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (words[firstRun + mid * 3] <= from) lo = mid + 1; else hi = mid; }
    return lo;
  };
  const row = index => {
    const from = starts[index], end = starts[index + 1] ?? source.length;
    const line = doc.createElement('span');
    let parent = line;
    if (!diff) { line.className = 'mk-line'; line.dataset.line = index + 1; }
    else if (index < diffRows) {
      const at = 2 + index * 3, kind = diff[at], old = diff[at + 1], next = diff[at + 2];
      line.className = `mk-line mk-diff-${KINDS[kind] ?? 'context'}`;
      if (old) line.dataset.old = old - 1;
      parent = doc.createElement(WRAPPERS[kind] ?? 'span');
      if (next) parent.dataset.new = next - 1;
      line.append(parent);
    } else line.className = 'mk-line mk-diff-context';
    let at = from;
    for (let run = runAt(from); run < count && at < end; run++) {
      const offset = firstRun + run * 3;
      const to = Math.min(end, words[offset]);
      const span = doc.createElement('span');
      span.setAttribute('style', `--mk-light:${color(words[offset + 1])};--mk-dark:${color(words[offset + 2])}`);
      span.textContent = source.slice(at, to); parent.append(span); at = to;
    }
    return line;
  };
  let width = 0;
  for (let i = 0; i < starts.length; i++) width = Math.max(width, (starts[i + 1] ?? source.length) - starts[i]);
  const ends = source === '' || source.endsWith('\n');
  return {
    length: starts.length,
    /** Lines with text, not counting the empty one after a final newline. */
    lines: starts.length - (ends ? 1 : 0),
    width, row, slice: (from, to) => Array.from({ length: to - from }, (_, i) => row(from + i)),
    source, words, starts, firstRun, count, runAt, diff, diffRows,
  };
}

function sameDiffRow(a, b, index) {
  if (!a.diff !== !b.diff) return false;
  if (!a.diff) return true;
  if ((index < a.diffRows) !== (index < b.diffRows)) return false;
  if (index >= a.diffRows) return true;
  const at = 2 + index * 3;
  return a.diff[at] === b.diff[at] && a.diff[at + 1] === b.diff[at + 1] && a.diff[at + 2] === b.diff[at + 2];
}

/** First line whose text, colors or diff row differ between two `codeLines` results. */
export function firstChangedLine(a, b) {
  const lines = Math.min(a.length, b.length);
  for (let index = 0; index < lines; index++) {
    const from = a.starts[index], end = a.starts[index + 1] ?? a.source.length;
    if (from !== b.starts[index] || end !== (b.starts[index + 1] ?? b.source.length)) return index;
    if (a.source.slice(from, end) !== b.source.slice(from, end)) return index;
    if (!sameDiffRow(a, b, index)) return index;
    let ra = a.runAt(from), rb = b.runAt(from), at = from;
    while (at < end) {
      if (ra >= a.count || rb >= b.count) return index;
      const oa = a.firstRun + ra * 3, ob = b.firstRun + rb * 3;
      const ta = Math.min(end, a.words[oa]), tb = Math.min(end, b.words[ob]);
      if (ta !== tb || a.words[oa + 1] !== b.words[ob + 1] || a.words[oa + 2] !== b.words[ob + 2]) return index;
      at = ta; ra++; rb++;
    }
  }
  return lines;
}
