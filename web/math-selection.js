// Selecting across math. Formulas are SVG, which has no text for the browser
// to select or paint, and a tall formula makes Chrome paint the selection on the
// words beside it as tall as the whole line. So:
// - a formula the selection touches gets `data-selected` and is painted in the
//   selection colour (style.css);
// - the selection is mirrored into the `markit-selection` custom highlight,
//   which paints at the height of the text; style.css shows it instead of the
//   native highlight in lines that hold inline math;
// - double-clicking a formula selects all of it.

const HIGHLIGHT = 'markit-selection';
const MATH = '.mk-math';
/** One selectionchange listener per document, shared by its renderers. */
const documents = new WeakMap();

export class MathSelection {
  #root; #marked = new Set();
  constructor(root, signal) {
    this.#root = root;
    const doc = root.ownerDocument, view = doc.defaultView;
    if (view?.CSS?.highlights && view.Highlight) root.classList.add('mk-highlights');
    root.addEventListener('dblclick', event => {
      const formula = event.target.closest?.(MATH);
      if (!formula || !root.contains(formula)) return;
      const range = doc.createRange();
      range.selectNode(formula);
      const selection = doc.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
    }, { signal });
    let shared = documents.get(doc);
    if (!shared) {
      const members = new Set();
      const changed = () => { mirror(doc); for (const member of members) member.refresh(); };
      shared = { members, changed };
      doc.addEventListener('selectionchange', changed);
      documents.set(doc, shared);
    }
    shared.members.add(this);
    signal.addEventListener('abort', () => {
      shared.members.delete(this);
      if (shared.members.size) return;
      doc.removeEventListener('selectionchange', shared.changed);
      documents.delete(doc);
      view?.CSS?.highlights?.delete(HIGHLIGHT);
    }, { once: true });
  }
  /** Marks the formulas the selection touches. Rendering replaces marked nodes, so it calls this again. */
  refresh() {
    const ranges = selectedRanges(this.#root.ownerDocument).filter(range => range.intersectsNode(this.#root));
    if (!ranges.length && !this.#marked.size) return;
    const next = new Set();
    if (ranges.length) {
      for (const formula of this.#root.querySelectorAll(MATH)) {
        if (ranges.some(range => range.intersectsNode(formula))) next.add(formula);
      }
    }
    for (const formula of this.#marked) if (!next.has(formula)) formula.removeAttribute('data-selected');
    for (const formula of next) if (!formula.hasAttribute('data-selected')) formula.setAttribute('data-selected', '');
    this.#marked = next;
  }
}

function selectedRanges(doc) {
  const selection = doc.getSelection();
  if (!selection || selection.isCollapsed) return [];
  return Array.from({ length: selection.rangeCount }, (_, index) => selection.getRangeAt(index));
}

/** Copies the selection into the custom highlight; ranges are copies so the highlight never aliases the selection. */
function mirror(doc) {
  const view = doc.defaultView, registry = view?.CSS?.highlights;
  if (!registry || !view.Highlight) return;
  const ranges = selectedRanges(doc);
  if (ranges.length) registry.set(HIGHLIGHT, new view.Highlight(...ranges.map(range => range.cloneRange())));
  else registry.delete(HIGHLIGHT);
}
