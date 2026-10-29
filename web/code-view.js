import { codeLines, firstChangedLine } from './code-lines.js';

/** Fences longer than this draw only the rows in view. */
const DOM_ROWS = 400;
const LINE_HEIGHT = 22;
const COPIED_MS = 1500;
/** A fence only this many lines past `collapseAfter` shows whole; folding it would hide almost nothing. */
const COLLAPSE_SLACK = 2;
let ids = 0;

function element(doc, tag, className) {
  const node = doc.createElement(tag);
  if (className) node.className = className;
  return node;
}

/** Any row with an old or new line number: hunk headers gave numbers. */
function numbered(diff) {
  for (let at = 3; at < diff.length; at += 3) if (diff[at] || diff[at + 1]) return true;
  return false;
}

/**
 * One fenced code block: header (file or language, a diff's +/− counts, Copy
 * with an announced status), highlighted rows, line numbers from
 * `lineNumbersFrom` lines, and a fold after `collapseAfter` lines. Streaming
 * text grows it in place from the first changed line.
 */
export class CodeView {
  #section; #options; #doc; #payload; #lines; #source = '';
  #pre; #code; #added; #removed; #status; #button; #toggle;
  #expanded = false; #virtual = false; #gpuView; #timer; #destroyed = false;

  /** `options`: `{ lineNumbersFrom, collapseAfter, codeRendering, gpu }`. */
  constructor(section, payload, options) {
    this.#section = section;
    this.#options = options;
    this.#doc = section.ownerDocument;
    if (payload) this.#build(payload);
  }

  get section() { return this.#section; }
  /** Full fence text, also when only part of it is drawn. */
  get source() { return this.#source; }

  #build(payload) {
    const doc = this.#doc, { info } = payload;
    const header = element(doc, 'header'), title = element(doc, 'span', 'mk-code-title');
    if (info.file) {
      // The path truncates from its start in CSS; <bdi> keeps it reading left to right.
      title.title = info.file;
      const bdi = element(doc, 'bdi'); bdi.textContent = info.file; title.append(bdi);
    } else title.textContent = info.language || 'text';
    header.append(title);
    if (info.diff) {
      const stat = element(doc, 'span', 'mk-diff-stat');
      this.#added = element(doc, 'span', 'mk-diff-added');
      this.#removed = element(doc, 'span', 'mk-diff-removed');
      stat.append(this.#added, ' ', this.#removed);
      header.append(stat);
    }
    const button = this.#button = element(doc, 'button');
    button.type = 'button'; button.dataset.markitCopy = ''; button.setAttribute('aria-label', 'Copy code'); button.textContent = 'Copy';
    const status = this.#status = element(doc, 'span', 'mk-copy-status');
    status.setAttribute('role', 'status');
    header.append(button, status);
    this.#pre = element(doc, 'pre');
    this.#code = element(doc, 'code', `language-${info.language}`);
    this.#pre.append(this.#code);
    this.#section.append(header, this.#pre);
    this.#payload = payload;
    this.#source = payload.source;
    const lines = codeLines(payload.source, payload.ranges, payload.diff ?? null, doc);
    if (lines.length <= DOM_ROWS) {
      this.#lines = lines;
      this.#code.replaceChildren(...lines.slice(0, lines.length));
    } else this.#virtualize(lines);
    this.#chrome(lines);
  }

  /** Same fence with more (or changed) text: rebuild rows from the first that
   * differs. False when it must be rebuilt instead: its kind changed, or it
   * crossed into the virtualized size. */
  grow(payload) {
    const before = this.#payload, old = this.#lines;
    if (!before || !old || !payload || typeof payload.source !== 'string') return false;
    const a = before.info, b = payload.info;
    if (a.language !== b.language || a.diff !== b.diff || a.file !== b.file) return false;
    const next = codeLines(payload.source, payload.ranges, payload.diff ?? null, this.#doc);
    if (next.length > DOM_ROWS) return false;
    const code = this.#code, keep = firstChangedLine(old, next);
    while (code.children.length > keep) code.lastElementChild.remove();
    if (keep < next.length) code.append(...next.slice(keep, next.length));
    this.#lines = next; this.#payload = payload; this.#source = payload.source;
    this.#chrome(next);
    return true;
  }

  /** Header counts, line-number visibility and the fold, for `lines`. */
  #chrome(lines) {
    const section = this.#section, diff = lines.diff, options = this.#options;
    if (diff) {
      const added = `+${diff[0]}`, removed = `−${diff[1]}`;
      if (this.#added.textContent !== added) this.#added.textContent = added;
      if (this.#removed.textContent !== removed) this.#removed.textContent = removed;
    }
    const numbers = diff ? numbered(diff) : lines.lines >= (options.lineNumbersFrom ?? 1);
    if (numbers) delete section.dataset.numbers; else if (section.dataset.numbers !== 'off') section.dataset.numbers = 'off';
    const limit = options.collapseAfter;
    const long = !this.#virtual && Number.isFinite(limit) && lines.lines > limit + COLLAPSE_SLACK;
    if (!long) {
      if (this.#toggle) { this.#toggle.remove(); this.#toggle = undefined; delete section.dataset.collapsed; }
      return;
    }
    if (!this.#toggle) {
      const toggle = this.#toggle = element(this.#doc, 'button', 'mk-code-toggle');
      toggle.type = 'button'; toggle.dataset.markitToggle = '';
      this.#pre.id ||= `mk-code-${++ids}`;
      toggle.setAttribute('aria-controls', this.#pre.id);
      section.style.setProperty('--mk-collapse-lines', String(limit));
      section.append(toggle);
    }
    const collapsed = !this.#expanded;
    const label = collapsed ? `Show all ${lines.lines} lines` : 'Collapse';
    if (this.#toggle.textContent !== label) this.#toggle.textContent = label;
    this.#toggle.setAttribute('aria-expanded', String(!collapsed));
    if (collapsed) section.dataset.collapsed = ''; else delete section.dataset.collapsed;
  }

  /** Shows the whole fence, or folds it again. */
  toggle() {
    this.#expanded = !this.#expanded;
    if (this.#lines) this.#chrome(this.#lines);
  }

  /** Copy feedback on the button, announced through the status region. */
  copied() {
    clearTimeout(this.#timer);
    const show = done => {
      this.#button.textContent = done ? 'Copied' : 'Copy';
      this.#button.setAttribute('aria-label', done ? 'Copied' : 'Copy code');
      this.#button.toggleAttribute('data-copied', done);
      this.#status.textContent = done ? 'Copied to clipboard' : '';
    };
    show(true);
    this.#timer = setTimeout(() => { if (!this.#destroyed) show(false); }, COPIED_MS);
  }

  /** Large fences keep their rows packed and build only the window in view. */
  #virtualize(lines) {
    this.#virtual = true;
    const pre = this.#pre, code = this.#code, doc = this.#doc;
    pre.dataset.markitBackend = 'dom';
    pre.classList.add('mk-virtual-code');
    const top = element(doc, 'span', 'mk-spacer'), bottom = element(doc, 'span', 'mk-spacer');
    code.style.minWidth = `${Math.min(4096, lines.width) + 7}ch`;
    let rows = lines;
    const update = () => {
      const first = Math.max(0, Math.floor(pre.scrollTop / LINE_HEIGHT) - 8);
      const last = Math.min(rows.length, first + Math.ceil((pre.clientHeight || 320) / LINE_HEIGHT) + 16);
      top.style.height = `${first * LINE_HEIGHT}px`; bottom.style.height = `${(rows.length - last) * LINE_HEIGHT}px`;
      code.replaceChildren(top, ...rows.slice(first, last), bottom);
    };
    pre.addEventListener('scroll', update, { passive: true }); update();
    // Automatic GPU selection remains disabled until rendering benchmarks show
    // a benefit. Hosts can explicitly request it for large source views.
    const gpu = this.#options.gpu;
    if (this.#options.codeRendering !== 'webgpu' || !gpu) return;
    queueMicrotask(() => {
      if (this.#destroyed || !pre.isConnected) return;
      gpu.attach(pre, rows.slice(0, rows.length), next => { rows = next; update(); }, () => { this.#gpuView = undefined; }).then(view => {
        if (!view || view.disposed) return;
        if (this.#destroyed || !pre.isConnected) view.destroy(); else this.#gpuView = view;
      });
    });
  }

  destroy() {
    this.#destroyed = true;
    clearTimeout(this.#timer);
    this.#gpuView?.destroy(); this.#gpuView = undefined;
  }
}
