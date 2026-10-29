// Brings a live subtree in line with freshly parsed markup, keeping every node
// that still fits. Streaming text then changes a text node's data instead of
// replacing the block, so selection, focus, scroll positions and element
// identity survive. Work is proportional to the subtree, which is only ever
// the open block or a block that changed.

const HTML = 'http://www.w3.org/1999/xhtml';

/**
 * `hooks.code(live, next)` keeps a code section in place (true) or lets it be
 * replaced. `hooks.added(node)` hydrates inserted nodes; `hooks.removed(node)`
 * releases what removed nodes own.
 */
export function morphChildren(parent, next, hooks) {
  let live = parent.firstChild, wanted = next.firstChild;
  while (wanted) {
    const following = wanted.nextSibling;
    if (live && morphNode(live, wanted, hooks)) live = live.nextSibling;
    else {
      parent.insertBefore(wanted, live);
      if (live) { const stale = live; live = live.nextSibling; stale.remove(); hooks.removed(stale); }
      hooks.added(wanted);
    }
    wanted = following;
  }
  while (live) { const stale = live; live = live.nextSibling; stale.remove(); hooks.removed(stale); }
}

/** Updates `live` to match `wanted` in place; false when it must be replaced. */
function morphNode(live, wanted, hooks) {
  if (live.nodeType !== wanted.nodeType) return false;
  if (live.nodeType !== 1) {
    if (live.nodeValue !== wanted.nodeValue) live.nodeValue = wanted.nodeValue;
    return true;
  }
  if (live.localName !== wanted.localName || live.namespaceURI !== wanted.namespaceURI) return false;
  // Math and other foreign markup is replaced whole; it changes as a unit.
  if (live.namespaceURI !== HTML) return live.isEqualNode(wanted);
  if (wanted.localName === 'section' && wanted.hasAttribute('data-markit-code')) {
    return live.hasAttribute('data-markit-code') && hooks.code(live, wanted);
  }
  if (wanted.localName === 'img') {
    // The host resolves images into `src`; a different image starts again.
    return live.getAttribute('data-markit-src') === wanted.getAttribute('data-markit-src')
      && live.getAttribute('alt') === wanted.getAttribute('alt');
  }
  syncAttributes(live, wanted);
  morphChildren(live, wanted, hooks);
  return true;
}

function syncAttributes(live, wanted) {
  for (const { name, value } of wanted.attributes) if (live.getAttribute(name) !== value) live.setAttribute(name, value);
  if (live.attributes.length === wanted.attributes.length) return;
  for (const name of live.getAttributeNames()) if (!wanted.hasAttribute(name)) live.removeAttribute(name);
}
