import { GpuCodeRenderer } from './gpu-code.js';

/** One in-flight render and one queued replacement keep typing ahead of parsing.
 * Parsing and highlighting always execute in the Rust WebAssembly worker. */
export class MarkitRenderer {
  #element; #worker; #pending = new Map(); #id = 0; #revision = 0;
  #failed = false; #running = false; #queued; #destroyed = false; #options; #code = new WeakMap();
  #html = []; #dirty = Infinity;
  #urls = new Map(); #imageJobs = new Map(); #listeners = new AbortController();
  #gpu = new GpuCodeRenderer(); #gpuViews = new Map();
  constructor(element, options = {}) {
    this.#element = element;
    this.#options = options;
    element.classList.add('markit');
    this.#startWorker();
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
  #startWorker() {
    this.#worker?.terminate(); this.#failed = false;
    this.#worker = new Worker(this.#options.workerURL ?? new URL('./worker.js', import.meta.url), { type: 'module' });
    this.#worker.onmessage = ({ data }) => {
      const pending = this.#pending.get(data.id);
      if (!pending) return;
      this.#pending.delete(data.id);
      if (data.error) pending.reject(new Error(data.error)); else pending.resolve(data.result);
    };
    this.#worker.onerror = event => {
      for (const pending of this.#pending.values()) pending.reject(new Error(event.message || 'Markdown worker failed'));
      this.#pending.clear(); this.#failed = true;
    };
  }
  #rpc(type, fields) {
    if (this.#destroyed) return Promise.reject(new Error('Renderer disposed'));
    if (this.#failed) { this.#startWorker(); this.#dirty = 0; }
    if (this.#pending.size >= 8) return Promise.reject(new Error('Too many pending jobs'));
    return new Promise((resolve, reject) => {
      const id = ++this.#id;
      this.#pending.set(id, { resolve, reject });
      this.#worker.postMessage({ id, type, ...fields });
    });
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
      const patch = await this.#rpc('render', { source: job.source, finalized: job.finalized });
      this.#html.splice(patch.replace_from, this.#html.length - patch.replace_from, ...patch.blocks);
      this.#dirty = Math.min(this.#dirty, patch.replace_from);
      if (job.revision === this.#revision) {
        this.#apply({ replace_from: this.#dirty, blocks: this.#html.slice(this.#dirty) });
        this.#dirty = Infinity;
      }
      job.resolve({ ...patch, superseded: job.revision !== this.#revision });
    } catch (error) { job.reject(error); }
    finally { this.#running = false; this.#drain(); }
  }
  highlight(source, language, { grammar = false } = {}) { return this.#rpc('highlight', { source, language, grammar }); }
  #apply(patch) {
    if (this.#destroyed) return;
    while (this.#element.children.length > patch.replace_from) {
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
    for (const html of patch.blocks) {
      const block = document.createElement('div');
      block.className = 'mk-block';
      block.innerHTML = html;
      for (const section of block.querySelectorAll('.mk-codeblock')) this.#prepareCode(section);
      fragment.append(block);
    }
    this.#element.append(fragment);
    if (this.#options.resolveImage) {
      for (const image of this.#element.querySelectorAll('img[data-markit-src]:not([data-requested])')) this.#resolveImage(image);
    }
  }
  #prepareCode(section) {
    const code = section.querySelector('code');
    this.#code.set(section, code.textContent);
    let rows = [...code.children];
    if (rows.length <= 400) return;
    const pre = section.querySelector('pre');
    pre.dataset.markitBackend = 'dom';
    pre.classList.add('mk-virtual-code');
    const lineHeight = 22;
    const top = document.createElement('span'); const bottom = document.createElement('span');
    top.className = bottom.className = 'mk-spacer';
    code.style.minWidth = `${Math.min(4096, Math.max(...rows.map(row => row.textContent.length)) + 7)}ch`;
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
        this.#gpu.attach(pre, rows, next => { rows = next; update(); }, () => this.#gpuViews.delete(pre)).then(view => {
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
    this.#destroyed = true; this.#worker.terminate(); this.#listeners.abort();
    this.#gpu.destroy(); this.#gpuViews.clear();
    for (const pending of this.#pending.values()) pending.reject(new Error('Renderer disposed'));
    this.#pending.clear();
    this.#queued?.reject(new Error('Renderer disposed')); this.#queued = undefined;
    for (const job of this.#imageJobs.values()) job.abort(); this.#imageJobs.clear();
    for (const url of this.#urls.values()) URL.revokeObjectURL(url); this.#urls.clear();
  }
}
