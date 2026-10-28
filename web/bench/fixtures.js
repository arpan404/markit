// Deterministic chat transcript fixtures shared by the Markit and Ace harnesses.
const words = 'the renderer parses streamed markdown into stable blocks while code fences math and tables keep their layout as tokens arrive from the model'.split(' ');
function sentence(seed, length) {
  let out = [];
  for (let i = 0; i < length; i++) out.push(words[(seed * 7 + i * 13) % words.length]);
  out[0] = out[0][0].toUpperCase() + out[0].slice(1);
  return out.join(' ') + '.';
}
function paragraph(seed, sentences = 4) {
  return Array.from({ length: sentences }, (_, i) => sentence(seed + i, 9 + ((seed + i) % 7))).join(' ');
}
function tsCode(seed, lines) {
  const out = [];
  for (let i = 0; i < lines; i++) {
    switch ((seed + i) % 6) {
      case 0: out.push(`export function step${seed}_${i}(value: number): number {`); break;
      case 1: out.push(`  const scaled = value * ${i + 1} + ${seed}; // scale the input`); break;
      case 2: out.push(`  if (scaled > 1000) return Math.floor(scaled / 2);`); break;
      case 3: out.push(`  const label = \`step \${scaled}\`;`); break;
      case 4: out.push(`  console.log("value", label, [1, 2, 3].map(x => x * ${i}));`); break;
      default: out.push(`  return scaled;\n}`);
    }
  }
  return out.join('\n');
}
function rustCode(seed, lines) {
  return Array.from({ length: lines }, (_, i) => i % 4 === 0
    ? `fn item_${seed}_${i}(value: i64) -> i64 {`
    : i % 4 === 3 ? '}' : `    let x = value.wrapping_add(${i}); /* step */ x * 2`).join('\n');
}
function diff(seed, lines) {
  const out = [`--- a/src/file${seed}.ts`, `+++ b/src/file${seed}.ts`, `@@ -1,${lines} +1,${lines} @@`];
  for (let i = 0; i < lines; i++) out.push(i % 3 === 0 ? `-const old${i} = ${i};` : i % 3 === 1 ? `+const new${i} = ${i * 2};` : ` const same${i} = true;`);
  return out.join('\n');
}
const formulas = ['x^2 + y^2 = z^2', '\\frac{a+b}{c-d}', '\\sum_{i=1}^{n} i = \\frac{n(n+1)}{2}', '\\int_0^1 x^2\\,dx = \\frac{1}{3}', '\\sqrt{2} \\approx 1.414', 'e^{i\\pi} + 1 = 0', '\\alpha + \\beta = \\gamma', '\\lim_{h \\to 0} \\frac{f(x+h)-f(x)}{h}'];

export function message(seed) {
  const parts = [`## Step ${seed}`, paragraph(seed)];
  switch (seed % 5) {
    case 0: parts.push('```ts title="src/step.ts"\n' + tsCode(seed, 18) + '\n```', paragraph(seed + 3, 2)); break;
    case 1: parts.push('- ' + sentence(seed, 8) + '\n- **' + sentence(seed + 1, 5) + '** with `inline code`\n- [a link](https://example.com/' + seed + ')', paragraph(seed + 2, 2)); break;
    case 2: parts.push('```diff\n' + diff(seed, 12) + '\n```'); break;
    case 3: parts.push(`Inline math $${formulas[seed % formulas.length]}$ in text.`, `$$\n${formulas[(seed + 3) % formulas.length]}\n$$`); break;
    default: parts.push('| Name | Value | Note |\n| --- | ---: | --- |\n' + Array.from({ length: 5 }, (_, i) => `| row ${i} | ${seed * i} | ${sentence(seed + i, 3)} |`).join('\n'), '```rust\n' + rustCode(seed, 12) + '\n```');
  }
  return parts.join('\n\n') + '\n';
}

export function transcript(count) {
  return Array.from({ length: count }, (_, i) => ({ id: `m${i}`, role: i % 2 ? 'assistant' : 'user', text: i % 2 ? message(i) : paragraph(i, 2) }));
}

/** One long assistant answer: prose, a 60-line fence, a diff, math and a list. */
export function longAnswer() {
  const parts = ['# Implementation plan', paragraph(1, 5), '```ts title="src/renderer.ts"\n' + tsCode(3, 60) + '\n```', paragraph(4, 3),
    '```diff\n' + diff(5, 30) + '\n```', 'The cost model:', ...formulas.map((f, i) => i % 2 ? `$$\n${f}\n$$` : `Inline $${f}$ and more text.`),
    Array.from({ length: 8 }, (_, i) => `- ${sentence(i, 10)} \`step_${i}\``).join('\n'), '```rust\n' + rustCode(9, 40) + '\n```', paragraph(12, 4)];
  return parts.join('\n\n') + '\n';
}
