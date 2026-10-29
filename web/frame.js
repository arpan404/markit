/** Longest wait for a frame before applying anyway: rAF stops in a page that hides. */
const FRAME_TIMEOUT_MS = 100;

/**
 * Runs `callback` once, before the next paint of `doc`'s window. A hidden page,
 * or one with no requestAnimationFrame (jsdom, Node), uses a macrotask instead.
 * Returns a function that cancels the call.
 */
export function scheduleFrame(doc, callback) {
  const view = doc?.defaultView;
  let done = false;
  const run = () => { if (done) return; done = true; cancel(); callback(); };
  let cancel;
  if (view?.requestAnimationFrame && !doc.hidden) {
    const frame = view.requestAnimationFrame(run);
    const timer = setTimeout(run, FRAME_TIMEOUT_MS);
    cancel = () => { done = true; view.cancelAnimationFrame(frame); clearTimeout(timer); };
  } else {
    const timer = setTimeout(run, 0);
    cancel = () => { done = true; clearTimeout(timer); };
  }
  return cancel;
}
