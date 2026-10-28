import { createElement, forwardRef, memo, useEffect, useImperativeHandle, useRef } from 'react';
import { MarkitRenderer } from './index.js';

const OPTIONS = ['onLink', 'onReference', 'resolveImage', 'onError'];

/** Owns one renderer for one element. Text updates go straight to the renderer,
 * never through React state, and survive StrictMode's detach/re-attach. */
class MarkitController {
  #latest; #element = null; #renderer = null; #observer = null; #visible = false;
  #source = ''; #finalized = true; #rendered = false; #pending = Promise.resolve();
  constructor(latest) {
    this.#latest = latest;
    this.ref = element => (element ? this.#attach(element) : this.#detach());
  }
  get renderer() { return this.#renderer; }
  /** Resolves when the newest text has been rendered (or superseded). */
  get rendered() { return this.#pending; }
  #attach(element) {
    if (this.#element === element) return;
    this.#detach();
    this.#element = element;
    const { workerURL, worker, codeRendering, lazy } = this.#latest.current;
    const options = { workerURL, worker, codeRendering };
    // Callbacks are read when used, so new closures never recreate the renderer.
    for (const name of OPTIONS) Object.defineProperty(options, name, { enumerable: true, get: () => this.#latest.current[name] });
    this.#renderer = new MarkitRenderer(element, options);
    this.#rendered = false;
    this.#visible = !lazy || typeof IntersectionObserver === 'undefined';
    if (!this.#visible) {
      this.#observer = new IntersectionObserver(entries => {
        if (!entries.some(entry => entry.isIntersecting)) return;
        this.#observer?.disconnect(); this.#observer = null;
        this.#visible = true; this.#flush();
      }, { rootMargin: typeof lazy === 'string' ? lazy : '1200px 0px' });
      this.#observer.observe(element);
    }
    this.#flush();
  }
  #detach() {
    this.#observer?.disconnect(); this.#observer = null;
    this.#renderer?.destroy(); this.#renderer = null;
    this.#element = null;
  }
  #flush() {
    const element = this.#element, renderer = this.#renderer;
    if (!renderer || (!this.#rendered && this.#source === '')) return this.#pending;
    if (!this.#visible) {
      // Offscreen until first seen: plain text keeps height, search and copy.
      element.classList.add('mk-pending');
      element.textContent = this.#source;
      return this.#pending;
    }
    if (!this.#rendered) { element.classList.remove('mk-pending'); element.textContent = ''; this.#rendered = true; }
    const job = renderer.render(this.#source, { finalized: this.#finalized });
    this.#pending = job.then(patch => {
      if (!patch.superseded) this.#latest.current.onRender?.(patch);
      return patch;
    }, error => {
      if (renderer === this.#renderer) this.#latest.current.onError?.(error);
      return { superseded: true };
    });
    return this.#pending;
  }
  /** Replaces the whole text. Unchanged text is ignored. */
  render(source, { finalized = true } = {}) {
    if (typeof source !== 'string') throw new TypeError('Markdown must be a string');
    if (source === this.#source && finalized === this.#finalized) return this.#pending;
    this.#source = source; this.#finalized = finalized;
    return this.#flush();
  }
  /** Appends streamed text to an open message. */
  append(delta) { return this.render(this.#source + delta, { finalized: false }); }
  /** Marks the message complete. */
  finish() { return this.render(this.#source, { finalized: true }); }
}

/**
 * A Markit renderer bound to a DOM element by `ref`. The returned controller is
 * stable for the component's life. Call `render`, or `append` and `finish`
 * from a stream handler, without storing the text in React state.
 */
export function useMarkit(options = {}) {
  const latest = useRef(options);
  latest.current = options;
  const controller = useRef(null);
  controller.current ??= new MarkitController(latest);
  return controller.current;
}

/** Renders `markdown` into a div. Re-rendering with longer text streams it;
 * `finalized={false}` keeps the last block open. */
export const Markit = memo(forwardRef(function Markit(props, ref) {
  const { markdown = '', finalized = true, lazy, workerURL, worker, codeRendering,
    onLink, onReference, resolveImage, onError, onRender, as = 'div', ...rest } = props;
  const markit = useMarkit({ lazy, workerURL, worker, codeRendering, onLink, onReference, resolveImage, onError, onRender });
  useImperativeHandle(ref, () => markit, [markit]);
  useEffect(() => { markit.render(markdown, { finalized }); }, [markit, markdown, finalized]);
  return createElement(as, { ...rest, ref: markit.ref });
}));
