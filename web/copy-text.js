// Copying a selection that holds math. Formulas are SVG with no text, so the
// browser's own copy drops them. Here each formula becomes its TeX source in
// text/plain, and text/html keeps the drawn formula.

const MATH = '[data-tex]';
const PARAGRAPH = new Set(['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'blockquote', 'table', 'ul', 'ol', 'dl', 'hr', 'section', 'figure', 'details']);
const LINE = new Set(['li', 'tr', 'dt', 'dd', 'div', 'summary']);
const SKIP = new Set(['button', 'script', 'style', 'template', 'svg']);

/** Fills the clipboard for a copy inside `root` when the selection holds math; otherwise leaves the browser's copy alone. */
export function copyMath(event, root) {
  const selection = root.ownerDocument.getSelection();
  if (!event.clipboardData || !selection?.rangeCount || selection.isCollapsed) return;
  const texts = [], htmls = [];
  let math = false;
  for (let index = 0; index < selection.rangeCount; index++) {
    const range = selection.getRangeAt(index);
    const fragment = range.cloneContents();
    if (fragment.querySelector(MATH)) math = true;
    const originals = selector => [...root.querySelectorAll(selector)].filter(node => range.intersectsNode(node));
    texts.push(selectionText(fragment, numbersFor(fragment, originals('li'))));
    htmls.push(selectionHtml(fragment, originals(MATH)));
  }
  if (!math) return;
  event.clipboardData.setData('text/plain', texts.join('\n'));
  event.clipboardData.setData('text/html', htmls.join(''));
  event.preventDefault();
}

/** The list number each cloned item had on the page; a clone loses the items before it. */
function numbersFor(fragment, items) {
  const numbers = new Map(), clones = fragment.querySelectorAll('li');
  if (clones.length !== items.length) return numbers;
  clones.forEach((clone, index) => {
    const item = items[index], list = item.parentElement;
    if (list?.localName !== 'ol') return;
    const position = [...list.children].filter(child => child.localName === 'li').indexOf(item);
    numbers.set(clone, (list.start || 1) + position);
  });
  return numbers;
}

/** Rendered markup, with every formula whole even when the selection cut through it. */
function selectionHtml(fragment, formulas) {
  const copy = fragment.cloneNode(true), clones = copy.querySelectorAll(MATH);
  if (clones.length === formulas.length) clones.forEach((clone, index) => clone.replaceWith(formulas[index].cloneNode(true)));
  for (const button of copy.querySelectorAll('button')) button.remove();
  const holder = fragment.ownerDocument.createElement('div');
  holder.append(copy);
  return holder.innerHTML;
}

/**
 * Plain text for a cloned selection: formulas as their TeX source, one line per
 * block with a blank line between paragraphs, list items with their markers,
 * table cells split by tabs, and code exactly as written.
 */
export function selectionText(fragment, numbers = new Map()) {
  const out = [];
  let pending = 0;
  const last = () => out.at(-1) ?? '';
  const block = lines => {
    if (!lines || !out.length) return;
    out[out.length - 1] = last().replace(/\s+$/, '');
    pending = Math.max(pending, lines);
  };
  const emit = text => {
    if (!text) return;
    if (pending) { out.push('\n'.repeat(pending)); pending = 0; }
    out.push(text);
  };
  const walk = (node, pre, depth) => {
    if (node.nodeType === 3) {
      if (pre) { emit(node.data); return; }
      let text = node.data.replace(/\s+/g, ' ');
      if (pending || !out.length || /\s$/.test(last())) text = text.replace(/^ /, '');
      emit(text);
      return;
    }
    if (node.nodeType !== 1 && node.nodeType !== 11) return;
    const name = node.localName;
    if (name) {
      if (node.matches(MATH)) {
        const display = node.classList.contains('mk-display');
        if (display) block(2);
        emit(node.dataset.tex);
        if (display) block(2);
        return;
      }
      if (SKIP.has(name) || node.matches('.mk-codeblock > header')) return;
      if (name === 'br') { emit('\n'); return; }
      if (name === 'img') { emit(node.getAttribute('alt') ?? ''); return; }
      if (name === 'input' && node.type === 'checkbox') { emit(node.checked ? '[x] ' : '[ ] '); return; }
    }
    const list = name === 'ul' || name === 'ol';
    // A nested list sits on the line under its item, not a paragraph away.
    const gap = PARAGRAPH.has(name) ? (list && depth ? 1 : 2) : LINE.has(name) ? 1 : 0;
    block(gap);
    if (name === 'li') {
      const parent = node.parentElement?.localName;
      const marker = parent === 'ol' ? `${numbers.get(node) ?? 1}. ` : parent === 'ul' ? '- ' : '';
      emit('   '.repeat(Math.max(0, depth - 1)) + marker);
    }
    if ((name === 'td' || name === 'th') && node.previousElementSibling) emit('\t');
    for (const child of node.childNodes) walk(child, pre || name === 'pre', depth + (list ? 1 : 0));
    block(gap);
  };
  walk(fragment, false, 0);
  return out.join('').replace(/^\s+|\s+$/g, '');
}
