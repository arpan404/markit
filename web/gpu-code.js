// Wasm owns syntax; WebGPU draws visible glyphs. No readback or animation loop.
const MAX_GLYPHS = 32768;
const STRIDE = 6;
const SHADER = `
struct View { size: vec2f, quad: vec2f, atlas: vec2f, cell: vec2f, scroll: vec2f }
@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var glyphs: texture_2d<f32>;
@group(0) @binding(2) var glyphSampler: sampler;
struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f, @location(1) color: vec3f }
@vertex fn vertex(@builtin(vertex_index) v: u32, @location(0) origin: vec2f, @location(1) glyph: f32, @location(2) color: vec3f) -> Vertex {
  let corners = array<vec2f,6>(vec2f(0,0),vec2f(1,0),vec2f(0,1),vec2f(0,1),vec2f(1,0),vec2f(1,1));
  let corner = corners[v]; let id = u32(glyph);
  var out: Vertex;
  out.position = vec4f((origin - view.scroll + corner * view.quad) / view.size * vec2f(2,-2) + vec2f(-1,1),0,1);
  out.uv = (vec2f(f32(id % 16),f32(id / 16)) + corner) * view.cell / view.atlas;
  out.color = color; return out;
}
@fragment fn fragment(v: Vertex) -> @location(0) vec4f {
  let alpha = textureSample(glyphs,glyphSampler,v.uv).a;
  return vec4f(v.color * alpha, alpha);
}`;
const palette = new Map();
function rgb(hex) {
  if (!palette.has(hex)) {
    if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error('Unsupported syntax palette');
    const n = Number.parseInt(hex.slice(1),16);
    palette.set(hex, [(n >>> 16) / 255, ((n >>> 8) & 255) / 255, (n & 255) / 255]);
  }
  return palette.get(hex);
}

export class GpuCodeRenderer {
  #ready; #views = new Set(); #disposed = false;
  async #device() {
    if (!this.#ready) this.#ready = (async () => {
      const adapter = await navigator.gpu?.requestAdapter({powerPreference:'low-power'});
      if (!adapter || adapter.info?.isFallbackAdapter || this.#disposed) return null;
      const device = await adapter.requestDevice();
      if (this.#disposed) { device.destroy(); return null; }
      const format = navigator.gpu.getPreferredCanvasFormat();
      const shader = device.createShaderModule({code:SHADER});
      const pipeline = await device.createRenderPipelineAsync({
        layout:'auto', vertex:{module:shader,entryPoint:'vertex',buffers:[{
          arrayStride:STRIDE * 4,stepMode:'instance',attributes:[
            {shaderLocation:0,offset:0,format:'float32x2'},
            {shaderLocation:1,offset:8,format:'float32'},
            {shaderLocation:2,offset:12,format:'float32x3'}
          ]
        }]}, fragment:{module:shader,entryPoint:'fragment',targets:[{format,blend:{
          color:{srcFactor:'one',dstFactor:'one-minus-src-alpha'},
          alpha:{srcFactor:'one',dstFactor:'one-minus-src-alpha'}
        }}]}, primitive:{topology:'triangle-list'}
      }).catch(error => { device.destroy(); throw error; });
      const state = {device,format,pipeline,atlases:new Map()};
      device.lost.then(() => { for (const view of [...this.#views]) view.destroy(); });
      device.addEventListener('uncapturederror', () => { for (const view of [...this.#views]) view.destroy(); });
      if (this.#disposed) { device.destroy(); return null; }
      return state;
    })().catch(() => null);
    return this.#ready;
  }
  async attach(pre, rows, setRows, onDisposed = () => {}) {
    // Complex shaping, RTL, Unicode, and diff backgrounds use the DOM path.
    // Their exact text, selection, and colors must survive on every backend.
    const bounds = pre.getBoundingClientRect();
    if (document.hidden || !bounds.width || bounds.bottom < 0 || bounds.top > innerHeight) return null;
    if (rows.some(row => /[^\t\n\r\x20-\x7e]/.test(row.textContent)) || pre.querySelector('[class*="mk-diff-"]')) return null;
    const state = await this.#device();
    if (!state || this.#disposed || !pre.isConnected || this.#views.size >= 4) return null;
    try {
      const view = new GpuCodeView(state, pre, rows, setRows, view => { this.#views.delete(view); onDisposed(); });
      this.#views.add(view); return view;
    } catch { return null; }
  }
  destroy() {
    this.#disposed = true;
    for (const view of [...this.#views]) view.destroy();
    this.#ready?.then(state => state?.device.destroy());
  }
}

class GpuCodeView {
  #state; #pre; #rows; #setRows; #onDestroy; #canvas; #context; #host;
  #vertices; #uniform; #bind; #atlas; #data = new Float32Array(STRIDE * 4096); #capacity = 4096;
  #resident; #viewData = new Float32Array(12);
  #abort = new AbortController(); #resize; #intersection; #mutation; #media;
  #frame = 0; #visible = false; #disposed = false; #lines; #padding; #gutter; #plainRows; #activated = false; #font;
  get disposed() { return this.#disposed; }
  constructor(state, pre, rows, setRows, onDestroy) {
    this.#state = state; this.#pre = pre; this.#rows = rows; this.#setRows = setRows; this.#onDestroy = onDestroy;
    try {
    const code = pre.querySelector('code');
    const style = getComputedStyle(code), rowStyle = getComputedStyle(rows[0]);
    const font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    this.#font = font;
    const lineHeight = Number.parseFloat(rowStyle.height);
    const fontSize = Number.parseFloat(style.fontSize);
    if (lineHeight !== 22 || fontSize < 6 || fontSize > 24 || !document.fonts.check(font) || style.letterSpacing !== 'normal' && Number.parseFloat(style.letterSpacing) !== 0) throw new Error('Unsupported code metrics');
    const measure = document.createElement('canvas').getContext('2d'); measure.font = font;
    const advance = measure.measureText('M').width;
    if (Math.abs(advance - measure.measureText('i').width) > 0.01) throw new Error('Code font must be monospace');
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const key = `${font}|${dpr}`;
    let atlas = state.atlases.get(key);
    if (!atlas) {
      const cell = [Math.ceil(advance * dpr) + 2, Math.ceil(lineHeight * dpr)];
      const bitmap = document.createElement('canvas'); bitmap.width = cell[0] * 16; bitmap.height = cell[1] * 6;
      const ctx = bitmap.getContext('2d'); ctx.font = font;
      const metrics = ctx.measureText('Mg');
      const ascent = metrics.fontBoundingBoxAscent ?? metrics.actualBoundingBoxAscent;
      const descent = metrics.fontBoundingBoxDescent ?? metrics.actualBoundingBoxDescent;
      const baseline = (lineHeight - ascent - descent) / 2 + ascent;
      ctx.scale(dpr,dpr); ctx.font = font; ctx.fillStyle = '#fff';
      for (let n=0;n<95;n++) ctx.fillText(String.fromCharCode(n+32), (n%16*cell[0]+1)/dpr, Math.floor(n/16)*lineHeight+baseline);
      const texture = state.device.createTexture({size:[bitmap.width,bitmap.height],format:'rgba8unorm',usage:GPUTextureUsage.TEXTURE_BINDING|GPUTextureUsage.COPY_DST|GPUTextureUsage.RENDER_ATTACHMENT});
      state.device.queue.copyExternalImageToTexture({source:bitmap},{texture},[bitmap.width,bitmap.height]);
      atlas = {texture,cell,size:[bitmap.width,bitmap.height],advance,dpr,key,refs:0}; state.atlases.set(key,atlas);
    }
    atlas.refs++; this.#atlas = atlas;
    // Read token colors once. Text nodes remain owned by the retained DOM rows.
    this.#lines = rows.map(row => [...row.querySelectorAll('span[style]')].map(span => ({
      text:span.textContent.replace(/[\r\n]/g,''), light:rgb(span.style.getPropertyValue('--mk-light').trim()), dark:rgb(span.style.getPropertyValue('--mk-dark').trim())
    })));
    this.#padding = Number.parseFloat(getComputedStyle(pre).paddingTop) || 0;
    const first = rows.find(row => row.firstElementChild?.textContent);
    if (!first?.firstElementChild?.isConnected) throw new Error('Code viewport must be mounted');
    this.#gutter = first.firstElementChild.getBoundingClientRect().left - code.getBoundingClientRect().left;
    const device = state.device;
    this.#vertices = device.createBuffer({size:this.#capacity*STRIDE*4,usage:GPUBufferUsage.VERTEX|GPUBufferUsage.COPY_DST});
    this.#uniform = device.createBuffer({size:48,usage:GPUBufferUsage.UNIFORM|GPUBufferUsage.COPY_DST});
    this.#bind = device.createBindGroup({layout:state.pipeline.getBindGroupLayout(0),entries:[
      {binding:0,resource:{buffer:this.#uniform}},{binding:1,resource:atlas.texture.createView()},
      {binding:2,resource:device.createSampler({minFilter:'linear',magFilter:'linear'})}
    ]});
    this.#canvas = document.createElement('canvas'); this.#canvas.className = 'mk-gpu-canvas'; this.#canvas.setAttribute('aria-hidden','true');
    this.#context = this.#canvas.getContext('webgpu');
    if (!this.#context) throw new Error('WebGPU canvas unavailable');
    this.#context.configure({device,format:state.format,alphaMode:'premultiplied'});
    this.#host = document.createElement('div'); this.#host.className = 'mk-gpu-host';
    pre.replaceWith(this.#host); this.#host.append(this.#canvas,pre);
    this.#plainRows = rows.map(row => { const plain = row.cloneNode(false); plain.textContent = row.textContent; return plain; });
    pre.dataset.gpuFrames = '0';
    const signal = this.#abort.signal;
    pre.addEventListener('scroll', () => this.invalidate(), {passive:true,signal});
    window.addEventListener('resize', () => this.destroy(), {signal});
    document.addEventListener('visibilitychange', () => { if(document.hidden) this.#cancel(); else this.invalidate(); }, {signal});
    document.fonts.addEventListener('loadingdone', () => this.destroy(), {signal});
    this.#media = matchMedia('(prefers-color-scheme:dark)'); this.#media.addEventListener('change', () => this.invalidate(), {signal});
    this.#resize = new ResizeObserver(() => this.invalidate()); this.#resize.observe(pre);
    this.#intersection = new IntersectionObserver(entries => {this.#visible = entries[0].isIntersecting; if(this.#visible) this.invalidate(); else this.#cancel();}); this.#intersection.observe(pre);
    this.#mutation = new MutationObserver(() => this.invalidate());
    this.#mutation.observe(document.documentElement,{attributes:true,attributeFilter:['class','style']});
    } catch (error) { this.destroy(); throw error; }
  }
  #cancel() { if(this.#frame) cancelAnimationFrame(this.#frame); this.#frame = 0; }
  invalidate() {
    if (this.#disposed || this.#frame || !this.#visible || document.hidden) return;
    this.#frame = requestAnimationFrame(() => {this.#frame=0; try {this.#draw();} catch {this.destroy();}});
  }
  #draw() {
    if (!this.#pre.isConnected) { this.destroy(); return; }
    const pre = this.#pre, atlas = this.#atlas, device = this.#state.device;
    const style = getComputedStyle(pre.querySelector('code'));
    if (`${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}` !== this.#font || Number.parseFloat(style.lineHeight) !== 22 || Number.parseFloat(getComputedStyle(pre).tabSize) !== 4) {this.destroy();return;}
    const width = pre.clientWidth, height = pre.clientHeight;
    if (!width || !height) return;
    const pixelWidth = Math.round(width*atlas.dpr), pixelHeight = Math.round(height*atlas.dpr);
    if (pixelWidth * pixelHeight > 2 * 1024 * 1024 || pixelWidth > device.limits.maxTextureDimension2D || pixelHeight > device.limits.maxTextureDimension2D) {this.destroy();return;}
    if (this.#canvas.width !== pixelWidth || this.#canvas.height !== pixelHeight) {
      this.#canvas.width=pixelWidth; this.#canvas.height=pixelHeight;
      this.#canvas.style.width=`${width}px`; this.#canvas.style.height=`${height}px`;
    }
    const first = Math.max(0,Math.floor((pre.scrollTop-this.#padding)/22));
    const last = Math.min(this.#lines.length,first+Math.ceil(height/22)+2);
    const dark = this.#media.matches;
    let resident = this.#resident;
    if (!resident || first < resident.first || last > resident.last || pre.scrollLeft < resident.left || pre.scrollLeft+width > resident.right || dark !== resident.dark) {
    const chunkFirst = Math.floor(first/128)*128, chunkLast = Math.min(this.#lines.length,chunkFirst+256);
    const left = Math.max(0,pre.scrollLeft-128), right = pre.scrollLeft+width+128;
    const offsets = new Uint32Array(chunkLast-chunkFirst+1);
    let count = 0;
    for(let row=chunkFirst;row<chunkLast;row++) {
      offsets[row-chunkFirst] = count;
      let column = 0;
      for(const token of this.#lines[row]) {
        const color = dark ? token.dark : token.light;
        for(const char of token.text) {
          if(char === '\t') {column += 4-column%4;continue;}
          const x = this.#gutter + column*atlas.advance; column++;
          if(char === ' ' || x < left-atlas.advance || x > right) continue;
          if(count === MAX_GLYPHS) {this.destroy();return;}
          if(count === this.#capacity) {
            this.#capacity *= 2; const grown = new Float32Array(this.#capacity*STRIDE); grown.set(this.#data); this.#data=grown;
            this.#vertices.destroy(); this.#vertices=device.createBuffer({size:this.#data.byteLength,usage:GPUBufferUsage.VERTEX|GPUBufferUsage.COPY_DST});
          }
          const at=count++*STRIDE;
          this.#data[at]=x-1/atlas.dpr; this.#data[at+1]=this.#padding+row*22;
          this.#data[at+2]=char.charCodeAt(0)-32; this.#data.set(color,at+3);
        }
      }
    }
    offsets[offsets.length-1] = count;
    if(count) device.queue.writeBuffer(this.#vertices,0,this.#data.buffer,0,count*STRIDE*4);
    resident = this.#resident = {first:chunkFirst,last:chunkLast,left,right,dark,offsets};
    }
    this.#viewData.set([width,height,atlas.cell[0]/atlas.dpr,22,...atlas.size,...atlas.cell,pre.scrollLeft,pre.scrollTop]);
    device.queue.writeBuffer(this.#uniform,0,this.#viewData);
    const from = resident.offsets[first-resident.first], to = resident.offsets[last-resident.first];
    const encoder=device.createCommandEncoder();
    const pass=encoder.beginRenderPass({colorAttachments:[{view:this.#context.getCurrentTexture().createView(),clearValue:[0,0,0,0],loadOp:'clear',storeOp:'store'}]});
    pass.setPipeline(this.#state.pipeline); pass.setBindGroup(0,this.#bind); pass.setVertexBuffer(0,this.#vertices); pass.draw(6,to-from,0,from); pass.end(); device.queue.submit([encoder.finish()]);
    if (!this.#activated) {
      this.#activated = true; pre.classList.add('mk-gpu-active'); pre.dataset.markitBackend = 'webgpu'; this.#setRows(this.#plainRows);
    }
    pre.dataset.gpuFrames=String(Number(pre.dataset.gpuFrames)+1);
  }
  destroy() {
    if(this.#disposed) return; this.#disposed=true; this.#cancel(); this.#abort.abort();
    this.#resize?.disconnect(); this.#intersection?.disconnect(); this.#mutation?.disconnect(); this.#context?.unconfigure();
    this.#vertices?.destroy(); this.#uniform?.destroy();
    if(this.#atlas && --this.#atlas.refs === 0) {this.#atlas.texture.destroy();this.#state.atlases.delete(this.#atlas.key);}
    this.#pre.classList.remove('mk-gpu-active'); this.#pre.dataset.markitBackend='dom'; this.#setRows(this.#rows);
    this.#host?.replaceWith(this.#pre); this.#onDestroy(this);
    this.#rows=[]; this.#lines=[]; this.#plainRows=[]; this.#data=new Float32Array(0); this.#resident=undefined;
  }
}
