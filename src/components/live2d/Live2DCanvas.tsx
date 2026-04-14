import { useEffect, useRef, useState } from 'react';

const GAP = 5;

export interface ModelBounds {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface NaturalSize {
  width: number;
  height: number;
}

export interface Live2DControls {
  scaleBy(factor: number): void;
  setScale(scale: number): void;
  getScale(): number;
  getBounds(): ModelBounds | null;
  getNaturalSize(): NaturalSize | null;
}

interface Props {
  modelUrl?: string;
  controlsRef?: React.RefObject<Live2DControls | null>;
  onBoundsChange?: (bounds: ModelBounds) => void;
  onReady?: () => void;
  /** Called each frame with raw RGBA pixels (top-down). Linux native overlay path. */
  onFrame?: (pixels: Uint8Array, width: number, height: number) => void;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function computeBounds(model: any): ModelBounds {
  const hw = model.width  / 2;
  const hh = model.height / 2;
  return {
    left:   model.x - hw,
    right:  model.x + hw,
    top:    model.y - hh,
    bottom: model.y + hh,
  };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function initialFit(model: any, w: number, h: number) {
  model.scale.set(1);
  const nw: number = model.width;
  const nh: number = model.height;
  if (!nw || !nh) return;
  const scale = Math.min((w - GAP * 2) / nw, (h - GAP * 2) / nh, 1);
  model.scale.set(scale);
  model.anchor.set(0.5, 0.5);
  model.x = w / 2;
  model.y = h / 2;
}

export function Live2DCanvas({ modelUrl, controlsRef, onBoundsChange, onReady, onFrame }: Props) {
  const wrapperRef = useRef<HTMLDivElement>(null);
  // This is the VISIBLE 2D canvas shown in the DOM.
  // PIXI renders to an off-DOM WebGL canvas, and we blit the result here
  // every frame via drawImage. This completely bypasses WebKit2GTK's
  // WebGL texture caching that causes ghost trails on transparent windows.
  const displayRef  = useRef<HTMLCanvasElement>(null);
  const [error, setError]     = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!modelUrl) return;
    setError(null);
    setLoading(true);
    if (!displayRef.current || !wrapperRef.current) return;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (controlsRef) (controlsRef as any).current = null;

    let cancelled = false;
    let observer: ResizeObserver | null = null;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let app: any = null;

    const resolvedUrl = modelUrl.startsWith('live2d://')
      ? modelUrl
      : 'asset://localhost' + modelUrl.split('/').map(encodeURIComponent).join('/');

    (async () => {
      if (cancelled || !displayRef.current || !wrapperRef.current) return;

      const [{ Application }, { Ticker }, { Live2DModel }] = await Promise.all([
        import('@pixi/app'),
        import('@pixi/ticker'),
        import('pixi-live2d-display/cubism4'),
      ]);

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      Live2DModel.registerTicker(Ticker as any);

      if (cancelled || !displayRef.current || !wrapperRef.current) return;

      const rect = wrapperRef.current.getBoundingClientRect();
      const w = rect.width  || wrapperRef.current.offsetWidth  || 400;
      const h = rect.height || wrapperRef.current.offsetHeight || 600;
      const dpr = window.devicePixelRatio || 1;

      // Off-DOM WebGL canvas — PIXI renders here.  WebKit never sees this
      // canvas in the DOM, so its WebGL texture-caching/compositor can't
      // accumulate stale frames (the root cause of ghost trails).
      const glCanvas = document.createElement('canvas');

      app = new Application({
        view: glCanvas,
        width: w,
        height: h,
        backgroundAlpha: 0,
        antialias: true,
        // Linux native-overlay path: render at logical (1×) resolution so the
        // frame sent via IPC is ~4× smaller on HiDPI displays.  GTK already
        // scales the surface to fit the overlay window, so visual quality is
        // unchanged.  Non-Linux keeps DPR so the 2D display canvas is sharp.
        resolution: onFrame ? 1 : dpr,
        autoDensity: true,
        forceCanvas: false,
        // preserveDrawingBuffer must be true so drawImage can read the
        // framebuffer after PIXI renders (otherwise the browser may clear
        // the buffer between the render and our read).
        preserveDrawingBuffer: true,
      });

      const model = await Live2DModel.from(resolvedUrl);
      if (cancelled) { model.destroy(); return; }

      // Cache natural size at scale=1 before any fitting.
      model.scale.set(1);
      const naturalW: number = model.width;
      const naturalH: number = model.height;

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const renderer = (app as any).renderer as any;

      initialFit(model, renderer.screen.width, renderer.screen.height);
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      app.stage.addChild(model as any);

      // ── Dual-canvas blit: WebGL → 2D ──────────────────────────────
      // Set up the visible 2D display canvas to match the GL canvas size.
      const displayCanvas = displayRef.current!;
      displayCanvas.width  = Math.round(w * dpr);
      displayCanvas.height = Math.round(h * dpr);
      const displayCtx = displayCanvas.getContext('2d')!;

      // WebGL2 context for the native GTK overlay path (Linux PBO readback).
      // MUST be acquired AFTER new Application() to reuse PIXI's context.
      // WebGL images are bottom-up; Y-flip is handled in Rust to avoid the
      // extra JS allocation + copy on every frame.
      const gl2 = onFrame
        ? (glCanvas.getContext('webgl2') as WebGL2RenderingContext | null)
        : null;
      if (onFrame && !gl2) console.error('[Live2DCanvas] WebGL2 required for Linux native overlay PBO');

      // Double-buffered PBOs: write to one this tick, read back from the other
      // (written last tick, GPU has had ≥1 frame to finish the DMA transfer).
      // This eliminates the CPU-GPU sync stall of synchronous readPixels.
      const pbos: (WebGLBuffer | null)[] = gl2
        ? [gl2.createBuffer(), gl2.createBuffer()]
        : [null, null];
      // Metadata for each PBO slot.
      const pboMeta = [
        { pw: 0, ph: 0, ready: false },
        { pw: 0, ph: 0, ready: false },
      ];
      let pboWrite = 0; // index of PBO to write to this tick
      // One frame in-flight at a time — prevents flooding the WebKit IPC
      // connection pool (which would block other Tauri API calls).
      let frameSending = false;

      // After each PIXI render (priority -50, PIXI renders at 0):
      //   • Linux native overlay: PBO kick happens EVERY tick so PIXI runs at
      //     full rAF rate; only the IPC send is gated by frameSending.
      //     Frames that arrive while busy are discarded (latest-frame-wins in
      //     Rust means only the newest painted frame matters anyway).
      //   • Non-Linux: blit off-DOM WebGL canvas to visible 2D canvas.
      app.ticker.add(() => {
        if (cancelled || !displayRef.current) return;

        if (onFrame && gl2) {
          const pw = glCanvas.width;
          const ph = glCanvas.height;

          // ── Retrieve pixels written last tick ──────────────────────────
          const pboRead = 1 - pboWrite;
          const meta = pboMeta[pboRead];
          if (meta.ready && meta.pw > 0 && meta.ph > 0) {
            if (!frameSending) {
              const buf = new Uint8Array(meta.pw * meta.ph * 4);
              gl2.bindBuffer(gl2.PIXEL_PACK_BUFFER, pbos[pboRead]);
              gl2.getBufferSubData(gl2.PIXEL_PACK_BUFFER, 0, buf);
              gl2.bindBuffer(gl2.PIXEL_PACK_BUFFER, null);
              frameSending = true;
              Promise.resolve(onFrame(buf, meta.pw, meta.ph))
                .finally(() => { frameSending = false; });
            }
            // Always clear the slot so stale data isn't re-read on next tick.
            meta.ready = false;
          }

          // ── Kick new async readback into pboWrite (always, every tick) ──
          gl2.bindBuffer(gl2.PIXEL_PACK_BUFFER, pbos[pboWrite]);
          gl2.bufferData(gl2.PIXEL_PACK_BUFFER, pw * ph * 4, gl2.STREAM_READ);
          gl2.readPixels(0, 0, pw, ph, gl2.RGBA, gl2.UNSIGNED_BYTE, 0);
          gl2.bindBuffer(gl2.PIXEL_PACK_BUFFER, null);
          pboMeta[pboWrite] = { pw, ph, ready: true };

          pboWrite = pboRead;
        } else {
          displayCtx.clearRect(0, 0, displayCanvas.width, displayCanvas.height);
          displayCtx.drawImage(glCanvas, 0, 0);
        }
      }, null, -50);

      if (controlsRef) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (controlsRef as any).current = {
          scaleBy(factor: number) {
            const next = Math.max(0.05, Math.min(10, model.scale.x * factor));
            model.scale.set(next);
            model.x = renderer.screen.width  / 2;
            model.y = renderer.screen.height / 2;
            onBoundsChange?.(computeBounds(model));
          },
          setScale(scale: number) {
            const next = Math.max(0.05, Math.min(10, scale));
            model.scale.set(next);
            model.x = renderer.screen.width  / 2;
            model.y = renderer.screen.height / 2;
            onBoundsChange?.(computeBounds(model));
          },
          getScale() { return model.scale.x; },
          getBounds() { return computeBounds(model); },
          getNaturalSize() {
            return { width: naturalW, height: naturalH };
          },
        } satisfies Live2DControls;
      }

      onBoundsChange?.(computeBounds(model));
      setLoading(false);
      onReady?.();

      observer = new ResizeObserver((entries) => {
        if (cancelled || !displayRef.current) return;
        const { width: nw, height: nh } = entries[0].contentRect;
        if (!nw || !nh) return;
        renderer.resize(nw, nh);
        // Resize visible display canvas to match.
        const ndpr = window.devicePixelRatio || 1;
        displayRef.current.width  = Math.round(nw * ndpr);
        displayRef.current.height = Math.round(nh * ndpr);
        model.x = renderer.screen.width  / 2;
        model.y = renderer.screen.height / 2;
        onBoundsChange?.(computeBounds(model));
      });
      observer.observe(wrapperRef.current);
    })().catch((err) => {
      if (!cancelled) {
        setLoading(false);
        const msg = err instanceof Error ? (err.stack ?? String(err)) : String(err);
        console.error('[Live2DCanvas]', err);
        setError(msg);
        onReady?.();
      }
    });

    return () => {
      cancelled = true;
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      if (controlsRef) (controlsRef as any).current = null;
      observer?.disconnect();
      app?.destroy(false);
    };
  }, [modelUrl]); // eslint-disable-line react-hooks/exhaustive-deps

  if (error) {
    return (
      <div style={{
        width: '100%', height: '100%', display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center', padding: 16,
        color: '#f87171', fontSize: 11, textAlign: 'center', gap: 8,
        background: 'rgba(0,0,0,0.6)', borderRadius: 10,
      }}>
        <div style={{ fontWeight: 600 }}>Failed to load model</div>
        <div style={{ opacity: 0.7, wordBreak: 'break-all' }}>{error}</div>
      </div>
    );
  }

  if (!modelUrl) {
    return (
      <div style={{
        width: '100%', height: '100%', display: 'flex', flexDirection: 'column',
        alignItems: 'center', justifyContent: 'center',
        color: 'rgba(255,255,255,0.5)', fontSize: 13,
        background: 'rgba(0,0,0,0.4)', borderRadius: 10,
      }}>
        No model selected
      </div>
    );
  }

  return (
    <div ref={wrapperRef}
      style={{ position: 'relative', width: '100%', height: '100%', isolation: 'isolate' }}
    >
      <canvas
        ref={displayRef}
        style={{
          width: '100%', height: '100%', display: 'block', background: 'transparent',
          // When onFrame is provided the character is shown in the GTK native overlay.
          // Hide this canvas so the WebviewWindow doesn't also show the character
          // (which would still accumulate ghost trails on Linux).
          visibility: onFrame ? 'hidden' : 'visible',
        }}
      />
      {loading && (
        <div style={{
          position: 'absolute', inset: 0, display: 'flex',
          alignItems: 'center', justifyContent: 'center',
          color: 'rgba(255,255,255,0.7)', fontSize: 12,
        }}>
          Loading model…
        </div>
      )}
    </div>
  );
}
