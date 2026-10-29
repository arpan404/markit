export interface CodeToken { text: string; light: string; dark: string }
export interface HighlightedCode { language: string; plain: boolean; tokens: CodeToken[] }
/** `mention` (`login` or `org/team`), `issue` (`#123` or `owner/repo#123`) and
 * `commit` (a SHA or `owner/repo@sha`) come from the GitHub flavour. */
export interface MarkitReference { kind: 'file' | 'thread' | 'pull_request' | 'mention' | 'issue' | 'commit'; target: string }
/** A task box the reader ticked. `offset` is the UTF-16 index of the character
 * inside its `[ ]` in the Markdown source (`source[offset - 1] === '['`), so the
 * host rewrites exactly that character; `index` is the box's order. */
export interface MarkitTask { index: number; offset: number; checked: boolean }
/** `default` is the plainest: raw HTML shows as text. `github` is for PR, issue
 * and comment bodies: footnotes, `> [!NOTE]` alerts, an allowlisted HTML subset,
 * task boxes, `@mentions`, `#123`, commit SHAs, emoji and mermaid diagrams.
 * `agent` is for coding agents' answers: everything `github` draws except the
 * forge references (`@mentions`, `#123`, commit SHAs), which stay text. */
export type MarkitFlavor = 'default' | 'github' | 'agent';
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
  /** Default `default`. Fixed for the renderer's lifetime. */
  flavor?: MarkitFlavor;
  onLink?: (url: string) => void;
  onReference?: (reference: MarkitReference) => void;
  /** Supplies authorized PNG, JPEG, WebP or GIF bytes for Markdown images and, in
   * the GitHub and agent flavours, HTML `<img>` and `<picture>` sources. Markit never loads them. */
  resolveImage?: (url: string, alt: string, signal: AbortSignal) => Blob | Uint8Array | null | Promise<Blob | Uint8Array | null>;
  /** GitHub and agent flavours: supplies a playable video for an HTML `<video>`: bytes, or a
   * `blob:`/`https:` URL the host minted. Without it videos stay empty. */
  resolveMedia?: (url: string, kind: 'video', signal: AbortSignal) => Blob | Uint8Array | string | null | Promise<Blob | Uint8Array | string | null>;
  /** GitHub and agent flavours: draws a closed ```` ```mermaid ```` fence. An element is inserted
   * as is; a string is SVG markup, shown as an image so it cannot run script. On
   * `null` or an error the code block stays. */
  renderDiagram?: (language: string, source: string, signal: AbortSignal) => Element | string | null | Promise<Element | string | null>;
  /** GitHub and agent flavours: task boxes are enabled only when this is set. */
  onTask?: (task: MarkitTask) => void;
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
  /** Colours the whole source, up to 8 MiB of UTF-8; larger sources come back as one plain token
   * with `plain: true`. Unknown non-empty languages use a generic lexer; `''` and `'text'` stay plain.
   * `grammar: true` uses the optional Sublime grammars, which keep their own smaller limits. */
  highlight(source: string, language: string, options?: { grammar?: boolean }): Promise<HighlightedCode>;
  destroy(): void;
}
/** Terminates the shared workers. Live renderers start a new one on their next request. */
export function terminateMarkitWorkers(): void;
/** Starts the shared worker (or, with `worker: 'inline'`, the in-process Wasm) ahead of the first render. */
export function preloadMarkit(options?: Pick<MarkitOptions, 'workerURL' | 'worker' | 'wasm'>): void;
