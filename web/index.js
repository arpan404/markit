import { unpackHighlight } from './highlight.js';
import { prepareCode, codeLines, firstChangedLine } from './code-lines.js';
import { GpuCodeRenderer } from './gpu-code.js';

/** One Web Worker and Wasm instance serves every renderer created with the
 * same worker URL. Each renderer owns a stream inside it; requests are
 * multiplexed by id and answered in order. */
class Engine {
  #url; #worker; #pending = new Map(); #id = 0; #streams = 0; #shared;
  generation = 0;
  constructor(url, shared) { this.#url = url; this.#shared = shared; }
  get shared() { return this.#shared; }
  #start() {
    this.#worker?.terminate();
    this.generation++;
    const worker = new Worker(this.#url, { type: 'module', name: 'markit' });
    worker.onmessage = ({ data }) => {
      const pending = this.#pending.get(data.id);
      if (!pending) return;
      this.#pending.delete(data.id);
      if (data.error) pending.reject(Object.assign(new Error(data.error), { code: data.code })); else pending.resolve(data.result);
    };
    worker.onerror = event => {
      event.preventDefault?.();
      const error = new Error(event.message || 'Markdown worker failed');
      for (const pending of this.#pending.values()) pending.reject(error);
      this.#pending.clear();
      if (this.#worker === worker) { worker.terminate(); this.#worker = undefined; }
    };
    this.#worker = worker;
  }
  /** Starts the worker on first use, or again after it failed. */
  ensure() { if (!this.#worker) this.#start(); return this.generation; }
  stream() { return ++this.#streams; }
  pending(owner) { let count = 0; for (const job of this.#pending.values()) if (job.owner === owner) count++; return count; }
  call(owner, type, fields) {
    this.ensure();
    return new Promise((resolve, reject) => {
      const id = ++this.#id;
      this.#pending.set(id, { resolve, reject, owner });
      this.#worker.postMessage({ id, type, ...fields });
    });
  }
  /** Rejects the owner's requests and frees its stream in the worker. */
  release(owner, stream) {
    for (const [id, job] of this.#pending) {
      if (job.owner !== owner) continue;
      this.#pending.delete(id); job.reject(new Error('Renderer disposed'));
    }
    if (this.#worker && stream !== undefined) this.#worker.postMessage({ type: 'release', stream });
  }
  terminate() {
    this.#worker?.terminate(); this.#worker = undefined;
    for (const job of this.#pending.values()) job.reject(new Error('Markdown worker terminated'));
    this.#pending.clear();
  }
}
const engines = new Map();
function engineFor(options) {
  const url = String(options.workerURL ?? new URL('./worker.js', import.meta.url));
  if (options.worker === 'dedicated') return new Engine(url, false);
  let engine = engines.get(url);
  if (!engine) { engine = new Engine(url, true); engines.set(url, engine); }
  return engine;
}
/** Terminates shared workers. Live renderers start a new one on their next request. */
export function terminateMarkitWorkers() {
  for (const engine of engines.values()) engine.terminate();
  engines.clear();
}

/** One in-flight render and one queued replacement keep typing ahead of parsing.
 * Parsing and highlighting always execute in the Rust WebAssembly worker. */
export class MarkitRenderer {
  #element; #engine; #stream; #generation = 0; #sent; #revision = 0;
  #running = false; #queued; #destroyed = false; #options; #code = new WeakMap();
  #html = []; #dirty = Infinity; #payloads = new WeakMap(); #lines = new WeakMap();
  #urls = new Map(); #imageJobs = new Map(); #listeners = new AbortController();
  #gpu = new GpuCodeRenderer(); #gpuViews = new Map();
  constructor(element, options = {}) {
    this.#element = element;
    this.#options = options;
    element.classList.add('markit');
    this.#engine = engineFor(options);
    this.#stream = this.#engine.stream();
    const signal = this.#listeners.signal;
    element.addEventListener('click', event => this.#activate(event), { signal });
    element.addEventListener('keydown', event => {
      if (event.key === 'Enter' && event.target.closest('[data-markit-reference]')) this.#activate(event);
    }, { signal });
    element.addEventListener('copy', event => {
      const selection = element.ownerDocument.getSelection();
      if (!selection?.rangeCount) return;
      const fragment = selection.getRangeAt(0).cloneContents();
      for (const formula of fragment.querySelectorAll('[data-tex]')) formula.replaceWith(formula.dataset.tex);
      if (fragment.querySelector('svg')) return;
      event.clipboardData?.setData('text/plain', fragment.textContent ?? '');
      event.preventDefault();
    }, { signal });
  }
  #rpc(type, fields) {
    if (this.#destroyed) return Promise.reject(new Error('Renderer disposed'));
    if (this.#engine.pending(this) >= 8) return Promise.reject(new Error('Too many pending jobs'));
    return this.#engine.call(this, type, fields);
  }
  /** Sends only the appended text when the source grew from the last request. */
  async #renderPatch(source, finalized) {
    for (let attempt = 0; ; attempt++) {
      const generation = this.#engine.ensure();
      if (generation !== this.#generation) { this.#generation = generation; this.#sent = undefined; this.#dirty = 0; }
      const sent = this.#sent;
      const fields = sent !== undefined && source.startsWith(sent)
        ? { stream: this.#stream, append: source.slice(sent.length), finalized }
        : { stream: this.#stream, source, finalized };
      this.#sent = source;
      try { return await this.#rpc('render', fields); }
      catch (error) {
        this.#sent = undefined;
        if (error.code !== 'resync' || attempt > 0) throw error;
      }
    }
  }
  render(source, { finalized = true } = {}) {
    if (this.#destroyed) return Promise.reject(new Error('Renderer disposed'));
    if (typeof source !== 'string') return Promise.reject(new TypeError('Markdown must be a string'));
    const revision = ++this.#revision;
    return new Promise((resolve, reject) => {
      if (this.#queued) this.#queued.resolve({ superseded: true });
      this.#queued = { source, finalized, revision, resolve, reject };
      this.#drain();
    });
  }
  async #drain() {
    if (this.#running || !this.#queued) return;
    this.#running = true;
    const job = this.#queued;
    this.#queued = undefined;
    try {
      const patch = await this.#renderPatch(job.source, job.finalized);
      for (const block of patch.blocks) {
        for (const code of block.codes ?? []) {
          if (typeof code.source !== 'string') code.source = job.source.slice(code.start, code.end);
        }
      }
      this.#html.splice(patch.replace_from, this.#html.length - patch.replace_from, ...patch.blocks);
      this.#dirty = Math.min(this.#dirty, patch.replace_from);
      if (job.revision === this.#revision) {
        this.#apply({ replace_from: this.#dirty, blocks: this.#html.slice(this.#dirty) });
        this.#dirty = Infinity;
      }
      job.resolve({ ...patch, blocks: patch.blocks.map(block => block.html), superseded: job.revision !== this.#revision });
    } catch (error) { job.reject(error); }
    finally { this.#running = false; this.#drain(); }
  }
  async highlight(source, language, { grammar = false } = {}) {
    const result = await this.#rpc('highlight', { source, language, grammar });
    return grammar ? result : unpackHighlight(source, result);
  }
  #apply(patch) {
    if (this.#destroyed) return;
    let from = patch.replace_from, blocks = patch.blocks;
    const open = this.#element.children[from];
    if (open && blocks.length && this.#grow(open, blocks[0])) { from++; blocks = blocks.slice(1); }
    while (this.#element.children.length > from) {
      const block = this.#element.lastElementChild;
      for (const pre of block.querySelectorAll('pre')) {
        this.#gpuViews.get(pre)?.destroy(); this.#gpuViews.delete(pre);
      }
      for (const image of block.querySelectorAll('img')) {
        this.#imageJobs.get(image)?.abort(); this.#imageJobs.delete(image);
        const url = this.#urls.get(image);
        if (url) { URL.revokeObjectURL(url); this.#urls.delete(image); }
      }
      block.remove();
    }
    const fragment = document.createDocumentFragment();
    for (const payload of blocks) {
      const block = document.createElement('div');
      block.className = 'mk-block';
      block.innerHTML = payload.html;
      this.#payloads.set(block, payload);
      for (const section of block.querySelectorAll('.mk-codeblock')) this.#prepareCode(section, payload.codes?.[Number(section.dataset.markitCode)]);
      fragment.append(block);
    }
    this.#element.append(fragment);
    if (this.#options.resolveImage) {
      for (const image of this.#element.querySelectorAll('img[data-markit-src]:not([data-requested])')) this.#resolveImage(image);
    }
  }
  /** An open block whose markup is unchanged and whose code fences only changed
   * text keeps its element; only code lines from the first changed line are rebuilt.
   * Streaming a fence then costs the new lines, not the whole fence. */
  #grow(block, payload) {
    const previous = this.#payloads.get(block);
    if (!previous || previous.html !== payload.html || !payload.codes?.length) return false;
    const sections = [...block.querySelectorAll('.mk-codeblock')];
    if (sections.length !== payload.codes.length) return false;
    const updates = [];
    for (const section of sections) {
      const index = Number(section.dataset.markitCode), before = previous.codes?.[index], after = payload.codes[index];
      const old = this.#lines.get(section);
      if (!before || !after || !old || typeof after.source !== 'string') return false;
      if (before.info.language !== after.info.language || before.info.diff !== after.info.diff || before.info.file !== after.info.file) return false;
      const next = codeLines(after.source, after.ranges, after.info.diff);
      if (next.length > 400) return false;
      updates.push([section, old, next, after.source]);
    }
    for (const [section, old, next, source] of updates) {
      const code = section.querySelector('code');
      const keep = firstChangedLine(old, next);
      while (code.children.length > keep) code.lastElementChild.remove();
      if (keep < next.length) code.append(...next.slice(keep, next.length));
      this.#lines.set(section, next); this.#code.set(section, source);
    }
    this.#payloads.set(block, payload);
    return true;
  }
  #prepareCode(section, payload) {
    let rows = payload ? prepareCode(section, payload) : null;
    const code = section.querySelector('code');
    this.#code.set(section, payload?.source ?? code.textContent);
    if (rows && rows.length <= 400) this.#lines.set(section, rows);
    rows ??= [...code.children];
    if (rows.length <= 400) { code.replaceChildren(...rows.slice(0, rows.length)); return; }
    const pre = section.querySelector('pre');
    pre.dataset.markitBackend = 'dom';
    pre.classList.add('mk-virtual-code');
    const lineHeight = 22;
    const top = document.createElement('span'); const bottom = document.createElement('span');
    top.className = bottom.className = 'mk-spacer';
    code.style.minWidth = `${Math.min(4096, (rows.width ?? Math.max(...rows.map(row => row.textContent.length))) + 7)}ch`;
    const update = () => {
      const first = Math.max(0, Math.floor(pre.scrollTop / lineHeight) - 8);
      const last = Math.min(rows.length, first + Math.ceil((pre.clientHeight || 320) / lineHeight) + 16);
      top.style.height = `${first * lineHeight}px`; bottom.style.height = `${(rows.length - last) * lineHeight}px`;
      code.replaceChildren(top, ...rows.slice(first, last), bottom);
    };
    pre.addEventListener('scroll', update, { passive: true }); update();
    // Automatic GPU selection remains disabled until rendering benchmarks show
    // a benefit. Hosts can explicitly request it for large source views.
    if (this.#options.codeRendering === 'webgpu') {
      queueMicrotask(() => {
        if (this.#destroyed || !pre.isConnected) return;
        this.#gpu.attach(pre, Array.isArray(rows) ? rows : rows.slice(0, rows.length), next => { rows = next; update(); }, () => this.#gpuViews.delete(pre)).then(view => {
          if (!view || view.disposed) return;
          if (this.#destroyed || !pre.isConnected) view.destroy();
          else this.#gpuViews.set(pre,view);
        });
      });
    }
  }
  async #resolveImage(image) {
    image.dataset.requested = 'true';
    const controller = new AbortController(); this.#imageJobs.set(image, controller);
    try {
      const value = await this.#options.resolveImage(image.dataset.markitSrc, image.alt, controller.signal);
      if (!value || controller.signal.aborted || !image.isConnected) return;
      const blob = value instanceof Blob ? value : new Blob([value], { type: 'image/png' });
      // The host supplies already-authorized image bytes, never arbitrary HTML/SVG.
      if (!['image/png', 'image/jpeg', 'image/webp', 'image/gif'].includes(blob.type)) throw new Error('Unsupported image type');
      const url = URL.createObjectURL(blob); this.#urls.set(image, url); image.src = url;
    } catch (error) { if (!controller.signal.aborted) this.#options.onError?.(error); }
    finally { this.#imageJobs.delete(image); }
  }
  async #activate(event) {
    const button = event.target.closest('[data-markit-copy]');
    if (button) {
      try {
        await navigator.clipboard.writeText(this.#code.get(button.closest('.mk-codeblock')) ?? '');
        button.textContent = 'Copied'; button.setAttribute('aria-label', 'Copied');
      } catch (error) { this.#options.onError?.(error); }
      return;
    }
    const reference = event.target.closest('[data-markit-reference]');
    if (reference) {
      if (this.#options.onReference) {
        event.preventDefault();
        this.#options.onReference({ kind: reference.dataset.referenceKind, target: reference.dataset.markitReference });
      }
      return;
    }
    const link = event.target.closest('a[href]');
    if (link && this.#options.onLink) { event.preventDefault(); this.#options.onLink(link.getAttribute('href')); }
  }
  destroy() {
    if (this.#destroyed) return;
    this.#destroyed = true; this.#listeners.abort();
    this.#engine.release(this, this.#stream);
    if (!this.#engine.shared) this.#engine.terminate();
    this.#gpu.destroy(); this.#gpuViews.clear();
    this.#queued?.reject(new Error('Renderer disposed')); this.#queued = undefined;
    for (const job of this.#imageJobs.values()) job.abort(); this.#imageJobs.clear();
    for (const url of this.#urls.values()) URL.revokeObjectURL(url); this.#urls.clear();
  }
}
