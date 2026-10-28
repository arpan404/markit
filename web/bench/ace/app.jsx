// Ace's web transcript prose: the real Prose component and its Markdown worker.
import { Prose } from '@/components/markdown/prose.tsx';
import { installBench } from '../common.jsx';

function Message({ message }) {
  return (
    <div className="group/answer relative min-w-0 font-prose text-prose leading-[1.6] tracking-[-0.005em] wrap-break-word">
      <Prose text={message.text} stream={`bench/${message.id}`} streaming={message.done === false} />
    </div>
  );
}
installBench({
  name: 'ace',
  Message,
  ready: host => !host.querySelector('[aria-busy="true"]') && [...host.querySelectorAll('.bench-message')].every(m => m.textContent.length > 0),
});
