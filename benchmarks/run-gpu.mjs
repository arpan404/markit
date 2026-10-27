import { chromium } from '../web/node_modules/playwright/index.mjs';
import { writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
const port=4183;
const dense=process.env.MARKIT_GPU_DENSITY === 'dense';
const server=spawn('python3',['-m','http.server',String(port),'--bind','127.0.0.1','--directory','web'],{stdio:'ignore'});
let browser;
try {
  for(let i=0;i<50;i++){try{if((await fetch(`http://127.0.0.1:${port}`)).ok)break;}catch{} await new Promise(r=>setTimeout(r,100));}
  browser=await chromium.launch({headless:process.env.MARKIT_GPU_HEADLESS === '1'});
  const page=await browser.newPage({viewport:{width:1280,height:800}});
  const errors=[];page.on('pageerror',e=>errors.push(String(e)));page.on('console',m=>{if(m.type()==='error')errors.push(m.text());});
  await page.goto(`http://127.0.0.1:${port}`);await page.waitForFunction(()=>window.renderer);
  const adapter=await page.evaluate(async()=>{const a=await navigator.gpu?.requestAdapter({powerPreference:'low-power'});return a?{vendor:a.info.vendor,architecture:a.info.architecture,fallback:a.info.isFallbackAdapter}:null;});
  if(!adapter || adapter.fallback)throw new Error('Hardware WebGPU adapter required; software results are not hardware evidence');
  const cdp=await page.context().newCDPSession(page);await cdp.send('Performance.enable');
  const taskTime=async()=>{const {metrics}=await cdp.send('Performance.getMetrics');return metrics.find(m=>m.name==='TaskDuration').value;};
  const rows=[];
  for(const mode of ['dom','webgpu']) {
    const startup=await page.evaluate(async ({mode,dense})=>{
      const {MarkitRenderer}=await import('/index.js');window.renderer.destroy();
      window.renderer=new MarkitRenderer(document.querySelector('#preview'),{codeRendering:mode});
      window.benchSource=Array.from({length:dense?401:1000},(_,i)=>dense?`const v_${i} = f(1,2,3,4,5,6,7,8);`:`let value_${i} = ${i}; // source line ${i}`).join('\n')+'\n';
      const start=window.benchStart=performance.now();await window.renderer.render('```rust\n'+window.benchSource+'```');return performance.now()-start;
    },{mode,dense});
    if(mode==='webgpu')await page.waitForFunction(()=>document.querySelector('pre')?.dataset.markitBackend==='webgpu');
    const backendReady=await page.evaluate(async()=>{await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));return performance.now()-window.benchStart;});
    const samples=[];
    for(let batch=0;batch<9;batch++) {
      const begin=await taskTime();
      await page.evaluate(async()=>{
        const pre=document.querySelector('pre');
        for(let frame=0;frame<30;frame++)await new Promise(resolve=>requestAnimationFrame(()=>{pre.scrollTop=(frame*437)%(pre.scrollHeight-pre.clientHeight);requestAnimationFrame(resolve);}));
      });
      samples.push(((await taskTime())-begin)*1000/30);
    }
    await page.waitForTimeout(100);
    const frames=await page.evaluate(()=>Number(document.querySelector('pre').dataset.gpuFrames||0));
    await page.waitForTimeout(250);
    const info=await page.evaluate(()=>({backend:document.querySelector('pre').dataset.markitBackend,nodes:document.querySelector('pre').querySelectorAll('*').length,frames:Number(document.querySelector('pre').dataset.gpuFrames||0),copied:window.benchSource.length}));
    if(mode==='webgpu' && info.frames!==frames)throw new Error('Idle GPU view kept submitting work');
    const sorted=[...samples].sort((a,b)=>a-b);
    rows.push({mode,worker_and_dom_ms:startup,backend_ready_ms:backendReady,median_main_thread_task_ms_per_scroll:sorted[4],samples_ms:samples,frames:270,idle_gpu_submissions:info.frames-frames,visible_dom_nodes:info.nodes,backend:info.backend,source_chars:info.copied});
  }
  if(errors.length)throw new Error(errors.join('\n'));
  const result={timestamp:new Date().toISOString(),browser_version:browser.version(),adapter,density:dense?'dense':'sparse',rows,note:'Standard WebGPU, browser-selected adapter, no graphics backend or unsafe WebGPU flags. TaskDuration measures main-thread CPU tasks during two-rAF scroll steps, not GPU execution time, energy, or battery life. worker_and_dom_ms excludes asynchronous GPU initialization. backend_ready_ms includes readiness and two rAF callbacks, not a GPU timestamp or proven presentation time.'};
  await writeFile(`benchmarks/results/webgpu${dense?'-dense':''}.json`,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result,null,2));
}finally{await browser?.close();server.kill();}
