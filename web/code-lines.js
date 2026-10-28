import { color } from './highlight.js';

// Keep ranges packed. Only the visible window becomes DOM nodes. Line starts
// are UTF-16 offsets, just like the packed run endpoints from Rust.
export function codeLines(source, words, diff = false) {
  const starts = [0];
  for (let at = source.indexOf('\n'); at !== -1; at = source.indexOf('\n', at + 1)) starts.push(at + 1);
  const firstRun = 3 + words[2];
  const count = (words.length - firstRun) / 3;
  const row = index => {
    const from = starts[index], end = starts[index + 1] ?? source.length;
    const line = document.createElement('span'); line.className = 'mk-line'; line.dataset.line = index + 1;
    let parent = line;
    if (diff && end > from) {
      parent = document.createElement('span');
      const text = source.slice(from, end);
      const kind = /^(\+\+\+|---|@@|diff )/.test(text) ? 'meta' : text[0] === '+' ? 'addition' : text[0] === '-' ? 'deletion' : 'context';
      parent.className = `mk-diff-${kind}`; line.append(parent);
    }
    let lo = 0, hi = count;
    while (lo < hi) { const mid = (lo + hi) >>> 1; if (words[firstRun + mid * 3] <= from) lo = mid + 1; else hi = mid; }
    let at = from;
    for (let run = lo; run < count && at < end; run++) {
      const offset = firstRun + run * 3;
      const to = Math.min(end, words[offset]);
      const span = document.createElement('span');
      span.setAttribute('style', `--mk-light:${color(words[offset + 1])};--mk-dark:${color(words[offset + 2])}`);
      span.textContent = source.slice(at, to); parent.append(span); at = to;
    }
    return line;
  };
  let width = 0;
  for (let i = 0; i < starts.length; i++) width = Math.max(width, (starts[i + 1] ?? source.length) - starts[i]);
  return { length: starts.length, width, row, slice: (from, to) => Array.from({ length: to - from }, (_, i) => row(from + i)) };
}

export function prepareCode(section, payload) {
  const { source, info, ranges } = payload;
  const header = document.createElement('header'), title = document.createElement('span'), button = document.createElement('button');
  title.textContent = info.file ?? info.language; button.type = 'button'; button.dataset.markitCopy = ''; button.setAttribute('aria-label', 'Copy code'); button.textContent = 'Copy';
  header.append(title, button);
  const pre = document.createElement('pre'), code = document.createElement('code'); code.className = `language-${info.language}`;
  pre.append(code); section.append(header, pre);
  return codeLines(source, ranges, info.diff);
}
