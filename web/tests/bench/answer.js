// A long, deterministic agent answer for streaming benchmarks: numbered parts
// with inline formatting, nested and task lists, a code fence, a table and a
// quote, repeated until the answer reaches the requested size.
function part(n) {
  const limit = 120 + n * 6;
  const table = Array.from({ length: 6 }, (_, row) => `| queue-${n}-${row} | ${limit + row * 3} | ${(8 + ((n * 5 + row * 7) % 30)).toFixed(1)} | ${row % 2 ? 'no' : 'yes'} |`).join('\n');
  return `## ${n}. Backpressure for queue ${n}

Each queue now holds at most a fixed number of pending jobs. This part explains **what moved**, why the \`highWater\` mark matters, and how [the drain protocol](https://example.com/docs/queues#drain) reacts when a consumer stalls halfway through a batch. Jobs past the limit are *rejected* at the edge instead of piling up.

- Limit queue ${n} to ${limit} jobs
  - shed the oldest low-priority work first
  - keep \`jobId\` ordering stable across restarts
- Record the high-water mark before rejecting
- [x] load tests pass for queue ${n}
- [ ] run queue ${n} under production traffic

\`\`\`ts
export function admit(queue: Job[], job: Job, limit: number): boolean {
  // Queue ${n}: admit while under the limit, shed low priority otherwise.
  if (queue.length < limit) { queue.push(job); return true; }
  const victim = queue.findIndex((pending) => pending.priority < job.priority);
  if (victim < 0) return false;
  queue.splice(victim, 1);
  queue.push(job);
  return true;
}
\`\`\`

| queue | limit | p95 (ms) | shed |
| --- | ---: | ---: | --- |
${table}

> A consumer that falls behind queue ${n}'s limit sees rejections, not
> unbounded latency, so retries stay cheap.

1. Ship the limit for queue ${n}
2. Watch the rejection rate for a day
3. Delete the unbounded fallback

`;
}

export function longAnswer(chars = 40_000) {
  let text = 'Here is the queue work, part by part, with code and numbers.\n\n';
  for (let n = 1; text.length < chars; n++) text += part(n);
  return `${text}That is every queue.\n`;
}
