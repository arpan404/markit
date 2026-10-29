import { createMarkitCore } from './render-core.js';

/** Requests per owner, so one renderer's work can be cancelled alone. */
class Jobs {
  #pending = new Map(); #id = 0;
  add(owner, resolve, reject) { const id = ++this.#id; this.#pending.set(id, { owner, resolve, reject }); return id; }
  take(id) { const job = this.#pending.get(id); this.#pending.delete(id); return job; }
  count(owner) { let count = 0; for (const job of this.#pending.values()) if (job.owner === owner) count++; return count; }
  rejectOwner(owner, error) {
    for (const [id, job] of this.#pending) if (job.owner === owner) { this.#pending.delete(id); job.reject(error); }
  }
  rejectAll(error) { for (const job of this.#pending.values()) job.reject(error); this.#pending.clear(); }
}

/** One Web Worker and Wasm instance serves every renderer created with the
 * same worker URL. Each renderer owns a stream inside it; requests are
 * multiplexed by id and answered in order. */
class WorkerEngine {
  #url; #worker; #jobs = new Jobs(); #streams = 0; #shared;
  generation = 0;
  constructor(url, shared) { this.#url = url; this.#shared = shared; }
  get shared() { return this.#shared; }
  #start() {
    this.#worker?.terminate();
    this.generation++;
    const worker = new Worker(this.#url, { type: 'module', name: 'markit' });
    worker.onmessage = ({ data }) => {
      const job = this.#jobs.take(data.id);
      if (!job) return;
      if (data.error) job.reject(Object.assign(new Error(data.error), { code: data.code })); else job.resolve(data.result);
    };
    worker.onerror = event => {
      event.preventDefault?.();
      this.#jobs.rejectAll(new Error(event.message || 'Markdown worker failed'));
      if (this.#worker === worker) { worker.terminate(); this.#worker = undefined; }
    };
    this.#worker = worker;
  }
  /** Starts the worker on first use, or again after it failed. */
  ensure() { if (!this.#worker) this.#start(); return this.generation; }
  stream() { return ++this.#streams; }
  pending(owner) { return this.#jobs.count(owner); }
  call(owner, type, fields) {
    this.ensure();
    return new Promise((resolve, reject) => {
      const id = this.#jobs.add(owner, resolve, reject);
      this.#worker.postMessage({ id, type, ...fields });
    });
  }
  /** Rejects the owner's requests and frees its stream in the worker. */
  release(owner, stream) {
    this.#jobs.rejectOwner(owner, new Error('Renderer disposed'));
    if (this.#worker && stream !== undefined) this.#worker.postMessage({ type: 'release', stream });
  }
  terminate() {
    this.#worker?.terminate(); this.#worker = undefined;
    this.#jobs.rejectAll(new Error('Markdown worker terminated'));
  }
}

/** The same Wasm and request handling on the calling thread, for hosts with no
 * `Worker` (Node and jsdom tests, old engines) or that ask for it. Requests stay
 * asynchronous and run in order. wasm-bindgen keeps one instance per module, so
 * there is one inline engine per page or process. */
class InlineEngine {
  #core; #jobs = new Jobs(); #streams = 0; #queue = Promise.resolve(); #wasm;
  generation = 1;
  get shared() { return true; }
  /** `wasm`: module bytes or a compiled module; otherwise the glue fetches its default URL. */
  constructor(wasm) { this.#wasm = wasm; }
  #load() {
    this.#core ??= import('./pkg/markit_wasm.js').then(async module => {
      if (this.#wasm !== undefined) module.initSync({ module: this.#wasm });
      else await module.default();
      return createMarkitCore(module);
    });
    // A failed load is retried on the next request.
    this.#core.catch(() => { this.#core = undefined; });
    return this.#core;
  }
  ensure() { this.#load(); return this.generation; }
  stream() { return ++this.#streams; }
  pending(owner) { return this.#jobs.count(owner); }
  call(owner, type, fields) {
    return new Promise((resolve, reject) => {
      const id = this.#jobs.add(owner, resolve, reject);
      const core = this.#load();
      this.#queue = this.#queue.then(() => core).then(engine => {
        const job = this.#jobs.take(id);
        if (!job) return; // released while queued
        try { job.resolve(engine.handle({ type, ...fields }).result); } catch (error) { job.reject(error); }
      }, error => this.#jobs.take(id)?.reject(error));
    });
  }
  release(owner, stream) {
    this.#jobs.rejectOwner(owner, new Error('Renderer disposed'));
    if (stream !== undefined) this.#queue = this.#queue.then(() => this.#core).then(core => core?.handle({ type: 'release', stream }), () => {});
  }
  terminate() { this.#jobs.rejectAll(new Error('Markdown engine terminated')); }
}

const engines = new Map();
let inline;
function wantsInline(options) {
  return options.worker === 'inline' || typeof Worker === 'undefined';
}
export function engineFor(options) {
  if (wantsInline(options)) return (inline ??= new InlineEngine(options.wasm));
  const url = String(options.workerURL ?? new URL('./worker.js', import.meta.url));
  if (options.worker === 'dedicated') return new WorkerEngine(url, false);
  let engine = engines.get(url);
  if (!engine) { engine = new WorkerEngine(url, true); engines.set(url, engine); }
  return engine;
}
/** Starts the shared worker (or the inline engine) and its Wasm compile before the first message mounts. */
export function preloadMarkit(options = {}) {
  if (options.worker !== 'dedicated') engineFor(options).ensure();
}
/** Terminates shared workers. Live renderers start a new one on their next request. */
export function terminateMarkitWorkers() {
  for (const engine of engines.values()) engine.terminate();
  engines.clear();
}
