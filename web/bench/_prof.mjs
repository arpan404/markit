import { chromium } from '@playwright/test';
import http from 'node:http'; import fs from 'node:fs'; import path from 'node:path';
const root='/workspace/markit/web';
const types={'.js':'text/javascript','.html':'text/html','.css':'text/css','.wasm':'application/wasm','.otf':'font/otf','.woff2':'font/woff2'};
const server=http.createServer((q,r)=>{const f=path.join(root,decodeURIComponent(q.url.split('?')[0]));if(!fs.existsSync(f)||fs.statSync(f).isDirectory()){r.writeHead(404);r.end();return;}r.writeHead(200,{'content-type':types[path.extname(f)]||'application/octet-stream'});fs.createReadStream(f).pipe(r);}).listen(0);
const port=server.address().port;
const { message, longAnswer } = await import(root+'/bench/fixtures.js');
const impl=process.argv[2]||'markit'; const throttle=Number(process.argv[3]||1); const reps=Number(process.argv[4]||3);
const b=await chromium.launch();
const tot=[];
for(let rep=0;rep<reps;rep++){
const c=await b.newContext({viewport:{width:1280,height:900}}); const p=await c.newPage();
await p.goto(`http://localhost:${port}/bench/${process.env.DIST||"dist"}/${impl}.html`); await p.waitForFunction(()=>window.benchReady);
await p.evaluate(m=>window.bench.mount([{id:'warm',role:'assistant',text:m}]),message(8));
const cdp=await c.newCDPSession(p);
await cdp.send('Profiler.enable'); await cdp.send('Profiler.setSamplingInterval',{interval:100});
if(throttle>1) await cdp.send('Emulation.setCPUThrottlingRate',{rate:throttle});
await cdp.send('Performance.enable');
const g=async()=>Object.fromEntries((await cdp.send('Performance.getMetrics')).metrics.map(m=>[m.name,m.value]));
const m0=await g();
if(rep===0) await cdp.send('Profiler.start');
await p.evaluate(t=>window.bench.stream(t,{chunk:24}),longAnswer());
const prof = rep===0 ? (await cdp.send('Profiler.stop')).profile : null;
const m1=await g(); const d=k=>((m1[k]-m0[k])*1e3).toFixed(1);
if(!process.env.QUIET) console.log('task',d('TaskDuration'),'script',d('ScriptDuration'),'layout',d('LayoutDuration'),'style',d('RecalcStyleDuration'),'layouts',m1.LayoutCount-m0.LayoutCount,'styles',m1.RecalcStyleCount-m0.RecalcStyleCount,'nodes',m1.Nodes);
tot.push((m1.TaskDuration-m0.TaskDuration)*1e3); (globalThis.ls??=[]).push((m1.LayoutDuration-m0.LayoutDuration+m1.RecalcStyleDuration-m0.RecalcStyleDuration+m1.ScriptDuration-m0.ScriptDuration)*1e3);
if(prof && process.env.TOP){const nodes=new Map(prof.nodes.map(n=>[n.id,n]));const self=new Map();
prof.samples.forEach((s,i)=>{const n=nodes.get(s);const k=n.callFrame.functionName+' '+path.basename(n.callFrame.url)+':'+n.callFrame.lineNumber;self.set(k,(self.get(k)||0)+(prof.timeDeltas[i]||0));});
[...self].sort((a,b)=>b[1]-a[1]).slice(0,Number(process.env.TOP)).forEach(([k,v])=>console.log((v/1000).toFixed(1),k));}
await c.close();}
tot.sort((a,b)=>a-b); const L=globalThis.ls.sort((a,b)=>a-b); console.log('median task', tot[(tot.length-1)>>1].toFixed(1), 'script+layout+style', L[(L.length-1)>>1].toFixed(1));
await b.close(); server.close();
