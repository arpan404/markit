import init, * as wasm from './pkg/markit_wasm.js';
import { createMarkitCore } from './render-core.js';
export { decodeWebPatch } from './render-core.js';

let memory;
const ready = init().then(exports => { memory = exports.memory; });
const core = createMarkitCore(wasm);
/** Diagnostics for tests and benchmarks: live streams and Wasm linear memory. */
self.markitStats = () => ({ streams: core.streams(), wasmMemory: memory?.buffer.byteLength ?? 0 });

self.onmessage = async ({ data }) => {
  const { id } = data;
  try {
    await ready;
    const answer = core.handle(data);
    if (answer) self.postMessage({ id, result: answer.result }, answer.transfers);
  } catch (error) {
    self.postMessage({ id, error: String(error?.message ?? error), code: error?.code });
  }
};
