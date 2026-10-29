import init, { MarkdownStream, highlightCodePacked, highlightGrammar, mathGlyphsMissing, addMathGlyphs } from './pkg/markit_wasm.js';

let memory;
const ready = init().then(exports => { memory = exports.memory; });
// Messages that arrived during init render first; then, if no formula has
// loaded the math font yet, one throwaway formula parses it and warms the
// math code, so the first formula a reader sees does not pay for that.
ready.then(() => setTimeout(() => {
  if (streams.size) return;
  const warm = MarkdownStream.web();
  warm.appendPacked('$x^2$', true);
  warm.free();
}, 0), () => {});
/** Renderer streams multiplexed over this worker: id -> { stream, previous }. */
const streams = new Map();
/** Diagnostics for tests and benchmarks: live streams and Wasm linear memory. */
self.markitStats = () => ({ streams: streams.size, wasmMemory: memory?.buffer.byteLength ?? 0 });
const decoder = new TextDecoder();

/** MKWP version 1. Packed ranges are copied out so string lengths need not be aligned. */
export function decodeWebPatch(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let cursor = 0;
  const u32 = () => {
    const value = view.getUint32(cursor, true);
    cursor += 4;
    return value;
  };
  const text = () => {
    const len = u32();
    const value = decoder.decode(bytes.subarray(cursor, cursor + len));
    cursor += len;
    return value;
  };
  if (decoder.decode(bytes.subarray(0, 4)) !== 'MKWP') throw new Error('Bad web patch');
  cursor = 4;
  if (u32() !== 1) throw new Error('Unsupported web patch');
  const patch = {
    replace_from: u32(),
    finalized: u32() !== 0,
    source_bytes: u32(),
    parsed_bytes: u32(),
    blocks: [],
  };
  const blocks = u32();
  for (let blockIndex = 0; blockIndex < blocks; blockIndex++) {
    const html = text();
    const codes = [];
    const codeCount = u32();
    for (let codeIndex = 0; codeIndex < codeCount; codeIndex++) {
      const flags = u32();
      const code = { start: u32(), end: u32(), info: { language: text(), diff: (flags & 2) !== 0 } };
      if (flags & 4) code.info.file = text();
      if (flags & 1) code.source = text();
      const words = u32();
      const ranges = new Uint32Array(words);
      for (let word = 0; word < words; word++) ranges[word] = u32();
      code.ranges = ranges;
      codes.push(code);
    }
    patch.blocks.push({ html, codes });
  }
  if (cursor !== bytes.byteLength) throw new Error('Truncated web patch');
  return patch;
}

/** The Wasm module embeds a core cut of STIX Two Math. The other outlines are
 * split by alphabet or symbol family; the first formula that needs a group
 * loads its chunk once. See `web_font` in Rust. */
const mathGlyphs = new Map();
function loadMathGlyphs(missing) {
  const loads = [];
  for (let group = 1; group < 32; group++) {
    if (!(missing & (1 << group))) continue;
    if (!mathGlyphs.has(group)) {
      const load = fetch(new URL(`./pkg/math-glyphs-${group}.bin`, import.meta.url))
        .then(response => { if (!response.ok) throw new Error(`Math glyphs: HTTP ${response.status}`); return response.arrayBuffer(); })
        .then(bytes => { if (!addMathGlyphs(new Uint8Array(bytes))) throw new Error('Invalid math glyphs'); })
        .catch(error => { mathGlyphs.delete(group); throw error; });
      mathGlyphs.set(group, load);
    }
    loads.push(mathGlyphs.get(group));
  }
  return Promise.all(loads);
}

async function render(data) {
  let entry = streams.get(data.stream);
  if (data.append !== undefined) {
    // An append continues the source this stream last rendered.
    if (!entry || typeof data.append !== 'string') throw Object.assign(new Error('Markit stream out of sync'), { code: 'resync' });
  } else if (typeof data.source !== 'string') throw new Error('Invalid or oversized document');
  if (!entry) { entry = { stream: MarkdownStream.web(), previous: '' }; streams.set(data.stream, entry); }
  let delta;
  if (data.append !== undefined) delta = data.append;
  else if (data.source.startsWith(entry.previous)) delta = data.source.slice(entry.previous.length);
  else { entry.stream.reset(); entry.previous = ''; delta = data.source; }
  if (entry.previous.length + delta.length > 8 * 1024 * 1024) throw new Error('Invalid or oversized document');
  let packed = entry.stream.appendPacked(delta, !!data.finalized);
  entry.previous += delta;
  let missing = mathGlyphsMissing();
  while (missing) {
    // A formula was left as source because its glyphs are not loaded yet.
    // Layouts do not depend on the font cut, so render the whole document
    // again with them; the patch replaces from block 0.
    await loadMathGlyphs(missing);
    if (streams.get(data.stream) !== entry) throw Object.assign(new Error('Markit stream released'), { code: 'resync' });
    entry.stream.reset();
    packed = entry.stream.appendPacked(entry.previous, !!data.finalized);
    missing = mathGlyphsMissing();
  }
  return decodeWebPatch(packed);
}

self.onmessage = async ({ data }) => {
  const { id, type } = data;
  try {
    await ready;
    if (type === 'release') {
      streams.get(data.stream)?.stream.free();
      streams.delete(data.stream);
      return;
    }
    if (type === 'highlight') {
      if (data.grammar) self.postMessage({ id, result: JSON.parse(highlightGrammar(data.source, data.language)) });
      else {
        const ranges = highlightCodePacked(data.source, data.language);
        self.postMessage({ id, result: ranges }, [ranges.buffer]);
      }
      return;
    }
    const patch = await render(data);
    const transfers = [];
    for (const block of patch.blocks) for (const code of block.codes ?? []) transfers.push(code.ranges.buffer);
    self.postMessage({ id, result: patch }, transfers);
  } catch (error) {
    if (type === 'render' && error?.code !== 'resync') { streams.get(data.stream)?.stream.free(); streams.delete(data.stream); }
    self.postMessage({ id, error: String(error?.message ?? error), code: error?.code });
  }
};
