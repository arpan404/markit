import { StrictMode, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { flushSync } from 'react-dom';
import { Markit, useMarkit } from '../../react.js';

let root;
let setMessages = () => {};
const renders = new Map();

function Transcript({ initial, lazy, code }) {
  const [messages, set] = useState(initial);
  setMessages = set;
  return messages.map(message => (
    <Markit key={message.id} data-id={message.id} markdown={message.text} finalized={message.done !== false} lazy={lazy} code={code}
      onRender={() => renders.set(message.id, (renders.get(message.id) ?? 0) + 1)} />
  ));
}

/** Streams without React state: text goes to the controller directly. */
function Imperative() {
  const markit = useMarkit();
  window.stream = markit;
  return <div data-id="imperative" ref={markit.ref} />;
}

function mount(element, strict) {
  root?.unmount();
  root = createRoot(document.getElementById('root'));
  flushSync(() => root.render(strict ? <StrictMode>{element}</StrictMode> : element));
}

window.app = {
  mountTranscript(messages, { strict = false, lazy = false, code } = {}) { renders.clear(); mount(<Transcript initial={messages} lazy={lazy} code={code} />, strict); },
  setMessages(messages) { flushSync(() => setMessages(messages)); },
  mountImperative(strict = false) { mount(<Imperative />, strict); },
  unmount() { root?.unmount(); root = undefined; },
  renders: id => renders.get(id) ?? 0,
};
window.ready = true;
