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

## Measurements

Standard WebGPU in Chromium 156.0.8078.4 on the benchmark Mac, using its browser-selected hardware adapter. Nine batches of 30 two-animation-frame scroll steps per mode. Each mode renders the same Wasm output and source.

| Fixture | DOM CPU tasks per scroll | WebGPU CPU tasks per scroll | DOM / GPU visible nodes |
| --- | ---: | ---: | ---: |
| 1,000 lines, sparse colors | 3.288 ms | 3.540 ms | 220 / 34 |
| 401 lines, dense colors | 3.604 ms | 3.362 ms | 654 / 34 |

WebGPU used about 8% more main-thread task time in the sparse case and about 7% less in the dense case. The sampled batch ranges overlap. Backend readiness, including two animation-frame callbacks, was about 33-35 ms for DOM and 82-84 ms for WebGPU. These first-use observations are not statistically sampled startup or GPU execution measurements.

Both cases recorded zero idle GPU submissions. Actual GPU execution time, watts, CPU energy, memory, and battery life are unmeasured. The current evidence does not justify enabling GPU rendering automatically or claiming an overall speed or battery win against other renderers. The earlier Wasm lexical comparison remains in [the optimization report](performance-optimization.md).

See [sparse results](../benchmarks/results/webgpu.json), [dense results](../benchmarks/results/webgpu-dense.json), and the [benchmark driver](../benchmarks/run-gpu.mjs).

## Validation

`npm --prefix web test` covers text fallback when WebGPU is unavailable, Unicode and diff preservation, source copying, and existing renderer flows. `npm --prefix web run test:gpu` runs a separate hardware test for colored GPU pixels, idle submissions, scrolling, selection, full-source copying, theme changes, offscreen suspension, and cleanup. It skips when the browser cannot provide a hardware adapter. The pixel assertion captures within the draw frame before presentation discards the canvas texture; production rendering does not read pixels back.

Run the hardware benchmark sequentially on an idle machine:

```sh
node benchmarks/run-gpu.mjs
MARKIT_GPU_DENSITY=dense node benchmarks/run-gpu.mjs
```

The hardware suite and benchmark use a normal visible Chromium window by default. Set `MARKIT_GPU_HEADLESS=1` only when that environment exposes standard WebGPU. The benchmark refuses software fallback adapters rather than presenting their numbers as hardware results. WebGPU requires a supported browser and a secure context, including localhost.

The API and ownership model follow the [WebGPU specification](https://www.w3.org/TR/webgpu/) and [WebGPU explainer](https://gpuweb.github.io/gpuweb/explainer/). Mobile and other desktop GPUs need their own runtime and energy measurements before choosing an automatic policy.
