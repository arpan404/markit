/** GitHub flavour behaviour for one renderer: task boxes the host can tick,
 * footnote links that stay inside the rendered element, diagrams the host
 * draws, and picture sources and videos the host resolves. Markit never loads
 * a URL from the text itself. */

const imageTypes = ['image/png', 'image/jpeg', 'image/webp', 'image/gif'];
const videoTypes = ['video/mp4', 'video/webm', 'video/quicktime', 'video/ogg'];

function svgImage(svg) {
  const bytes = new TextEncoder().encode(svg);
  let binary = '';
  for (let at = 0; at < bytes.length; at += 0x8000) binary += String.fromCharCode(...bytes.subarray(at, at + 0x8000));
  const image = document.createElement('img');
  // As an image the SVG cannot run script or reach the page.
  image.src = `data:image/svg+xml;base64,${btoa(binary)}`;
  image.alt = 'Diagram';
  return image;
}

export class GithubFeatures {
  #root; #options; #jobs = new Map(); #urls = new Map();
  constructor(root, options) { this.#root = root; this.#options = options; }

  /** Prepares a newly inserted block. */
  mount(block) {
    if (this.#options.onTask) for (const box of block.querySelectorAll('input[data-markit-task]')) box.disabled = false;
    if (this.#options.renderDiagram) for (const node of block.querySelectorAll('.mk-diagram[data-markit-diagram]')) this.#diagram(node);
    if (this.#options.resolveImage) for (const source of block.querySelectorAll('source[data-markit-srcset]')) this.#source(source);
    if (this.#options.resolveMedia) for (const video of block.querySelectorAll('video[data-markit-src]')) this.#video(video);
  }

  /** Cancels work for a block that is being removed. */
  release(block) {
    for (const [node, job] of this.#jobs) if (block.contains(node)) { job.abort(); this.#jobs.delete(node); }
    for (const [node, url] of this.#urls) if (block.contains(node)) { URL.revokeObjectURL(url); this.#urls.delete(node); }
  }

  /** Footnote and back-reference links scroll within this renderer. True when handled. */
  activate(event) {
    const note = event.target.closest('[data-markit-footnote]');
    const back = note ? null : event.target.closest('[data-markit-footnote-back]');
    if (!note && !back) return false;
    event.preventDefault();
    const selector = note
      ? `[data-markit-footnote-def="${CSS.escape(note.dataset.markitFootnote)}"]`
      : `[data-markit-footnote="${CSS.escape(back.dataset.markitFootnoteBack)}"]`;
    this.#root.querySelector(selector)?.scrollIntoView({ block: 'center', behavior: 'smooth' });
    return true;
  }

  /** A ticked task box reports where its mark sits in the source. */
  change(event) {
    const box = event.target.closest?.('input[data-markit-task]');
    if (!box || !this.#options.onTask) return;
    this.#options.onTask({ index: Number(box.dataset.markitTask), offset: Number(box.dataset.markitTaskOffset), checked: box.checked });
  }

  destroy() {
    for (const job of this.#jobs.values()) job.abort();
    this.#jobs.clear();
    for (const url of this.#urls.values()) URL.revokeObjectURL(url);
    this.#urls.clear();
  }

  async #run(node, work) {
    node.dataset.requested = 'true';
    const controller = new AbortController();
    this.#jobs.set(node, controller);
    try { await work(controller.signal); }
    catch (error) { if (!controller.signal.aborted) this.#options.onError?.(error); }
    finally { if (this.#jobs.get(node) === controller) this.#jobs.delete(node); }
  }

  #diagram(node) {
    return this.#run(node, async signal => {
      const drawn = await this.#options.renderDiagram(node.dataset.markitDiagram, node.dataset.markitDiagramSource ?? '', signal);
      if (!drawn || signal.aborted || !node.isConnected) return;
      const element = typeof drawn === 'string' ? svgImage(drawn) : drawn instanceof Element ? drawn : null;
      if (!element) return;
      element.classList.add('mk-diagram-drawn');
      // The code block stays in the DOM as the fallback and for copying.
      for (const fallback of node.querySelectorAll(':scope > .mk-codeblock')) fallback.hidden = true;
      node.append(element);
      node.dataset.rendered = 'true';
    });
  }

  #objectURL(node, value, types) {
    const blob = value instanceof Blob ? value : new Blob([value], { type: types[0] });
    if (!types.includes(blob.type)) throw new Error('Unsupported media type');
    const url = URL.createObjectURL(blob);
    this.#urls.set(node, url);
    return url;
  }

  #source(source) {
    return this.#run(source, async signal => {
      const value = await this.#options.resolveImage(source.dataset.markitSrcset, '', signal);
      if (!value || signal.aborted || !source.isConnected) return;
      source.srcset = this.#objectURL(source, value, imageTypes);
    });
  }

  #video(video) {
    return this.#run(video, async signal => {
      const value = await this.#options.resolveMedia(video.dataset.markitSrc, 'video', signal);
      if (!value || signal.aborted || !video.isConnected) return;
      if (typeof value === 'string') {
        // Only URLs the host minted for playback; never a scriptable scheme.
        if (!/^(blob:|https?:)/i.test(value)) throw new Error('Unsupported media URL');
        video.src = value;
      } else video.src = this.#objectURL(video, value, videoTypes);
    });
  }
}
