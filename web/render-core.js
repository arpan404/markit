// Request handling shared by the Web Worker and the in-process engine. It takes
// the wasm-bindgen module as an argument, so both run exactly the same code.

const decoder = new TextDecoder();
const MAX_SOURCE = 8 * 1024 * 1024;

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
  const words = () => {
    const count = u32();
    const out = new Uint32Array(count);
    for (let word = 0; word < count; word++) out[word] = u32();
    return out;
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
      code.ranges = words();
      // [additions, deletions, then kind, old + 1, new + 1 per line]
      if (flags & 8) code.diff = words();
      codes.push(code);
    }
    patch.blocks.push({ html, codes });
  }
  if (cursor !== bytes.byteLength) throw new Error('Truncated web patch');
  return patch;
}

/** Buffers a patch can hand over instead of copying. */
export function patchTransfers(patch) {
  const transfers = [];
  for (const block of patch.blocks) {
    for (const code of block.codes ?? []) {
      transfers.push(code.ranges.buffer);
      if (code.diff) transfers.push(code.diff.buffer);
    }
  }
  return transfers;
}

const resync = () => Object.assign(new Error('Markit stream out of sync'), { code: 'resync' });

/**
 * Renderer streams multiplexed over one Wasm instance. `wasm` is the initialized
 * wasm-bindgen module (`MarkdownStream`, `highlightCodePacked`, `highlightGrammar`).
 * `handle` answers one request: `{ result, transfers }`, or undefined for a release.
 */
export function createMarkitCore(wasm) {
  /** stream id -> { stream, previous } */
  const streams = new Map();
  const release = id => {
    streams.get(id)?.stream.free();
    streams.delete(id);
  };
  const render = data => {
    let entry = streams.get(data.stream);
    if (data.append !== undefined) {
      // An append continues the source this stream last rendered.
      if (!entry || typeof data.append !== 'string') throw resync();
    } else if (typeof data.source !== 'string') throw new Error('Invalid or oversized document');
    if (!entry) {
      entry = { stream: wasm.MarkdownStream.web(), previous: '' };
      streams.set(data.stream, entry);
    }
    let delta;
    if (data.append !== undefined) delta = data.append;
    else if (data.source.startsWith(entry.previous)) delta = data.source.slice(entry.previous.length);
    else { entry.stream.reset(); entry.previous = ''; delta = data.source; }
    if (entry.previous.length + delta.length > MAX_SOURCE) throw new Error('Invalid or oversized document');
    const packed = entry.stream.appendPacked(delta, !!data.finalized);
    entry.previous += delta;
    return decodeWebPatch(packed);
  };
  const handle = data => {
    if (data.type === 'release') { release(data.stream); return undefined; }
    if (data.type === 'highlight') {
      if (data.grammar) return { result: JSON.parse(wasm.highlightGrammar(data.source, data.language)), transfers: [] };
      const ranges = wasm.highlightCodePacked(data.source, data.language);
      return { result: ranges, transfers: [ranges.buffer] };
    }
    try {
      const patch = render(data);
      return { result: patch, transfers: patchTransfers(patch) };
    } catch (error) {
      // A failed stream starts over; a resync keeps it for the full resend.
      if (error?.code !== 'resync') release(data.stream);
      throw error;
    }
  };
  return { handle, streams: () => streams.size };
}
