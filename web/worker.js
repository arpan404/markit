import init, { MarkdownStream, highlightCodePacked, highlightGrammar } from './pkg/markit_wasm.js';
const ready = init();
let stream;
let previous = '';
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
    const patch = JSON.parse(stream.append(data.source.slice(previous.length), !!data.finalized));
    previous = data.source;
    const transfers = [];
    for (const block of patch.blocks) for (const code of block.codes ?? []) {
      code.ranges = highlightCodePacked(code.source, code.info.language);
      transfers.push(code.ranges.buffer);
    }
    self.postMessage({ id, result: patch }, transfers);
  } catch (error) { if (type === 'render') { stream?.free(); stream = undefined; previous = ''; } self.postMessage({ id, error: String(error?.message ?? error) }); }
};
