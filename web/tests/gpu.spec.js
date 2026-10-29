import { test, expect } from '@playwright/test';

test('WebGPU colors source, stays idle, scrolls, copies, changes theme and cleans up', async ({page,context}) => {
  const errors=[]; page.on('pageerror',error=>errors.push(String(error))); page.on('console',message=>{if(message.type()==='error')errors.push(message.text());});
  await page.goto('/'); await expect(page.locator('#status')).toContainText('Ready');
  const available=await page.evaluate(async()=>{const a=await navigator.gpu?.requestAdapter({powerPreference:'low-power'});return !!a && !a.info?.isFallbackAdapter;});
  test.skip(!available,'A hardware WebGPU adapter is required');
  await page.evaluate(async()=>{
    const {MarkitRenderer}=await import('/index.js');window.renderer.destroy();
    window.renderer=new MarkitRenderer(document.querySelector('#preview'),{codeRendering:'webgpu'});
    window.gpuSource=Array.from({length:1000},(_,i)=>`let value_${i} = ${i};`).join('\n')+'\n';
    await window.renderer.render('```rust\n'+window.gpuSource+'```');
  });
  const pre=page.locator('pre.mk-virtual-code');
  await expect(pre).toHaveAttribute('data-markit-backend','webgpu');
  // Capture in the draw frame, before WebGPU discards the presented texture.
  const coloredPixels=await page.evaluate(()=>new Promise(resolve=>{
    const pre=document.querySelector('pre');
    pre.addEventListener('scroll',()=>requestAnimationFrame(()=>{
    const canvas=document.querySelector('.mk-gpu-canvas');
    const copy=document.createElement('canvas');copy.width=canvas.width;copy.height=canvas.height;
    const ctx=copy.getContext('2d');ctx.drawImage(canvas,0,0);
    const rgba=ctx.getImageData(0,0,copy.width,copy.height).data;
    let colored=0;for(let i=0;i<rgba.length;i+=4)if(rgba[i+3]>100 && Math.abs(rgba[i]-rgba[i+1])>50)colored++;
    resolve(colored);
    }),{once:true});
    pre.scrollTop=1;
  }));
  expect(coloredPixels).toBeGreaterThan(100);
  const count=Number(await pre.getAttribute('data-gpu-frames'));
  await page.waitForTimeout(250);
  expect(Number(await pre.getAttribute('data-gpu-frames'))).toBe(count);
  expect(await pre.locator('.mk-line span').count()).toBe(0);
  await pre.evaluate(element=>{element.scrollTop=element.scrollHeight;});
  await expect(pre).toContainText('value_999');
  await expect.poll(async()=>Number(await pre.getAttribute('data-gpu-frames'))).toBeGreaterThan(count);
  const copied=await page.evaluate(()=>{const first=document.querySelector('.mk-line');const range=document.createRange();range.selectNodeContents(first);const s=getSelection();s.removeAllRanges();s.addRange(range);return s.toString();});
  expect(copied).toContain('let value_');
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await page.getByRole('button',{name:'Copy code',exact:true}).click();
  expect(await page.evaluate(()=>navigator.clipboard.readText())).toBe(await page.evaluate(()=>window.gpuSource));
  await page.emulateMedia({colorScheme:'dark'});
  await expect.poll(async()=>Number(await pre.getAttribute('data-gpu-frames'))).toBeGreaterThan(count+1);
  const frames=Number(await pre.getAttribute('data-gpu-frames'));
  await page.evaluate(()=>{document.querySelector('#preview').style.marginTop='3000px';});
  await page.waitForTimeout(150);
  const hiddenFrames=Number(await pre.getAttribute('data-gpu-frames'));
  await page.waitForTimeout(200);
  expect(Number(await pre.getAttribute('data-gpu-frames'))).toBe(hiddenFrames);
  expect(hiddenFrames).toBeGreaterThanOrEqual(frames);
  await page.evaluate(async()=>{document.querySelector('#preview').style.marginTop='';await window.renderer.render('Replacement paragraph.');});
  await expect(page.locator('.mk-gpu-canvas')).toHaveCount(0);
  expect(errors).toEqual([]);
});
