export interface CodeToken { text: string; light: string; dark: string }
export interface HighlightedCode { language: string; plain: boolean; tokens: CodeToken[] }
/** `mention` (`login` or `org/team`), `issue` (`#123` or `owner/repo#123`) and
 * `commit` (a SHA or `owner/repo@sha`) come from the GitHub flavour. */
export interface MarkitReference { kind: 'file' | 'thread' | 'pull_request' | 'mention' | 'issue' | 'commit'; target: string }
/** A task box the reader ticked. `offset` is the UTF-16 index of the character
 * inside its `[ ]` in the Markdown source (`source[offset - 1] === '['`), so the
 * host rewrites exactly that character; `index` is the box's order. */
export interface MarkitTask { index: number; offset: number; checked: boolean }
/** `default` is agent prose: raw HTML shows as text. `github` is for PR, issue
 * and comment bodies: footnotes, `> [!NOTE]` alerts, an allowlisted HTML subset,
 * task boxes, `@mentions`, `#123`, commit SHAs, emoji and mermaid diagrams. */
export type MarkitFlavor = 'default' | 'github';
export interface MarkitOptions {
  workerURL?: URL | string;
  /** Renderers share one worker and Wasm instance per worker URL by default.
   * `dedicated` starts a private worker that `destroy()` terminates. */
  worker?: 'shared' | 'dedicated';
  /** WebGPU draws eligible large ASCII source views. Auto currently uses DOM until benchmarked. */
  codeRendering?: 'auto' | 'dom' | 'webgpu';
  /** Default `default`. Fixed for the renderer's lifetime. */
  flavor?: MarkitFlavor;
  onLink?: (url: string) => void;
  onReference?: (reference: MarkitReference) => void;
  /** Supplies authorized PNG, JPEG, WebP or GIF bytes for Markdown images and, in
   * the GitHub flavour, HTML `<img>` and `<picture>` sources. Markit never loads them. */
  resolveImage?: (url: string, alt: string, signal: AbortSignal) => Blob | Uint8Array | null | Promise<Blob | Uint8Array | null>;
  /** GitHub flavour: supplies a playable video for an HTML `<video>`: bytes, or a
   * `blob:`/`https:` URL the host minted. Without it videos stay empty. */
  resolveMedia?: (url: string, kind: 'video', signal: AbortSignal) => Blob | Uint8Array | string | null | Promise<Blob | Uint8Array | string | null>;
  /** GitHub flavour: draws a closed ```` ```mermaid ```` fence. An element is inserted
   * as is; a string is SVG markup, shown as an image so it cannot run script. On
   * `null` or an error the code block stays. */
  renderDiagram?: (language: string, source: string, signal: AbortSignal) => Element | string | null | Promise<Element | string | null>;
  /** GitHub flavour: task boxes are enabled only when this is set. */
  onTask?: (task: MarkitTask) => void;
  onError?: (error: Error) => void;
}
export interface RenderPatch { replace_from: number; blocks: string[]; finalized: boolean; source_bytes: number; parsed_bytes: number; superseded: boolean }
export class MarkitRenderer {
  constructor(element: HTMLElement, options?: MarkitOptions);
  render(source: string, options?: { finalized?: boolean }): Promise<RenderPatch | { superseded: true }>;
  highlight(source: string, language: string, options?: { grammar?: boolean }): Promise<HighlightedCode>;
  destroy(): void;
}
/** Terminates the shared workers. Live renderers start a new one on their next request. */
export function terminateMarkitWorkers(): void;
/** Starts the shared worker and its Wasm compile ahead of the first render. */
export function preloadMarkit(options?: Pick<MarkitOptions, 'workerURL'>): void;
