import init, { MarkdownStream, highlightCodePacked, highlightGrammar } from './pkg/markit_wasm.js';

const ready = init();
let stream;
let previous = '';
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

self.onmessage = async ({ data }) => {
  const { id, type } = data;
  try {
    await ready;
    if (type === 'highlight') {
      if (data.grammar) self.postMessage({ id, result: JSON.parse(highlightGrammar(data.source, data.language)) });
      else {
        const ranges = highlightCodePacked(data.source, data.language);
        self.postMessage({ id, result: ranges }, [ranges.buffer]);
      }
      return;
    }
    if (typeof data.source !== 'string' || data.source.length > 8 * 1024 * 1024) throw new Error('Invalid or oversized document');
    if (!stream) stream = MarkdownStream.web();
    if (!data.source.startsWith(previous)) { stream.reset(); previous = ''; }
    const packed = stream.appendPacked(data.source.slice(previous.length), !!data.finalized);
    previous = data.source;
    const patch = decodeWebPatch(packed);
    const transfers = [];
    for (const block of patch.blocks) for (const code of block.codes ?? []) transfers.push(code.ranges.buffer);
    self.postMessage({ id, result: patch }, transfers);
  } catch (error) {
    if (type === 'render') { stream?.free(); stream = undefined; previous = ''; }
    self.postMessage({ id, error: String(error?.message ?? error) });
  }
};
