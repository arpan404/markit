import { unpackHighlight } from '../web/highlight.js';
import { fixture, markdownSizes, codeSizes } from './fixtures.mjs';
const rounds = 9, roundMs = 70;
let sink = 0;
function percentile(values, p) { return [...values].sort((a,b)=>a-b)[Math.min(values.length-1,Math.ceil(values.length*p)-1)]; }
function measure(name, source, operation, extra = {}) {
  const warm = performance.now(); let index = 0;
  while (performance.now()-warm < 50) sink ^= operation(index++);
  const samples = [], counts = [];
  for (let batch=0;batch<rounds;batch++) {
    const start = performance.now(); let count = 0;
    do { sink ^= operation(index++); count++; } while (performance.now()-start < roundMs);
    samples.push((performance.now()-start)/count); counts.push(count);
  }
  return { name, bytes:new TextEncoder().encode(source).length, median_ms:percentile(samples,.5), p95_batch_ms:percentile(samples,.95), samples_ms:samples, iterations:counts.reduce((a,b)=>a+b,0), ...extra };
}
function shape(html) {
  const template = document.createElement('template'); template.innerHTML = html;
  return { text:[...template.content.querySelectorAll('h1,h2,h3,p,li,th,td')].map(node=>node.textContent.replace(/\s+/g,' ').trim()).join(' | '), headings:template.content.querySelectorAll('h1,h2,h3').length, rows:template.content.querySelectorAll('tr').length, tasks:template.content.querySelectorAll('input[type=checkbox]').length };
}
window.runBench = async () => {
  const status = document.querySelector('#status');
  const cold = [];
  let start = performance.now();
  const wasm = await import('../web/pkg/markit_wasm.js'); await wasm.default();
  sink ^= wasm.renderHtml(fixture(4096)).length;
  cold.push({name:'Markit Wasm import + init + first HTML',ms:performance.now()-start});
  start = performance.now();
  const marked = await import('./artifacts/marked.esm.js'); sink ^= marked.marked.parse(fixture(4096),{gfm:true}).length;
  cold.push({name:'Marked import + first HTML',ms:performance.now()-start});
  start = performance.now();
  const ace = await import('./artifacts/ace/apps/web/src/components/markdown/highlight.js'); sink ^= ace.highlight(fixture(4096,'code'),'ts').length;
  cold.push({name:'Ace lexical import + first highlight',ms:performance.now()-start});
  start = performance.now();
  const prism = await import('./artifacts/ace/packages/ui-core/src/source-highlight.js');
  const prismFirst = await prism.sourceHighlight(fixture(4096,'code'),'ts'); sink ^= prismFirst.length;
  cold.push({name:'Ace Prism import + TS grammar + first highlight',ms:performance.now()-start});
  const html = [], highlighting = [], validation = [];
  for (const size of markdownSizes) {
    const source = fixture(size);
    const markitHtml = wasm.renderHtml(source), markedHtml = marked.marked.parse(source,{gfm:true});
    const equal = JSON.stringify(shape(markitHtml)) === JSON.stringify(shape(markedHtml));
    if(!equal) {
      const a=shape(markitHtml),b=shape(markedHtml);let index=0;while(index<Math.min(a.text.length,b.text.length)&&a.text[index]===b.text[index])index++;
      throw new Error(JSON.stringify({size,a:{...a,text:a.text.slice(Math.max(0,index-60),index+180)},b:{...b,text:b.text.slice(Math.max(0,index-60),index+180)},index}));
    }
    validation.push({fixture:size,html_text_headings_table_rows_tasks_match:true});
    status.textContent = `HTML ${size} bytes`;
    html.push(measure('Markit Wasm HTML',source,()=>wasm.renderHtml(source).length,{category:'html'}));
    html.push(measure('Marked HTML',source,()=>marked.marked.parse(source,{gfm:true}).length,{category:'html'}));
  }
  for (const size of codeSizes) {
    const source = fixture(size,'code');
    const colored = JSON.parse(wasm.highlightCode(source,'ts'));
    if(colored.plain || colored.tokens.map(t=>t.text).join('')!==source) throw new Error('Markit highlight fallback or text mismatch');
    if(ace.highlight(source,'ts').map(t=>t.text).join('')!==source) throw new Error('Ace text mismatch');
    const prepared = await prism.sourceHighlight(source,'ts');
    if(prepared.map(line=>line.map(t=>t.text).join('')).join('\n')!==source) throw new Error('Prism text mismatch');
    // Unique suffixes force Markit's content cache to miss. Cached-file timings
    // are measured separately, never compared against uncached tokenization.
    status.textContent = `Lexical highlighting ${size} bytes`;
    highlighting.push(measure('Markit Wasm lexical + JSON decode',source,i=>JSON.parse(wasm.highlightCode(`${source}\n// benchmark-${i}`,'ts')).tokens.length,{category:'lexical',cache:'miss'}));
    highlighting.push(measure('Markit Wasm lexical ranges + tokens',source,i=>{const input=`${source}\n// packed-benchmark-${i}`;return unpackHighlight(input,wasm.highlightCodePacked(input,'ts')).tokens.length;},{category:'lexical',cache:'miss'}));
    highlighting.push(measure('Ace JS lexical',source,i=>ace.highlight(`${source}\n// benchmark-${i}`,'ts').length,{category:'lexical',cache:'none'}));
    // Prism's actual sourceHighlight wrapper is async; measure batches after
    // grammar loading, including its flat colored line conversion.
    const samples = []; let iterations = 0;
    for(let batch=0;batch<rounds;batch++) {
      const begin=performance.now(); let count=0;
      do { const lines=await prism.sourceHighlight(`${source}\n// benchmark-${iterations++}`,'ts'); sink ^= lines.length; count++; } while(performance.now()-begin<roundMs);
      samples.push((performance.now()-begin)/count);
    }
    highlighting.push({name:'Ace Prism colored lines',bytes:new TextEncoder().encode(source).length,median_ms:percentile(samples,.5),p95_batch_ms:percentile(samples,.95),samples_ms:samples,iterations,category:'full_grammar',cache:'source miss; grammar warm'});
    highlighting.push(measure('Markit Wasm repeated file + JSON decode',source,()=>JSON.parse(wasm.highlightCode(source,'ts')).tokens.length,{category:'cached',cache:'hit'}));
  }
  status.textContent = 'Optional grammar first use';
  start = performance.now();
  const grammar = await import('./artifacts/grammars-web/markit_wasm.js'); await grammar.default();
  const firstGrammar = JSON.parse(grammar.highlightGrammar(fixture(4096,'code'),'ts'));
  if(firstGrammar.plain) throw new Error('Full grammar unexpectedly plain');
  cold.push({name:'Markit full grammar import + init + first TS highlight',ms:performance.now()-start});
  for(const size of codeSizes) {
    const source=fixture(size,'code');
    const check=JSON.parse(grammar.highlightGrammar(source,'ts'));
    if(check.plain || check.tokens.map(token=>token.text).join('')!==source)throw new Error('Full grammar fallback or source mismatch');
    highlighting.push(measure('Markit Wasm full grammar + JSON decode',source,i=>JSON.parse(grammar.highlightGrammar(`${source}\n// benchmark-${i}`,'ts')).tokens.length,{category:'full_grammar',cache:'miss; grammar warm'}));
  }
  // Isolate one append after the same settled history; setup is not timed.
  const { BlockStream } = await import('./artifacts/ace/apps/web/src/components/markdown/block-stream.js');
  const prefix = `${fixture(131072)}\n\nOpen tail`;
  const streaming=[];
  for(const name of ['Markit Wasm HTML patch + JSON decode','Ace BlockStream tokens']) {
    const samples=[];
    for(let batch=0;batch<9;batch++) {
      let total=0;
      for(let run=0;run<100;run++) {
        const stream=name.startsWith('Markit') ? new wasm.MarkdownStream() : new BlockStream();
        if(name.startsWith('Markit')) stream.append(prefix,false); else stream.push(prefix,false);
        const begin=performance.now();
        const result=name.startsWith('Markit') ? JSON.parse(stream.append(' continues',false)) : stream.push(' continues',false);
        total += performance.now()-begin; sink ^= name.startsWith('Markit') ? result.blocks.length : result.open.length;
        stream.free?.();
      }
      samples.push(total/100);
    }
    streaming.push({name,history_bytes:new TextEncoder().encode(prefix).length,median_ms:percentile(samples,.5),p95_batch_ms:percentile(samples,.95),samples_ms:samples,iterations:900,note:'Different outputs: HTML patches versus token patches; no speedup ratio.'});
  }
  const result={timestamp:new Date().toISOString(),user_agent:navigator.userAgent,cross_origin_isolated:crossOriginIsolated,highlight_language:'typescript',rounds,round_ms:roundMs,cold,html,highlighting,streaming,validation,sink};
  document.querySelector('#results').textContent=JSON.stringify(result,null,2); status.textContent='Benchmark complete';
  return result;
};

window.runMathBench = async () => {
  let start=performance.now();
  const wasm=await import('../web/pkg/markit_wasm.js'); await wasm.default();
  const first=wasm.renderHtml('$$x^2+1$$');
  if(!first.includes('role="math"'))throw new Error('Math fallback');
  const cold=[{name:'Markit module + init + first math',ms:performance.now()-start}];
  start=performance.now();
  const katex=await import('./artifacts/katex.mjs');
  const options={displayMode:true,throwOnError:true,trust:false,strict:'ignore',output:'htmlAndMathml'};
  katex.renderToString('x^2+1',options);
  cold.push({name:'KaTeX module + first math',ms:performance.now()-start});
  const math=[];
  for(const [name,formula] of [['simple',String.raw`x^2+1`],['sum_fraction',String.raw`\sum_{i=1}^{n} i^2 = \frac{n(n+1)(2n+1)}{6}`]]) {
    if(!wasm.renderHtml(`$$${formula}$$`).includes('role="math"'))throw new Error(`Math unsupported: ${name}`);
    math.push(measure('Markit Wasm SVG, uncached',formula,i=>wasm.renderHtml(`$$${formula}+${i}$$`).length,{category:'math',formula:name,cache:'miss'}));
    math.push(measure('KaTeX HTML + MathML, uncached',formula,i=>katex.renderToString(`${formula}+${i}`,options).length,{category:'math',formula:name,cache:'none'}));
    math.push(measure('Markit Wasm math, repeated formula',formula,()=>wasm.renderHtml(`$$${formula}$$`).length,{category:'math',formula:name,cache:'hit'}));
  }
  return {timestamp:new Date().toISOString(),user_agent:navigator.userAgent,cross_origin_isolated:crossOriginIsolated,cold,math,note:'Different output formats. Markit includes Markdown parsing, SVG path output; KaTeX emits HTML+MathML. No equivalence or speedup claim.'};
};
