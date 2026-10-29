import type { ComponentPropsWithoutRef, ElementType, ForwardRefExoticComponent, RefAttributes } from 'react';
import type { MarkitOptions, MarkitRenderer, RenderPatch } from './index.js';
export { preloadMarkit } from './index.js';

/** `workerURL`, `worker`, `wasm`, `codeRendering`, `code` and `flavor` are read when the element
 * attaches; changing them later applies to the next attached element. */
export interface MarkitHookOptions extends MarkitOptions {
  /** Defer rendering until the element is near the viewport; plain text shows until then.
   * `true` uses a 1200px vertical margin; a string is an IntersectionObserver rootMargin. */
  lazy?: boolean | string;
  /** Called after each render that was not superseded by newer text. */
  onRender?: (patch: RenderPatch) => void;
}
export interface MarkitController {
  /** Callback ref for the element the renderer draws into. */
  readonly ref: (element: HTMLElement | null) => void;
  readonly renderer: MarkitRenderer | null;
  /** Resolves when the newest text has rendered or been superseded. */
  readonly rendered: Promise<RenderPatch | { superseded: true }>;
  render(source: string, options?: { finalized?: boolean }): Promise<RenderPatch | { superseded: true }>;
  append(delta: string): Promise<RenderPatch | { superseded: true }>;
  finish(): Promise<RenderPatch | { superseded: true }>;
}
export function useMarkit(options?: MarkitHookOptions): MarkitController;
export interface MarkitProps extends MarkitHookOptions, Omit<ComponentPropsWithoutRef<'div'>, 'onError'> {
  markdown?: string;
  /** False while the message is still streaming. Default true. */
  finalized?: boolean;
  as?: ElementType;
}
export const Markit: ForwardRefExoticComponent<MarkitProps & RefAttributes<MarkitController>>;
