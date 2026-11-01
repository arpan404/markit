// While a message streams, its open paragraph can end in markup that is not finished yet: a
// backtick waiting for its pair (on this line or a later one), `[text](url` before the `)`,
// `**` before the closing `**`, a table header before its delimiter row. Rendered as it
// stands, that markup shows as literal characters, and a moment later, when it closes, they
// are taken back: the text flickers. `StreamHold` finds where unfinished markup starts. The
// renderer shows the text before it, and the rest once the markup closes, the paragraph ends
// or the message finishes.
//
// Inline spans continue across soft line breaks and end with their paragraph, as in
// CommonMark. An append is processed from where the last one settled: lines completed since
// then once each, and the open paragraph from its last settled point, so the work follows the
// new text and any span still open, never the history before them.

const FENCE = /^ {0,3}(`{3,}|~{3,})(.*)$/;
const FENCE_AT = / {0,3}(?:`{3,}|~{3,})/y;
const PIPE_AT = / {0,3}\|/y;
const BLANK = /^\s*$/;
/** Lines that are a block of their own: the open paragraph starts after them. */
const SINGLE_LINE_BLOCK = /^ {0,3}(?:#{1,6}(?: |$)|\||(?:=+|-+|(?:[-_*][ \t]*){3,})[ \t]*$)/;
/** Lines that open a block whose paragraph continues on the next lines. */
const BLOCK_START_AT = / {0,3}(?:#{1,6}(?: |$)|\||\s*(?:[-*+]|\d{1,9}[.)])(?: |$))/y;
const QUOTE_AT = / {0,3}>/y;
/** Block markers ahead of a line's inline text: indent, quotes, a list marker, a task box, a heading. */
const LINE_PREFIX = / {0,3}(?:> ?)*(?:(?:[-*+]|\d{1,9}[.)])(?: +|$)(?:\[[ xX]\] +)?|#{1,6}(?: +|$))?/y;
/**
 * A line that is only the start of a block marker whose meaning the next character decides:
 * a fence, a list marker, a table pipe, a rule or heading underline. Such lines are short.
 */
const UNDECIDED_LINE = /^\s*(?:`{1,2}|~{1,2}|\d{1,9}|\|[\s|]*|(?:> ?)*(?:[-*+]|\d{1,9}[.)])\s*|[-=_*](?:\s*[-=_*])*\s*)$/;
const UNDECIDED_MAX = 40;
const AUTOLINK_AT = /[A-Za-z/][^\s<>]*/y;
const SPECIAL = /[\\`[!<*_~$]/g;
const SPACE = /\s/;
const WORD = /[\p{L}\p{N}]/u;
const DIGIT = /[0-9]/;

const at = (pattern, s, index) => { pattern.lastIndex = index; return pattern.test(s); };
const pipeAt = (s, index) => at(PIPE_AT, s, index);

export class StreamHold {
  #length = 0; #shown = 0;
  /** Offset just past the last complete line processed, and the fence open there. */
  #scanned = 0; #fence = null;
  /** Where the open paragraph starts, and whether the line before it was a quote. */
  #unit = 0; #quoted = false;
  /** The open paragraph is settled up to `pos` (scanned from `start`); `closers` lie past it. */
  #checkpoint = { start: -1, pos: 0, closers: new Set() };

  /** How much of `source` to show; `source` is new text, no continuation of earlier text. */
  replace(source) {
    this.#length = 0; this.#shown = 0; this.#scanned = 0; this.#fence = null;
    this.#unit = 0; this.#quoted = false; this.#checkpoint = { start: -1, pos: 0, closers: new Set() };
    return this.append(source);
  }

  /** How much of `source` to show; `source` is the text last given with more appended. */
  append(source) {
    const from = this.#length;
    this.#length = source.length;
    // Only appended text can hold a new line break.
    const open = source.indexOf('\n', from) === -1 ? this.#scanned : source.lastIndexOf('\n') + 1;
    this.#completeLines(source, open);
    const held = this.#fence ? fenceHold(source, open, this.#fence) : this.#heldFrom(source, open);
    // Shown text never shrinks, even if a later character changes what came before.
    this.#shown = Math.max(this.#shown, held === -1 ? source.length : held);
    return this.#shown;
  }

  /** The message finished: everything given so far shows. */
  showAll() { this.#shown = this.#length; }

  #completeLines(source, open) {
    while (this.#scanned < open) {
      const start = this.#scanned, end = source.indexOf('\n', start);
      const line = source.slice(start, end);
      this.#scanned = end + 1;
      const fence = FENCE.exec(line);
      if (this.#fence) {
        if (fence && fence[1][0] === this.#fence.mark && fence[1].length >= this.#fence.length && !fence[2].trim()) this.#fence = null;
        this.#unit = end + 1; continue;
      }
      if (fence && !(fence[1][0] === '`' && fence[2].includes('`'))) {
        this.#fence = { mark: fence[1][0], length: fence[1].length };
        this.#unit = end + 1; continue;
      }
      const quoted = at(QUOTE_AT, source, start);
      if (BLANK.test(line) || SINGLE_LINE_BLOCK.test(line)) this.#unit = end + 1;
      else if (at(BLOCK_START_AT, source, start) || (quoted && !this.#quoted)) this.#unit = start;
      this.#quoted = quoted && !BLANK.test(line);
    }
  }

  /** Where unfinished markup starts in the open paragraph, or -1 when it has none. */
  #heldFrom(source, open) {
    const table = tableHold(source, open);
    if (table !== -1) return table;
    if (source.length - open <= UNDECIDED_MAX && UNDECIDED_LINE.test(source.slice(open))) return open;
    // A line that opens a block starts its own paragraph; an opening fence line waits whole.
    const opensBlock = at(BLOCK_START_AT, source, open) || at(FENCE_AT, source, open) || (at(QUOTE_AT, source, open) && !this.#quoted);
    const start = opensBlock ? open : this.#unit;
    let checkpoint = this.#checkpoint;
    if (checkpoint.start !== start) {
      LINE_PREFIX.lastIndex = start; LINE_PREFIX.test(source);
      checkpoint = this.#checkpoint = { start, pos: LINE_PREFIX.lastIndex, closers: new Set() };
    }
    const scan = inlineHold(source, checkpoint.pos, checkpoint.closers);
    checkpoint.pos = scan.settled;
    for (const closer of checkpoint.closers) if (closer < scan.settled) checkpoint.closers.delete(closer);
    if (scan.held === -1) return -1;
    // What stays of the held line must read as it will: not a lone list marker or table pipe.
    const lineStart = source.lastIndexOf('\n', scan.held - 1) + 1;
    return scan.held - lineStart <= UNDECIDED_MAX && UNDECIDED_LINE.test(source.slice(lineStart, scan.held)) ? lineStart : scan.held;
  }
}

/** Inside a code fence only its closing line can be unfinished: those marks are no code. */
function fenceHold(source, open, fence) {
  if (open === source.length || source.length - open > UNDECIDED_MAX) return -1;
  return new RegExp(`^ {0,3}\\${fence.mark}*$`).test(source.slice(open)) ? open : -1;
}

/** A table shows once its delimiter row is complete; until then its header could be prose. */
function tableHold(source, open) {
  const empty = open === source.length;
  if (!empty && !pipeAt(source, open)) return -1;
  if (open === 0) return empty ? -1 : open;
  const previous = source.lastIndexOf('\n', open - 2) + 1;
  if (!pipeAt(source, previous)) return empty ? -1 : open;
  // `previous` is a header whose delimiter row is still being written (or not started).
  const before = previous === 0 ? -1 : source.lastIndexOf('\n', previous - 2) + 1;
  return before !== -1 && pipeAt(source, before) ? -1 : previous;
}

/**
 * Scans `s` from `from` (everything before it settled) for the first unfinished inline span.
 * Returns where it opens (`held`, -1 if none) and how far the text is `settled`: no later
 * character can change how it reads. Closing runs found ahead are noted in `closers`.
 */
function inlineHold(s, from, closers) {
  const end = s.length;
  let i = from;
  const held = position => ({ held: position, settled: position });
  for (;;) {
    SPECIAL.lastIndex = i;
    const special = SPECIAL.exec(s);
    if (!special) return { held: -1, settled: end };
    i = special.index;
    const c = s[i];
    if (c === '\\') {
      // A trailing backslash escapes whatever comes next, and then disappears.
      if (i + 1 === end) return held(i);
      i += 2; continue;
    }
    if (c === '`') {
      const after = codeSpanEnd(s, i);
      if (after === -1 || after === end) return held(i);
      i = after; continue;
    }
    if (c === '[' || c === '!') {
      if (c === '!' && s[i + 1] !== '[') { if (i + 1 === end) return held(i); i++; continue; }
      const after = linkEnd(s, c === '!' ? i + 1 : i);
      if (after === -1) return held(i);
      i = after; continue;
    }
    if (c === '<') {
      AUTOLINK_AT.lastIndex = i + 1;
      if (i + 1 === end || (AUTOLINK_AT.test(s) && AUTOLINK_AT.lastIndex === end)) return held(i);
      i++; continue;
    }
    const length = runLength(s, i);
    if (closers.has(i)) { closers.delete(i); i += length; continue; }
    const hold = delimiterHold(s, i, length, closers);
    if (hold !== -1) return held(hold);
    i += length;
  }
}

function runLength(s, start) {
  let end = start;
  while (s[end] === s[start]) end++;
  return end - start;
}

/** Just past the code span opening at `start`, or -1 while its closing run has not arrived. */
function codeSpanEnd(s, start) {
  const length = runLength(s, start);
  for (let j = s.indexOf('`', start + length); j !== -1; j = s.indexOf('`', j)) {
    const run = runLength(s, j);
    if (run === length) return j + run;
    j += run;
  }
  return -1;
}

/** Just past a link or bracket opening at `start`, or -1 while it could still become a link. */
function linkEnd(s, start) {
  let depth = 0, close = -1;
  for (let j = start; j < s.length; j++) {
    const c = s[j];
    if (c === '\\') { j++; continue; }
    if (c === '`') { const after = codeSpanEnd(s, j); if (after === -1) return -1; j = after - 1; continue; }
    if (c === '[') depth++;
    else if (c === ']' && --depth === 0) { close = j; break; }
  }
  if (close === -1 || close + 1 === s.length) return -1;
  const next = s[close + 1];
  if (next === '[') { const end = s.indexOf(']', close + 2); return end === -1 ? -1 : end + 1; }
  // Plain brackets: their text reads on, inline markup inside them included.
  if (next !== '(') return start + 1;
  let parens = 0;
  for (let j = close + 1; j < s.length; j++) {
    if (s[j] === '\\') { j++; continue; }
    if (s[j] === '(') parens++;
    else if (s[j] === ')' && --parens === 0) return j + 1;
  }
  return -1;
}

/**
 * An emphasis, strikethrough or math run at `start`: where to hold from when it opens a span
 * that has not closed yet, else -1. A closing run it finds is noted in `closers`.
 */
function delimiterHold(s, start, length, closers) {
  const mark = s[start], before = s[start - 1], next = s[start + length];
  // A run at the very end could still open a span with the next character.
  if (next === undefined) return start;
  if (mark === '~' && length !== 2) return -1;
  if (mark === '$' && (length !== 1 || DIGIT.test(next))) return -1;
  if (SPACE.test(next)) return -1;
  if (mark === '_' && before !== undefined && WORD.test(before)) return -1;
  for (let j = s.indexOf(mark, start + length); j !== -1; j = s.indexOf(mark, j)) {
    const run = runLength(s, j), after = s[j + run];
    const closes = !SPACE.test(s[j - 1]) && (mark !== '_' || after === undefined || !WORD.test(after))
      && (mark !== '$' || run === 1) && (mark !== '~' || run === 2);
    if (closes) {
      // A closing run at the end could still grow, or (math) gain a digit that undoes it.
      if (after === undefined) return start;
      if (mark === '$' && DIGIT.test(after)) { j += run; continue; }
      closers.add(j);
      return -1;
    }
    j += run;
  }
  return start;
}
