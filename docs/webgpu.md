# WebGPU code rendering

The browser package uses standard `navigator.gpu` and WGSL. It has no Metal, Vulkan, Direct3D, macOS, or vendor-specific rendering calls. The browser selects its backend. The test runner and benchmark use an ordinary browser with no graphics-backend override or unsafe WebGPU flags.

Wasm produces the syntax colors. WebGPU draws glyphs in eligible large ASCII code viewports. Native iOS and Android clients continue to consume the shared Rust syntax data through their native bridges.

```js
const renderer = new MarkitRenderer(element, { codeRendering: 'webgpu' });
```

The local demo can request this path with `/?codeRendering=webgpu`, followed by its large-source control.

`codeRendering` accepts `auto`, `dom`, or `webgpu`. `auto` currently keeps the text renderer. Explicit `webgpu` attempts the GPU path on code blocks over 400 lines and falls back when unsupported. Unicode shaping, bidirectional text, diff backgrounds, unavailable or lost devices, changed font metrics, and unsupported geometry retain the text renderer. Visible text remains selectable and accessible, and the copy control retains the complete source.

## Efficiency

- Request a low-power adapter through the standard API. This is a browser hint, not a battery guarantee.
- Share the device, pipeline, and glyph atlas between views. Rasterize glyph masks once; theme changes reuse the same atlas.
- Upload bounded resident chunks and draw only the visible line range. Ordinary scrolling reuses the uploaded vertices and changes viewport uniforms.
- Coalesce changes into one pending animation-frame callback. There is no animation loop, GPU polling, production readback, or keepalive timer.
- Suspend submissions for hidden documents and offscreen views. Initially hidden/offscreen code stays on the text renderer.
- Limit each renderer to four GPU views, 32,768 resident glyphs per view, and 2,097,152 canvas pixels per view. Raster scale is capped at 2. Size limits or allocation/device failures restore text rendering.
- Release buffers, textures, observers, and event handlers when views are replaced or the renderer is destroyed.

The retained source and selection DOM still consume memory. Bounded GPU allocations are not a measurement of total process memory or energy.

## Validation

`npm --prefix web test` covers text fallback when WebGPU is unavailable, Unicode and diff preservation, source copying, and existing renderer flows. `npm --prefix web run test:gpu` runs a separate hardware test for colored GPU pixels, idle submissions, scrolling, selection, full-source copying, theme changes, offscreen suspension, and cleanup. It skips when the browser cannot provide a hardware adapter. The pixel assertion captures within the draw frame before presentation discards the canvas texture; production rendering does not read pixels back.

Run the hardware benchmark sequentially on an idle machine with your own driver script.

The hardware suite and benchmark use a normal visible Chromium window by default. Set `MARKIT_GPU_HEADLESS=1` only when that environment exposes standard WebGPU. The benchmark refuses software fallback adapters rather than presenting their numbers as hardware results. WebGPU requires a supported browser and a secure context, including localhost.

The API and ownership model follow the [WebGPU specification](https://www.w3.org/TR/webgpu/) and [WebGPU explainer](https://gpuweb.github.io/gpuweb/explainer/). Mobile and other desktop GPUs need their own runtime and energy measurements before choosing an automatic policy.
