# Live2D Character Debugging Log

A full record of every error encountered while integrating `pixi-live2d-display` with Cubism Core 6.x inside a transparent Tauri v2 window, and exactly how each was fixed.

---

## 1. "Attempted to assign to readonly property"

### What happened
Clicking the "meeting" button opened the Live2D window and immediately threw:
```
TypeError: Attempted to assign to readonly property
```

### Why it happened
The XHR patch in `live2d-main.tsx` intercepted `responseType='json'` requests but returned the raw text string as the `response` property:

```ts
// WRONG — returns a string
Object.defineProperty(self, 'response', { value: text, ... });
```

`pixi-live2d-display` then did `data.url = "..."` on the parsed model JSON. In V8 (Chrome/Node), assigning a property to a primitive string silently fails. In WebKit (used by Tauri on macOS/Linux), it throws `"Attempted to assign to readonly property"`.

### Fix — `src/live2d-main.tsx`
Parse the JSON and return the **object**, not the string:

```ts
} else if (this._responseType2 === 'json') {
  const text = new TextDecoder().decode(bytes);
  let parsed: unknown = null;
  try { parsed = JSON.parse(text); } catch { parsed = null; }
  Object.defineProperty(self, 'response', { value: parsed, configurable: true });
}
```

---

## 2. Wrong texture / moc3 URLs (404s)

### What happened
Textures and `.moc3` were requested as:
```
asset://localhost/八千代辉夜姬.moc3   ← wrong, no path prefix
```
instead of:
```
asset://localhost/home/uty/Downloads/.../八千代辉夜姬.moc3
```

### Why it happened
`convertFileSrc()` (Tauri's helper) encodes **the entire path** with `encodeURIComponent`, turning every `/` into `%2F`. The whole absolute path becomes a single URL segment. When `pixi-live2d-display` resolves relative URLs (e.g. `八千代辉夜姬.moc3`) against the base URL, it strips everything after the last `/` — but since there are no literal `/` characters, it strips the entire path.

### Fix — `src/components/live2d/Live2DCanvas.tsx`
Build the `asset://` URL manually, encoding **only within each segment**:

```ts
const resolvedUrl =
  'asset://localhost' +
  modelUrl.split('/').map(encodeURIComponent).join('/');
```

This preserves `/` separators while encoding special characters (spaces, CJK, etc.) inside directory names.

---

## 3. Textures 403 / 404 via the asset protocol

### What happened
Even with the correct URL, PNG textures returned 403 or 404 from Tauri's asset protocol.

### Why it happened
Tauri's `assetProtocol.scope` uses glob patterns relative to the app bundle directory, not the filesystem root. Setting `["/**"]` or `["$HOME/**"]` was rejected (403). Setting `["**"]` matched but looked up paths relative to the bundle, not `/home/...`.

### Fix — `src/live2d-main.tsx`
Bypass the asset protocol entirely for textures. Patch `HTMLImageElement.prototype.src` to intercept any `asset://localhost/` URL and load the file via Tauri IPC (`read_file_as_base64`), then set the image source to a `data:` URI:

```ts
Object.defineProperty(HTMLImageElement.prototype, 'src', {
  set(url: string) {
    if (url.startsWith('asset://localhost/')) {
      const path = '/' + decodeURIComponent(url.slice('asset://localhost/'.length));
      invoke<string>('read_file_as_base64', { path })
        .then((b64) => { nativeSet.call(this, `data:image/png;base64,${b64}`); })
        .catch(() => { nativeSet.call(this, url); });
    } else {
      nativeSet.call(this, url);
    }
  }
});
```

---

## 4. moc3 version not supported

### What happened
Console showed:
```
[CSM] moc3 version is not supported
```
The model failed to load.

### Why it happened
The bundled `live2dcubismcore.min.js` was version `04.02.0002`, which only supports moc3 files up to format version 4. The user's model used format version 5.

### Fix
Replace `public/live2dcubismcore.min.js` with Cubism Core version `06.00.0001`, which supports moc3 formats 4, 5, and 6.

---

## 5. `renderOrder[i]` is undefined — Cubism Core 6.x API change

### What happened
After loading the model:
```
TypeError: undefined is not an object (evaluating 'renderOrder[i]')
  at doDrawModel — cubism4.es.js:3520
```

### Why it happened
`pixi-live2d-display` v0.4.0 was written against Cubism Core 4.x. Its `getDrawableRenderOrders()` reads:

```js
// cubism4.es.js (original)
getDrawableRenderOrders() {
  return this._model.drawables.renderOrders;  // ← undefined in Core 6.x
}
```

Cubism Core 6.x **moved** `renderOrders` from `model.drawables.renderOrders` to `model.renderOrders` directly:

```js
// Core 6.x constructor (minified)
this.drawables    = new Drawables(this._ptr);
this.renderOrders = new Int32Array(...);   // ← on Model, not Drawables
```

### Fix — patch the built files
Edit both the source and the Vite pre-bundle cache to add a fallback:

```js
// node_modules/pixi-live2d-display/dist/cubism4.es.js
// node_modules/pixi-live2d-display/dist/index.es.js
// node_modules/.vite/deps/pixi-live2d-display_cubism4.js
// node_modules/.vite/deps/pixi-live2d-display.js
getDrawableRenderOrders() {
  return this._model.renderOrders ?? this._model.drawables.renderOrders;
}
```

Also add an esbuild plugin in `vite.config.ts` so the patch survives cache rebuilds:

```ts
esbuildOptions: {
  plugins: [{
    name: 'cubism6-compat',
    setup(build) {
      build.onLoad({ filter: /pixi-live2d-display/ }, async (args) => {
        const fs = await import('fs/promises');
        let contents = await fs.readFile(args.path, 'utf8');
        contents = contents.replace(
          /this\._model\.drawables\.renderOrders/g,
          '(this._model.renderOrders ?? this._model.drawables.renderOrders)',
        );
        return { contents, loader: 'js' };
      });
    },
  }],
},
```

---

## 6. Character only shows legs — wrong model anchor

### What happened
The character rendered with anchor at the bottom-left. Only the feet were visible.

### Why it happened
The original code set `model.anchor.set(0.5, 1.0)` (bottom-centre) and positioned the model at the bottom edge of the canvas. With `MODEL_SCALE = 0.25`, most of the character was off-screen.

### Fix — `src/components/live2d/Live2DCanvas.tsx`
Implement a `fitModel()` function that measures the model's natural size at scale 1, then computes the largest scale that fits inside the window with a small gap:

```ts
function fitModel(model, w, h) {
  model.scale.set(1);
  const naturalW = model.width;
  const naturalH = model.height;
  const scale = Math.min((w - GAP*2) / naturalW, (h - GAP*2) / naturalH, 1);
  model.scale.set(scale);
  model.anchor.set(0.5, 0.5);  // centre anchor
  model.x = w / 2;
  model.y = h / 2;
}
```

Call `fitModel()` once after loading and again inside a `ResizeObserver` callback.

---

## 7. Resize drag only changes width OR height, character doesn't scale

### What happened
Dragging the window border changed only one dimension. The character did not scale to fill the new size.

### Why it happened
Two bugs:
1. PIXI's `autoDensity: true` sets `canvas.style.width` and `canvas.style.height` as inline styles after every `renderer.resize()` call, overriding the CSS `width: 100%; height: 100%`. Once overridden, the CSS layout no longer drives the canvas size, so subsequent `ResizeObserver` entries reported zero delta.
2. The `ResizeObserver` was watching the `<canvas>` element directly, so the above override prevented it from firing again.

### Fix
1. Watch the **wrapper div**, not the canvas element, with `ResizeObserver`.
2. After every `renderer.resize()`, explicitly reset the canvas inline styles:

```ts
observer = new ResizeObserver((entries) => {
  const { width: nw, height: nh } = entries[0].contentRect;
  renderer.resize(nw, nh);
  canvasRef.current.style.width  = '100%';  // ← reset PIXI's override
  canvasRef.current.style.height = '100%';
  fitModel(model, renderer.screen.width, renderer.screen.height);
});
observer.observe(wrapperRef.current);  // ← watch wrapper, not canvas
```

---

## 8. Ghost trail / shadow when character moves

### What happened
When the character's arms or hair moved, the previous positions left a faint shadow that took several seconds to fade.

### Why it happened
WebKit (used by Tauri on Linux/macOS) does not automatically clear the WebGL compositing layer between frames when the window is transparent. PIXI's `clearBeforeRender: true` (default) clears the WebGL framebuffer, but WebKit's compositor caches the previous GPU layer and blends it with the new frame, producing smearing.

**Failed attempt**: calling `gl.bindFramebuffer(gl.FRAMEBUFFER, null)` before clearing corrupted PIXI's internal framebuffer state cache. PIXI tracks the currently-bound FBO to avoid redundant `bindFramebuffer` calls; forcing it to `null` made PIXI think no FBO was bound, then the next render wrote to the wrong target, causing a solid black background.

### Fix — `src/components/live2d/Live2DCanvas.tsx`
Add a PIXI ticker callback at **priority 25** (higher than the default render priority of 0) that clears the color buffer **without rebinding the framebuffer**. At priority 25, PIXI has not yet bound any off-screen FBO, so the current binding is already the default (null) framebuffer = the canvas:

```ts
const gl = renderer.gl as WebGLRenderingContext;
app.ticker.add(() => {
  // Do NOT call gl.bindFramebuffer here — that would corrupt PIXI's FBO cache
  gl.clearColor(0, 0, 0, 0);
  gl.clear(gl.COLOR_BUFFER_BIT);
}, null, 25);  // 25 = before PIXI's normal render at priority 0
```

Also set `background: 'transparent'` on the canvas element itself so the CSS background doesn't show through during the initial load before WebGL takes over.

---

## 9. Scroll-to-resize not working

### What happened
Scrolling in edit mode did nothing.

### Why it happened (two bugs)

**Bug A — passive listener**: React's `onWheel` synthetic event is registered as a passive listener. Passive listeners cannot call `e.preventDefault()`. Without `preventDefault()`, the browser's own scroll handling fired, and the Tauri window intercepted the scroll before any resize logic ran.

**Bug B — wrong pixel units**: `win.innerSize()` returns **physical pixels** (multiplied by `devicePixelRatio`). `LogicalSize` expects **logical (CSS) pixels**. On a 2× retina display, the calculation doubled the target size on every scroll tick.

Additionally, `win.innerSize()` is async — a rapid scroll would fire multiple wheel events before the first `await` resolved, causing them to all read the same stale size.

### Fix
1. Use `document.addEventListener('wheel', handler, { passive: false, capture: true })` — capture phase fires before any element-level listener, and non-passive allows `preventDefault()`.
2. Read `window.innerWidth` / `window.innerHeight` (synchronous, already logical pixels) instead of `win.innerSize()`:

```ts
const onWheel = (e: WheelEvent) => {
  if (!editModeRef.current) return;
  e.preventDefault();
  const factor = e.deltaY < 0 ? 1.06 : 0.94;
  // window.innerWidth/Height are CSS logical pixels — exactly what LogicalSize needs
  const newW = Math.max(100, Math.round(window.innerWidth  * factor));
  const newH = Math.max(100, Math.round(window.innerHeight * factor));
  win.setSize(new LogicalSize(newW, newH));
};
document.addEventListener('wheel', onWheel, { passive: false, capture: true });
```

---

## 10. Close button stuck visible / edit mode not toggling after drag

### What happened
After dragging the window once, clicking the character no longer toggled edit mode. The close button stayed visible permanently.

### Why it happened
After `win.startDragging()`, Tauri's native OS drag takes over the pointer. The webview **never receives the `mouseup` event**. The cleanup listener `window.addEventListener('mouseup', onUp)` therefore never fired, and `isDraggingRef.current` stayed `true` forever.

The `handleClick` guard was:
```ts
function handleClick() {
  if (isDraggingRef.current) {
    isDraggingRef.current = false;  // reset once...
    return;                          // ...but still don't toggle
  }
  setEditMode(prev => !prev);
}
```

The first click after a drag consumed the reset but didn't toggle. Every click after that toggled correctly — but users noticed the first click was swallowed.

### Fix — `src/components/live2d/Live2DWindowApp.tsx`
Reset `isDraggingRef` immediately after `startDragging()` with a short `setTimeout`. 300 ms is long enough for the browser to have already decided no `click` event follows the drag, but short enough that a subsequent intentional click works:

```ts
win.startDragging();
setTimeout(() => { isDraggingRef.current = false; }, 300);
```

Also simplify `handleClick` — no need to reset the flag there:

```ts
function handleClick() {
  if (isDraggingRef.current) return;  // drag in progress, ignore
  setEditMode(prev => !prev);
}
```
