export interface CodeToken { text: string; light: string; dark: string }
export interface HighlightedCode { language: string; plain: boolean; tokens: CodeToken[] }
export interface MarkitReference { kind: 'file' | 'thread' | 'pull_request'; target: string }
/** Code fence chrome. */
export interface MarkitCodeOptions {
  /** Show line numbers once a fence has at least this many lines. Default 1 (always).
   * Diff fences show old/new numbers whenever `@@ -a,b +c,d @@` hunk headers give them. */
  lineNumbersFrom?: number;
  /** Fold fences longer than this many lines (plus two, so a barely longer fence shows whole)
   * behind a "Show all N lines" / "Collapse" toggle. Default: never fold. Fences over
   * 400 lines use a scrolling viewport instead. */
  collapseAfter?: number;
}
export interface MarkitOptions {
  workerURL?: URL | string;
  /** Renderers share one worker and Wasm instance per worker URL by default.
   * `dedicated` starts a private worker that `destroy()` terminates.
   * `inline` runs the same Wasm on the calling thread (asynchronously, in order);
   * it is also used automatically where `Worker` does not exist (Node, jsdom). */
  worker?: 'shared' | 'dedicated' | 'inline';
  /** Inline engine only: the Wasm (`markit_wasm_bg.wasm` bytes or a compiled module).
   * Without it the inline engine fetches the file next to `pkg/markit_wasm.js`.
   * One inline engine serves the whole page or process; the first `wasm` given wins. */
  wasm?: BufferSource | WebAssembly.Module;
  /** WebGPU draws eligible large ASCII source views. Auto currently uses DOM until benchmarked. */
  codeRendering?: 'auto' | 'dom' | 'webgpu';
  code?: MarkitCodeOptions;
  onLink?: (url: string) => void;
  onReference?: (reference: MarkitReference) => void;
  resolveImage?: (url: string, alt: string, signal: AbortSignal) => Blob | Uint8Array | null | Promise<Blob | Uint8Array | null>;
  onError?: (error: Error) => void;
}
export interface RenderPatch { replace_from: number; blocks: string[]; finalized: boolean; source_bytes: number; parsed_bytes: number; superseded: boolean }
export class MarkitRenderer {
  constructor(element: HTMLElement, options?: MarkitOptions);
  /** Resolves once this source is on the page, or with `superseded: true` when newer
   * text replaced it first. Results reach the DOM at most once per animation frame
   * (a macrotask when the page is hidden or has no requestAnimationFrame); the open
   * last block is updated in place and settled blocks are never rebuilt. */
  render(source: string, options?: { finalized?: boolean }): Promise<RenderPatch | { superseded: true }>;
  highlight(source: string, language: string, options?: { grammar?: boolean }): Promise<HighlightedCode>;
  destroy(): void;
}
/** Terminates the shared workers. Live renderers start a new one on their next request. */
export function terminateMarkitWorkers(): void;
/** Starts the shared worker (or, with `worker: 'inline'`, the in-process Wasm) ahead of the first render. */
export function preloadMarkit(options?: Pick<MarkitOptions, 'workerURL' | 'worker' | 'wasm'>): void;
