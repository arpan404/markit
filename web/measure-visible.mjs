import { chromium } from '@playwright/test';
import { writeFileSync } from 'node:fs';
const browser = await chromium.launch({headless:true});
const page = await browser.newPage(); await page.goto('http://127.0.0.1:4187');
await page.getByRole('status').filter({hasText:'Ready'}).waitFor();
const results = await page.evaluate(async () => {
  const wasm = await import('/pkg/markit_wasm.js'); await wasm.default();
  const { prepareCode } = await import('/code-lines.js');
  const median = a => a.sort((x,y)=>x-y)[Math.floor(a.length/2)];
  const results = [];
  for (const length of [1000,5000,50000]) {
    const source = Array.from({length},(_,i)=>`let value_${i} = ${i};`).join('\n')+'\n';
    // Input transport/parse excluded: compare building initial DOM from prepared
    // full HTML vs packed ranges, alternating order after three warmups.
    const html = wasm.renderHtml('```rust\n'+source+'```');
    const ranges = wasm.highlightCodePacked(source,'rust');
    const host = document.createElement('div'); document.body.append(host);
    const old = [], packed = [];
    for (let sample=0;sample<24;sample++) {
      const measure = legacy => {
        host.replaceChildren(); const start = performance.now();
        if (legacy) {
          host.innerHTML = html;
          const code = host.querySelector('code'), rows = [...code.children];
          code.style.minWidth = `${Math.min(4096,Math.max(...rows.map(r=>r.textContent.length))+7)}ch`;
          const top=document.createElement('span'), bottom=document.createElement('span');
          top.style.height='0px';bottom.style.height=`${(rows.length-31)*22}px`;
          code.replaceChildren(top,...rows.slice(0,31),bottom);
        } else {
          const section = document.createElement('section'); host.append(section);
          const rows = prepareCode(section,{source,info:{language:'rust',diff:false},ranges});
          const code=section.querySelector('code');code.style.minWidth=`${Math.min(4096,rows.width+7)}ch`;
          const top=document.createElement('span'), bottom=document.createElement('span');
          top.style.height='0px'; bottom.style.height=`${(rows.length-31)*22}px`;
          code.replaceChildren(top,...rows.slice(0,31),bottom);
        }
        const elapsed=performance.now()-start;
        if (sample>=3) (legacy?old:packed).push(elapsed);
      };
      measure(sample%2===0);measure(sample%2!==0);
    }
    host.remove(); results.push({lines:length,legacy_initial_dom_ms:median(old),packed_initial_dom_ms:median(packed)});
  }
  return results;
});
writeFileSync('../measurements/web-dom.json',JSON.stringify(results,null,2)+'\n');
console.log(results); await browser.close();
