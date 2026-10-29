export interface CodeToken { text: string; light: string; dark: string }
export interface HighlightedCode { language: string; plain: boolean; tokens: CodeToken[] }
export interface MarkitReference { kind: 'file' | 'thread' | 'pull_request'; target: string }
export interface MarkitOptions {
  workerURL?: URL | string;
  /** Renderers share one worker and Wasm instance per worker URL by default.
   * `dedicated` starts a private worker that `destroy()` terminates. */
  worker?: 'shared' | 'dedicated';
  /** WebGPU draws eligible large ASCII source views. Auto currently uses DOM until benchmarked. */
  codeRendering?: 'auto' | 'dom' | 'webgpu';
  onLink?: (url: string) => void;
  onReference?: (reference: MarkitReference) => void;
  resolveImage?: (url: string, alt: string, signal: AbortSignal) => Blob | Uint8Array | null | Promise<Blob | Uint8Array | null>;
  onError?: (error: Error) => void;
}
export interface RenderPatch { replace_from: number; blocks: string[]; finalized: boolean; source_bytes: number; parsed_bytes: number; superseded: boolean }
export class MarkitRenderer {
  constructor(element: HTMLElement, options?: MarkitOptions);
  render(source: string, options?: { finalized?: boolean }): Promise<RenderPatch | { superseded: true }>;
  /** Colours the whole source, up to 8 MiB of UTF-8; larger sources come back as one plain token
   * with `plain: true`. Unknown non-empty languages use a generic lexer; `''` and `'text'` stay plain.
   * `grammar: true` uses the optional Sublime grammars, which keep their own smaller limits. */
  highlight(source: string, language: string, options?: { grammar?: boolean }): Promise<HighlightedCode>;
  destroy(): void;
}
/** Terminates the shared workers. Live renderers start a new one on their next request. */
export function terminateMarkitWorkers(): void;
/** Starts the shared worker and its Wasm compile ahead of the first render. */
export function preloadMarkit(options?: Pick<MarkitOptions, 'workerURL'>): void;
