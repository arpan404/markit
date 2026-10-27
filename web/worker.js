import { unpackHighlight } from './highlight.js';
import init, { MarkdownStream, highlightCodePacked, highlightGrammar } from './pkg/markit_wasm.js';
const ready = init();
let stream;
let previous = '';
self.onmessage = async ({ data }) => {
  const { id, type } = data;
  try {
    await ready;
    if (type === 'highlight') {
      self.postMessage({ id, result: data.grammar ? JSON.parse(highlightGrammar(data.source, data.language)) : unpackHighlight(data.source, highlightCodePacked(data.source, data.language)) });
      return;
    }
    if (typeof data.source !== 'string' || data.source.length > 8 * 1024 * 1024) throw new Error('Invalid or oversized document');
    if (!stream) stream = new MarkdownStream();
    if (!data.source.startsWith(previous)) { stream.reset(); previous = ''; }
    const patch = JSON.parse(stream.append(data.source.slice(previous.length), !!data.finalized));
    previous = data.source;
    self.postMessage({ id, result: { ...patch, blocks: patch.blocks.map(block => block.html) } });
  } catch (error) { if (type === 'render') { stream?.free(); stream = undefined; previous = ''; } self.postMessage({ id, error: String(error?.message ?? error) }); }
};
