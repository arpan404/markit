import { unpackHighlight } from './highlight.js';
import { GpuCodeRenderer } from './gpu-code.js';
import { CodeView } from './code-view.js';
import { morphChildren } from './dom-morph.js';
import { engineFor } from './engine.js';
import { scheduleFrame } from './frame.js';
export { preloadMarkit, terminateMarkitWorkers } from './engine.js';

const SECTIONS = 'section[data-markit-code]';
const within = (node, selector) => (node.matches(selector) ? [node] : node.querySelectorAll(selector));

/** One in-flight render and one queued replacement keep typing ahead of parsing.
 * Parsing and highlighting run in Rust WebAssembly, in a worker or in-process.
 * Finished patches are folded together and reach the DOM at most once per frame. */
export class MarkitRenderer {
  #element; #engine; #stream; #generation = 0; #sent; #revision = 0;
  #running = false; #queued; #destroyed = false; #options; #codeOptions;
  /** Block payloads as the engine last described them, and the first index the DOM lacks. */
  #html = []; #dirty = Infinity;
  /** Finished jobs whose results wait for the next frame. */
  #ready = []; #cancelFrame;
  #payloads = new WeakMap(); #views = new WeakMap();
  #urls = new Map(); #imageJobs = new Map(); #listeners;
  #gpu = new GpuCodeRenderer();
  constructor(element, options = {}) {
    this.#element = element;
    this.#options = options;
    const code = options.code ?? {};
    this.#codeOptions = { lineNumbersFrom: code.lineNumbersFrom ?? 1, collapseAfter: code.collapseAfter, codeRendering: options.codeRendering, gpu: this.#gpu };
    element.classList.add('markit');
    // The element's own realm: jsdom rejects another realm's AbortSignal.
    this.#listeners = new (element.ownerDocument.defaultView?.AbortController ?? AbortController)();
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
      if (this.#destroyed) throw new Error('Renderer disposed');
      for (const block of patch.blocks) {
        for (const code of block.codes ?? []) {
          if (typeof code.source !== 'string') code.source = job.source.slice(code.start, code.end);
        }
      }
      this.#html.splice(patch.replace_from, this.#html.length - patch.replace_from, ...patch.blocks);
      this.#dirty = Math.min(this.#dirty, patch.replace_from);
      this.#ready.push({ job, patch });
      this.#cancelFrame ??= scheduleFrame(this.#element.ownerDocument, () => this.#flush());
    } catch (error) { job.reject(error); }
    finally { this.#running = false; this.#drain(); }
  }
  /** Applies every patch finished since the last frame, then settles their promises.
   * The newest result is what the DOM shows; earlier ones in the frame were superseded. */
  #flush() {
    this.#cancelFrame = undefined;
    if (this.#destroyed) return;
    const ready = this.#ready;
    this.#ready = [];
    if (this.#dirty !== Infinity) { const from = this.#dirty; this.#dirty = Infinity; this.#apply(from); }
    const last = ready.at(-1);
    for (const entry of ready) {
      const { job, patch } = entry;
      job.resolve({ ...patch, blocks: patch.blocks.map(block => block.html), superseded: entry !== last });
    }
  }
  async highlight(source, language, { grammar = false } = {}) {
    const result = await this.#rpc('highlight', { source, language, grammar });
    return grammar ? result : unpackHighlight(source, result);
  }
  /** Brings blocks from `from` in line with `#html`. Existing blocks are updated in
   * place (the open block keeps its element); only missing ones are created. */
  #apply(from) {
    const root = this.#element, doc = root.ownerDocument, html = this.#html, added = [];
    let block = root.children[from], index = from;
    for (; index < html.length && block; index++) {
      const next = block.nextElementSibling;
      this.#update(block, html[index], added);
      block = next;
    }
    while (root.children.length > html.length) { const stale = root.lastElementChild; stale.remove(); this.#release(stale); }
    if (index < html.length) {
      const fragment = doc.createDocumentFragment();
      for (; index < html.length; index++) {
        const node = doc.createElement('div');
        node.className = 'mk-block';
        node.innerHTML = html[index].html;
        this.#payloads.set(node, html[index]);
        fragment.append(node);
        added.push(node);
      }
      root.append(fragment);
    }
    for (const node of added) this.#hydrate(node);
  }
  /** One block: unchanged markup only grows its code; changed markup is morphed. */
  #update(block, payload, added) {
    const previous = this.#payloads.get(block);
    if (previous === payload) return;
    this.#payloads.set(block, payload);
    if (previous?.html === payload.html) {
      for (const section of block.querySelectorAll(SECTIONS)) {
        if (this.#keepCode(section, section, payload)) continue;
        const fresh = section.cloneNode(false);
        section.replaceWith(fresh); this.#release(section); added.push(fresh);
      }
      return;
    }
    const template = block.ownerDocument.createElement('template');
    template.innerHTML = payload.html;
    morphChildren(block, template.content, {
      code: (live, wanted) => this.#keepCode(live, wanted, payload),
      added: node => added.push(node),
      removed: node => this.#release(node),
    });
  }
  /** Keeps a live code section when its fence only grew. */
  #keepCode(live, wanted, payload) {
    const index = wanted.dataset.markitCode;
    if (!this.#views.get(live)?.grow(payload.codes?.[Number(index)])) return false;
    if (live.dataset.markitCode !== index) live.dataset.markitCode = index;
    return true;
  }
  /** Builds code views and requests images inside newly inserted nodes. */
  #hydrate(node) {
    if (node.nodeType !== 1) return;
    const payload = this.#payloads.get(node.closest('.mk-block'));
    for (const section of within(node, SECTIONS)) {
      this.#views.set(section, new CodeView(section, payload?.codes?.[Number(section.dataset.markitCode)], this.#codeOptions));
    }
    if (!this.#options.resolveImage) return;
    for (const image of within(node, 'img[data-markit-src]')) if (!image.dataset.requested) this.#resolveImage(image);
  }
  /** Frees what a removed node owns: code views (and their GPU views), image jobs and object URLs. */
  #release(node) {
    if (node.nodeType !== 1) return;
    for (const section of within(node, SECTIONS)) { this.#views.get(section)?.destroy(); this.#views.delete(section); }
    for (const image of within(node, 'img')) {
      this.#imageJobs.get(image)?.abort(); this.#imageJobs.delete(image);
      const url = this.#urls.get(image);
      if (url) { URL.revokeObjectURL(url); this.#urls.delete(image); }
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
    const toggle = event.target.closest('[data-markit-toggle]');
    if (toggle) { this.#views.get(toggle.closest(SECTIONS))?.toggle(); return; }
    const button = event.target.closest('[data-markit-copy]');
    if (button) {
      const view = this.#views.get(button.closest(SECTIONS));
      try {
        await navigator.clipboard.writeText(view?.source ?? '');
        view?.copied();
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
    this.#cancelFrame?.(); this.#cancelFrame = undefined;
    for (const { job } of this.#ready) job.reject(new Error('Renderer disposed'));
    this.#ready = [];
    this.#release(this.#element);
    this.#gpu.destroy();
    this.#queued?.reject(new Error('Renderer disposed')); this.#queued = undefined;
    for (const job of this.#imageJobs.values()) job.abort(); this.#imageJobs.clear();
    for (const url of this.#urls.values()) URL.revokeObjectURL(url); this.#urls.clear();
  }
}
