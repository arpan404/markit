# Markit vs Ace web Markdown benchmark

Compares `@markit/web/react` with the Markdown stack in Ace's web chat in Chromium, using production React builds and the same fixtures.

The Ace side is Ace's own code at a pinned commit (`2502a66`), bundled with esbuild:

- `Prose` and `Markdown` from `apps/web/src/components/markdown`.
- The incremental `marked` block stream in the shared `markdown.worker`.
- The cold main-thread path, the regex code highlighter, and KaTeX loaded lazily on the main thread.

`bench/ace/stubs.jsx` replaces only the app components that need Ace's client and router, such as thread and file references, the image resolver, link wrappers and skeletons. Ace's Tailwind stylesheet is not built; only KaTeX's CSS is linked. Ace ships through Vite, so its real chunk sizes differ from this esbuild bundle.

```sh
npm ci
../scripts/build-web.sh          # Markit Wasm into web/pkg
bench/ace/fetch.sh               # Ace checkout into bench/.ace (not committed)
node bench/build.mjs             # both harnesses into bench/dist
node bench/run.mjs --runs 7 --out bench/results/chromium-linux.json
node bench/run.mjs --runs 5 --throttle 4 --out bench/results/chromium-linux-cpu4x.json
```

Each run uses a fresh browser context, so caches are cold. The fixtures in `fixtures.js` are deterministic: prose, lists, TS/Rust fences, diffs, inline and display math, and tables.

| Scenario | What is measured |
| --- | --- |
| `first_message_cold` | Page loaded, then one assistant message mounted. Time until its DOM has been quiet for 300 ms, counted to the last mutation, plus time since navigation. Includes worker start, Wasm or lazy chunks, and KaTeX. |
| `first_message_cold_preload` | The same, after `preloadMarkit()` ran at import. Ace's `Prose` module also starts loading its renderer when imported. |
| `first_message_warm` | A second, different message on the same page. |
| `transcript_50`, `transcript_200` | Mounting 50 or 200 messages (about 33 KB and 135 KB of Markdown). Also records the main-thread JS heap after GC, DOM nodes, worker count, and worker JS heap plus ArrayBuffer backing stores through CDP. Markit's Wasm linear memory is reported separately as `wasmMemory`. |
| `transcript_200_lazy` | Markit with `lazy`: messages more than 1200 px away render later. |
| `stream_long` | One 6.6 KB answer with a 60-line TS fence, a 30-line diff, 8 formulas, a list and a Rust fence. It is streamed in 24-character tokens, one per animation frame, through React state, the way Ace's store feeds `Prose`. Records frame times, long tasks, total blocking time, main-thread task and script time from CDP `Performance.getMetrics`, and the lag from the last token to the final DOM. |
| `transfer_*` | Every file requested until the scenario settled, raw and gzip -9. |

The settle rule makes absolute times include the moment of the final DOM mutation, not a fixed 300 ms wait. `mainTaskMs` covers all main-thread tasks, including style, layout and paint; `mainScriptMs` is JavaScript only.
