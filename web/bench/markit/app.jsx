import { Markit, preloadMarkit } from '../../react.js';

// Like Ace's Prose module, which starts loading its renderer when imported.
if (window.markitPreload) preloadMarkit();
import { installBench } from '../common.jsx';

function Message({ message }) {
  return <Markit markdown={message.text} finalized={message.done !== false} lazy={window.markitLazy || undefined} />;
}
installBench({
  name: 'markit',
  Message,
  // Lazy messages offscreen stay plain text by design; everything else must have rendered.
  ready: host => [...host.querySelectorAll('.bench-message')].every(m => m.querySelector('.mk-block, .mk-pending')),
});
