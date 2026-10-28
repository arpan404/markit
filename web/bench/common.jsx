// Shared React benchmark shell. Each harness supplies a Message component.
import { useState } from 'react';
import { createRoot } from 'react-dom/client';
import { flushSync } from 'react-dom';

const longTasks = [];
try { new PerformanceObserver(list => longTasks.push(...list.getEntries())).observe({ type: 'longtask', buffered: true }); } catch {}

export function installBench({ name, Message, ready }) {
  let root, set = () => {};
  function Transcript({ initial }) {
    const [messages, setMessages] = useState(initial);
    set = setMessages;
    return messages.map(message => <div key={message.id} className="bench-message" data-id={message.id} data-role={message.role}><Message message={message} /></div>);
  }
  const host = document.getElementById('root');
  /** Resolves once the transcript is ready and the DOM has been quiet for `quiet` ms. */
  function settle(t0, quiet = 300) {
    return new Promise(resolve => {
      let last = performance.now();
      const observer = new MutationObserver(() => { last = performance.now(); });
      observer.observe(host, { childList: true, subtree: true, characterData: true });
      const check = () => {
        const now = performance.now();
        if (now - last >= quiet && ready(host)) { observer.disconnect(); resolve(last - t0); }
        else setTimeout(check, 25);
      };
      setTimeout(check, 25);
    });
  }
  const tasks = (from, to) => {
    const inside = longTasks.filter(task => task.startTime >= from && task.startTime <= to);
    return { longTasks: inside.length, tbt: inside.reduce((sum, task) => sum + Math.max(0, task.duration - 50), 0), longestTask: Math.max(0, ...inside.map(task => task.duration)) };
  };
  function mountRoot(messages) {
    root?.unmount();
    root = createRoot(host);
    flushSync(() => root.render(<Transcript initial={messages} />));
  }
  window.bench = {
    name,
    async mount(messages) {
      const t0 = performance.now();
      mountRoot(messages);
      const ms = await settle(t0);
      return { ms, ...tasks(t0, t0 + ms) };
    },
    /** Feeds `text` in `chunk`-character tokens, one per animation frame. */
    async stream(text, { chunk = 24, id = 'live' } = {}) {
      mountRoot([{ id, role: 'assistant', text: '', done: false }]);
      await new Promise(requestAnimationFrame);
      const frames = [];
      const t0 = performance.now();
      let previous = t0;
      for (let at = chunk; ; at += chunk) {
        const done = at >= text.length;
        set([{ id, role: 'assistant', text: text.slice(0, Math.min(at, text.length)), done }]);
        if (done) break;
        await new Promise(requestAnimationFrame);
        const now = performance.now(); frames.push(now - previous); previous = now;
      }
      const fed = performance.now() - t0;
      const ms = await settle(t0);
      frames.sort((a, b) => a - b);
      const pick = q => frames[Math.min(frames.length - 1, Math.floor(q * frames.length))];
      return { ms, fedMs: fed, tokens: frames.length + 1, frameP50: pick(0.5), frameP95: pick(0.95), frameMax: frames.at(-1), slowFrames: frames.filter(f => f > 50).length, ...tasks(t0, t0 + ms) };
    },
    async memory() {
      for (let i = 0; i < 3; i++) { globalThis.gc?.(); await new Promise(r => setTimeout(r, 50)); }
      const result = { mainHeap: performance.memory?.usedJSHeapSize ?? null, domNodes: document.getElementsByTagName('*').length };
      return result;
    },
    unmount() { root?.unmount(); root = undefined; },
  };
  window.benchReady = true;
}
