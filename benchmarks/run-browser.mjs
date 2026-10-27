import { chromium } from '../web/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
const port=4180;
const task=process.env.BENCH_FUNCTION ?? 'runBench';
const output=process.env.BENCH_OUTPUT ?? 'browser.json';
const server=spawn('python3',['benchmarks/serve.py',String(port)],{stdio:'ignore'});
let browser;
try {
  let ready=false;
  for(let i=0;i<50;i++){try{if((await fetch(`http://127.0.0.1:${port}/benchmarks/browser.html`)).ok){ready=true;break;}}catch{} await new Promise(r=>setTimeout(r,100));}
  if(!ready)throw new Error('Benchmark server failed to start');
  browser=await chromium.launch();
  const page=await browser.newPage();
  page.on('console',msg=>{if(msg.type()==='error')process.stderr.write(msg.text()+'\n');});
  await page.goto(`http://127.0.0.1:${port}/benchmarks/browser.html`);
  await page.waitForFunction(name=>typeof window[name]==='function',task);
  const result=await page.evaluate(name=>window[name](),task);
  result.browser_version=browser.version();
  await writeFile(`benchmarks/results/${output}`,JSON.stringify(result,null,2)+'\n');
  if(task !== 'runBench') { console.log(JSON.stringify(result,null,2)); } else console.log(JSON.stringify({cold:result.cold,html:result.html.map(({name,bytes,median_ms})=>({name,bytes,median_ms})),highlighting:result.highlighting.map(({name,bytes,median_ms})=>({name,bytes,median_ms})),streaming:result.streaming},null,2));
} finally { await browser?.close(); server.kill(); }
