import init, { MarkdownStream, highlightCodePacked, highlightGrammar } from './pkg/markit_wasm.js';

const ready = init();
/** Renderer streams multiplexed over this worker: id -> { stream, previous }. */
const streams = new Map();
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

function render(data) {
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
  const packed = entry.stream.appendPacked(delta, !!data.finalized);
  entry.previous += delta;
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
    const patch = render(data);
    const transfers = [];
    for (const block of patch.blocks) for (const code of block.codes ?? []) transfers.push(code.ranges.buffer);
    self.postMessage({ id, result: patch }, transfers);
  } catch (error) {
    if (type === 'render' && error?.code !== 'resync') { streams.get(data.stream)?.stream.free(); streams.delete(data.stream); }
    self.postMessage({ id, error: String(error?.message ?? error), code: error?.code });
  }
};
