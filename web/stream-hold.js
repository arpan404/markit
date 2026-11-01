// While a message streams, its last line can end in markup that is not finished yet: a
// backtick waiting for its pair, `[text](url` before the `)`, `**` before the closing `**`,
// a table header before its delimiter row. Rendered as it stands, that markup shows as
// literal characters, and a moment later, when it closes, they are taken back: the line
// flickers. `StreamHold` finds where unfinished markup starts. The renderer shows the text
// before it, and the rest once the markup closes, the line ends or the message finishes.
// Each update scans only the lines that completed since the last one and the open line.

const FENCE = /^ {0,3}(`{3,}|~{3,})(.*)$/;
const PIPE_ROW = /^ {0,3}\|/;
/** Block markers ahead of a line's inline text: indent, quotes, a list marker, a task box, a heading. */
const LINE_PREFIX = /^ {0,3}(?:> ?)*(?:(?:[-*+]|\d{1,9}[.)])(?: +|$)(?:\[[ xX]\] +)?|#{1,6}(?: +|$))?/;
/**
 * A line that is only the start of a block marker whose meaning the next character decides:
 * a fence, a list marker, a table pipe, a rule or heading underline.
 */
const UNDECIDED_LINE = /^\s*(?:`{1,2}|~{1,2}|\d{1,9}|\|[\s|]*|(?:> ?)*(?:[-*+]|\d{1,9}[.)])\s*|[-=_*](?:\s*[-=_*])*\s*)$/;
const SPACE = /\s/;
const WORD = /[\p{L}\p{N}]/u;
const DIGIT = /[0-9]/;

export class StreamHold {
  #source = ''; #shown = 0;
  /** Offset just past the last complete line scanned, and the fence open there. */
  #scanned = 0; #fence = null;

  /** How much of `source`, a message still streaming, to show now. */
  shownLength(source) {
    if (!source.startsWith(this.#source)) { this.#scanned = 0; this.#fence = null; this.#shown = 0; }
    this.#source = source;
    const open = source.lastIndexOf('\n') + 1;
    this.#scanFences(source, open);
    const held = this.#fence ? fenceHold(source, open, this.#fence) : heldFrom(source, open);
    // Shown text never shrinks, even if a later character changes what came before.
    this.#shown = Math.max(this.#shown, held === -1 ? source.length : held);
    return this.#shown;
  }

  /** `source` was shown whole (the message finished). */
  showAll(source) {
    this.shownLength(source);
    this.#shown = source.length;
  }

  #scanFences(source, open) {
    while (this.#scanned < open) {
      const end = source.indexOf('\n', this.#scanned);
      const line = source.slice(this.#scanned, end);
      this.#scanned = end + 1;
      const fence = FENCE.exec(line);
      if (!fence) continue;
      const [, marks, rest] = fence;
      if (!this.#fence) {
        if (!(marks[0] === '`' && rest.includes('`'))) this.#fence = { mark: marks[0], length: marks.length };
      } else if (marks[0] === this.#fence.mark && marks.length >= this.#fence.length && !rest.trim()) this.#fence = null;
    }
  }
}

/** Inside a code fence only its closing line can be unfinished: those marks are no code. */
function fenceHold(source, open, fence) {
  const line = source.slice(open);
  return line && new RegExp(`^ {0,3}\\${fence.mark}*$`).test(line) ? open : -1;
}

/** Where unfinished markup starts in the open line (from `open`), or -1 when it has none. */
function heldFrom(source, open) {
  const line = source.slice(open);
  const previous = lineBefore(source, open);
  const table = tableHold(source, open, line, previous);
  if (table !== -1) return table;
  if (UNDECIDED_LINE.test(line)) return open;
  const at = inlineHold(line, LINE_PREFIX.exec(line)[0].length);
  // What stays of the line must read as it will: not a lone list marker or table pipe.
  return at === -1 ? -1 : UNDECIDED_LINE.test(line.slice(0, at)) ? open : open + at;
}

function lineBefore(source, open) {
  if (open === 0) return undefined;
  const start = source.lastIndexOf('\n', open - 2) + 1;
  return { start, text: source.slice(start, open - 1) };
}

/** A table shows once its delimiter row is complete; until then its header could be prose. */
function tableHold(source, open, line, previous) {
  if (PIPE_ROW.test(line) && !(previous && PIPE_ROW.test(previous.text))) return open;
  if (!previous || !PIPE_ROW.test(previous.text)) return -1;
  const before = lineBefore(source, previous.start);
  // `previous` is a header whose delimiter row is still being written (or not started).
  if (!(before && PIPE_ROW.test(before.text)) && (line === '' || PIPE_ROW.test(line)))
    return previous.start;
  return -1;
}

/** Where the first unfinished inline span of `line` opens, scanning from `from`; -1 if none. */
function inlineHold(line, from) {
  const closers = new Set();
  for (let i = from; i < line.length;) {
    const c = line[i];
    if (c === '\\') { i += 2; continue; }
    if (c === '`') {
      const after = codeSpanEnd(line, i);
      if (after === -1 || after === line.length) return i;
      i = after; continue;
    }
    if (c === '[' || (c === '!' && line[i + 1] === '[')) {
      const after = linkEnd(line, c === '!' ? i + 1 : i);
      if (after === -1) return i;
      i = after; continue;
    }
    if (c === '<') {
      if (/^(?:[A-Za-z/][^\s<>]*)?$/.test(line.slice(i + 1))) return i;
      i++; continue;
    }
    if (c === '*' || c === '_' || c === '~' || c === '$') {
      const length = runLength(line, i);
      if (closers.has(i)) { i += length; continue; }
      const held = delimiterHold(line, i, length, closers);
      if (held !== -1) return held;
      i += length; continue;
    }
    i++;
  }
  return -1;
}

function runLength(line, at) {
  let end = at;
  while (line[end] === line[at]) end++;
  return end - at;
}

/** Just past the code span opening at `at`, or -1 while its closing run has not arrived. */
function codeSpanEnd(line, at) {
  const length = runLength(line, at);
  for (let j = line.indexOf('`', at + length); j !== -1; j = line.indexOf('`', j)) {
    const run = runLength(line, j);
    if (run === length) return j + run;
    j += run;
  }
  return -1;
}

/** Just past a link or bracket opening at `at`, or -1 while it could still become a link. */
function linkEnd(line, at) {
  let depth = 0, close = -1;
  for (let j = at; j < line.length; j++) {
    const c = line[j];
    if (c === '\\') { j++; continue; }
    if (c === '`') { const after = codeSpanEnd(line, j); if (after === -1) return -1; j = after - 1; continue; }
    if (c === '[') depth++;
    else if (c === ']' && --depth === 0) { close = j; break; }
  }
  if (close === -1 || close + 1 === line.length) return -1;
  const next = line[close + 1];
  if (next === '[') { const end = line.indexOf(']', close + 2); return end === -1 ? -1 : end + 1; }
  // Plain brackets: their text reads on, inline markup inside them included.
  if (next !== '(') return at + 1;
  let parens = 0;
  for (let j = close + 1; j < line.length; j++) {
    if (line[j] === '\\') { j++; continue; }
    if (line[j] === '(') parens++;
    else if (line[j] === ')' && --parens === 0) return j + 1;
  }
  return -1;
}

/**
 * An emphasis, strikethrough or math run at `at`: where to hold from when it opens a span
 * that has not closed yet, else -1. A closing run it finds is noted in `closers`.
 */
function delimiterHold(line, at, length, closers) {
  const mark = line[at], before = line[at - 1], next = line[at + length];
  // A run at the very end could still open a span with the next character.
  if (next === undefined) return at;
  if (mark === '~' && length !== 2) return -1;
  if (mark === '$' && (length !== 1 || DIGIT.test(next))) return -1;
  if (SPACE.test(next)) return -1;
  if (mark === '_' && before !== undefined && WORD.test(before)) return -1;
  for (let j = line.indexOf(mark, at + length); j !== -1; j = line.indexOf(mark, j)) {
    const run = runLength(line, j), after = line[j + run];
    const closes = !SPACE.test(line[j - 1]) && (mark !== '_' || after === undefined || !WORD.test(after))
      && (mark !== '$' || run === 1) && (mark !== '~' || run === 2);
    if (closes) {
      // A closing run at the end could still grow, or (math) gain a digit that undoes it.
      if (after === undefined) return at;
      if (mark === '$' && DIGIT.test(after)) { j += run; continue; }
      closers.add(j);
      return -1;
    }
    j += run;
  }
  return at;
}
