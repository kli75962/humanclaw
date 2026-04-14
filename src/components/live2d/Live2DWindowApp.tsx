import { useCallback, useEffect, useRef, useState } from 'react';
import { getCurrentWindow, currentMonitor } from '@tauri-apps/api/window';
import { LogicalSize, LogicalPosition } from '@tauri-apps/api/dpi';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Live2DCanvas, type Live2DControls } from './Live2DCanvas';

// On Linux the character is rendered in a native GTK overlay window (cairo
// Operator::Source = no ghost trails).  Buttons and drag are handled by the
// GTK overlay directly; the WebviewWindow is only the PIXI/JS runtime.
const IS_LINUX_DESKTOP = (() => {
  try {
    return (
      typeof navigator !== 'undefined' &&
      navigator.userAgent.toLowerCase().includes('linux') &&
      !navigator.userAgent.toLowerCase().includes('android')
    );
  } catch { return false; }
})();


function getUrlModel(): string | undefined {
  const param = new URLSearchParams(window.location.search).get('model');
  return param ? decodeURIComponent(param) : undefined;
}

const BTN_SIZE  = 28;
const BTN_RIGHT = 8;
const PAD = 8;
const INIT_HEIGHT_FRAC = 0.6;
const HOVER_TIMEOUT_MS = 1500;

const rAF = () => new Promise<void>(r => requestAnimationFrame(() => r()));

export function Live2DWindowApp() {
  const win = getCurrentWindow();
  const modelUrl = getUrlModel();

  const controlsRef  = useRef<Live2DControls | null>(null);
  // Linux: GTK overlay position/size in logical pixels, updated by live2d-moved events.
  const overlayPosRef = useRef({ x: 0, y: 0, w: 400, h: 600 });
  // Linux: desired logical scale — kept separately because ctrls.getScale() returns the
  // PIXI display scale which may differ after a resize, so this gives a stable baseline.
  const virtualScaleRef = useRef(1.0);
  // Linux: screen logical size — remembered so handleScale and live2d-resized
  // can clamp the WebviewWindow.  X11 clips windows that exceed screen bounds,
  // making the PIXI canvas smaller than the GTK overlay → Cairo upscales →
  // the character stretches.  The GTK overlay itself uses override_redirect and
  // can be placed/sized freely, but both must agree on the same clamped size.
  const screenSizeRef = useRef({ w: 1280, h: 800 });
  // Linux: natural model aspect ratio (width/height), sent to Rust so resize uses exact ratio.
  const natAspectRef = useRef(0.0);
  // Stable ref to handleScale so event-listener closures always call latest version.
  const handleScaleRef = useRef<(f: number) => void>(() => {});
  // Prevent concurrent scale operations from piling up (each has awaits).
  const scalingRef = useRef(false);

  // Non-Linux hover state for HTML buttons.
  const [hovered, setHovered]   = useState(false);
  const hoverTimerRef = useRef<number | null>(null);

  // ── Hover tracking (non-Linux only) ────────────────────────────────────
  useEffect(() => {
    if (IS_LINUX_DESKTOP) return;
    const bumpHover = () => {
      setHovered(true);
      if (hoverTimerRef.current !== null) window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = window.setTimeout(() => {
        setHovered(false);
        hoverTimerRef.current = null;
      }, HOVER_TIMEOUT_MS);
    };
    const hideNow = () => {
      setHovered(false);
      if (hoverTimerRef.current !== null) {
        window.clearTimeout(hoverTimerRef.current);
        hoverTimerRef.current = null;
      }
    };
    document.addEventListener('mousemove', bumpHover);
    document.addEventListener('mouseleave', hideNow);
    const onMouseOut = (e: MouseEvent) => {
      if (!e.relatedTarget) hideNow();
    };
    document.addEventListener('mouseout', onMouseOut);
    return () => {
      document.removeEventListener('mousemove', bumpHover);
      document.removeEventListener('mouseleave', hideNow);
      document.removeEventListener('mouseout', onMouseOut);
      if (hoverTimerRef.current !== null) window.clearTimeout(hoverTimerRef.current);
    };
  }, []);

  // ── Drag (non-Linux only — GTK overlay handles drag on Linux) ──────────
  useEffect(() => {
    if (IS_LINUX_DESKTOP) return;
    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      if ((e.target as HTMLElement).closest('[data-live2d-btn]')) return;
      e.preventDefault();
      win.startDragging();
    };
    document.addEventListener('mousedown', onMouseDown, { capture: true });
    return () => document.removeEventListener('mousedown', onMouseDown, true);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Window size calculation ─────────────────────────────────────────────
  const computeWindowSize = useCallback((targetScale: number) => {
    const nat = controlsRef.current?.getNaturalSize();
    if (!nat) return null;
    return {
      w: Math.max(80,  Math.ceil(nat.width  * targetScale + PAD * 2)),
      h: Math.max(100, Math.ceil(nat.height * targetScale + PAD * 2)),
    };
  }, []);

  // ── Scale handler ───────────────────────────────────────────────────────
  const handleScale = useCallback(async (factor: number) => {
    // Drop concurrent calls — each has multiple awaits and they'd pile up.
    if (scalingRef.current) return;
    scalingRef.current = true;
    try {
      const ctrls = controlsRef.current;
      if (!ctrls) return;

      if (IS_LINUX_DESKTOP) {
        // Linux: resize WebviewWindow AND GTK overlay to the same size.
        // Clamp to screen so X11 never clips the WebviewWindow (clipping makes
        // the PIXI canvas smaller than the overlay → Cairo stretches the frame).
        const cur = virtualScaleRef.current;
        const next = Math.max(0.05, Math.min(10, cur * factor));
        if (next === cur) return;

        const desired = computeWindowSize(next);
        if (!desired) return;
        const { w: screenW, h: screenH } = screenSizeRef.current;
        const newW = Math.min(desired.w, screenW);
        const newH = Math.min(desired.h, screenH);

        // Effective PIXI scale for the (clamped) canvas — preserves aspect.
        const nat = controlsRef.current?.getNaturalSize();
        if (!nat) return;
        const pixiScale = Math.min(next, (newW - PAD * 2) / nat.width, (newH - PAD * 2) / nat.height);

        const { x: oldX, y: oldY, w: oldW, h: oldH } = overlayPosRef.current;
        const newX = Math.round(oldX + oldW - newW);
        const newY = Math.max(0, Math.round(oldY + oldH / 2 - newH / 2));
        // Track the actual achieved scale so shrinking after hitting the cap works.
        virtualScaleRef.current = pixiScale;
        overlayPosRef.current = { x: newX, y: newY, w: newW, h: newH };

        if (factor >= 1) {
          await win.setSize(new LogicalSize(newW, newH));
          await rAF(); await rAF();
          ctrls.setScale(pixiScale);
        } else {
          ctrls.setScale(pixiScale);
          await win.setSize(new LogicalSize(newW, newH));
        }
        await invoke('show_live2d_overlay', { x: newX, y: newY, width: newW, height: newH, natAspect: natAspectRef.current });
        return;
      }

      // Non-Linux: resize the actual window and update PIXI scale.
      const nextScale = Math.max(0.05, Math.min(10, ctrls.getScale() * factor));
      if (nextScale === ctrls.getScale()) return;
      const newSize = computeWindowSize(nextScale);
      if (!newSize) return;
      const [physPos, physSize] = await Promise.all([
        win.outerPosition(), win.outerSize(),
      ]);
      const dpr = window.devicePixelRatio || 1;
      const oldX = physPos.x / dpr;  const oldY = physPos.y / dpr;
      const oldW = physSize.width / dpr;  const oldH = physSize.height / dpr;
      const newX = Math.round(oldX + oldW - newSize.w);
      const newY = Math.max(0, Math.round(oldY + oldH / 2 - newSize.h / 2));
      if (factor >= 1) {
        await Promise.all([
          win.setSize(new LogicalSize(newSize.w, newSize.h)),
          win.setPosition(new LogicalPosition(newX, newY)),
        ]);
        await rAF();
        ctrls.setScale(nextScale);
      } else {
        ctrls.setScale(nextScale);
        await Promise.all([
          win.setSize(new LogicalSize(newSize.w, newSize.h)),
          win.setPosition(new LogicalPosition(newX, newY)),
        ]);
      }
    } finally {
      scalingRef.current = false;
    }
  }, [computeWindowSize]); // eslint-disable-line react-hooks/exhaustive-deps

  // Keep the scale ref current so GTK event listeners always call latest version.
  handleScaleRef.current = handleScale;

  // ── Close handler ────────────────────────────────────────────────────────
  const handleClose = useCallback(() => {
    if (IS_LINUX_DESKTOP) invoke('hide_live2d_overlay').catch(() => {});
    win.close();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Mount effect ─────────────────────────────────────────────────────────
  useEffect(() => {
    if (!IS_LINUX_DESKTOP) {
      win.show().catch(() => {});
      return;
    }

    // Linux: show WebviewWindow (hidden PIXI runtime) then show GTK overlay.
    // The overlay handles all user interaction; the WebviewWindow is invisible
    // (transparent window, canvas visibility:hidden) but must stay on-screen so
    // X11 keeps the full render area alive — a window moved off-screen can have
    // its effective size clamped by the compositor, breaking canvas dimensions.
    // setIgnoreCursorEvents makes the WebviewWindow fully pass-through for mouse
    // input so it never blocks clicks to other apps or the GTK overlay.
    win.show().catch(() => {});
    win.setIgnoreCursorEvents(true).catch(() => {});

    (async () => {
      const [pos, sz] = await Promise.all([win.outerPosition(), win.outerSize()]);
      const dpr = window.devicePixelRatio || 1;
      const w = Math.round(sz.width  / dpr);
      const h = Math.round(sz.height / dpr);
      const x = Math.round(pos.x / dpr);
      const y = Math.round(pos.y / dpr);
      overlayPosRef.current = { x, y, w, h };
      await invoke('show_live2d_overlay', { x, y, width: w, height: h, natAspect: 0 });
    })().catch(() => {});

    // GTK overlay events.
    // Close and resize are handled natively in Rust (GTK window).
    const unlistens: (() => void)[] = [];

    // Keep overlayPosRef in sync with GTK window position/size.
    listen<{ x: number; y: number; w: number; h: number }>('live2d-moved', (e) => {
      overlayPosRef.current = e.payload;
    }).then(fn => unlistens.push(fn));

    // User finished a corner-drag resize: sync the WebviewWindow canvas.
    listen<{ x: number; y: number; w: number; h: number }>('live2d-resized', async (e) => {
      const { x, y, w, h } = e.payload;
      const nat = controlsRef.current?.getNaturalSize();
      if (!nat) return;
      const { w: screenW, h: screenH } = screenSizeRef.current;
      const aspect = nat.width / nat.height;

      // Clamp to screen so the WebviewWindow doesn't get X11-clipped (which
      // would make the PIXI canvas smaller than the GTK overlay → stretch).
      let clampW = w, clampH = h;
      if (clampH > screenH) { clampH = screenH; clampW = Math.round(clampH * aspect); }
      if (clampW > screenW) { clampW = screenW; clampH = Math.round(clampW / aspect); }
      const adjX = x + w - clampW;
      const adjY = Math.max(0, Math.round(y + h / 2 - clampH / 2));

      const pixiScale = Math.min(
        (clampW - PAD * 2) / nat.width,
        (clampH - PAD * 2) / nat.height,
      );
      virtualScaleRef.current = pixiScale;
      overlayPosRef.current = { x: adjX, y: adjY, w: clampW, h: clampH };

      await win.setSize(new LogicalSize(clampW, clampH));
      await win.setPosition(new LogicalPosition(adjX, adjY));
      await rAF(); await rAF();
      controlsRef.current?.setScale(pixiScale);
      // Sync GTK overlay to the clamped size.
      await invoke('show_live2d_overlay', {
        x: adjX, y: adjY, width: clampW, height: clampH,
        natAspect: natAspectRef.current,
      });
    }).then(fn => unlistens.push(fn));

    return () => {
      unlistens.forEach(fn => fn());
      invoke('hide_live2d_overlay').catch(() => {});
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // ── After model loads ────────────────────────────────────────────────────
  const handleReady = useCallback(async () => {
    try {
      const ctrls = controlsRef.current;
      if (!ctrls) return;
      const nat = ctrls.getNaturalSize();
      if (!nat?.width || !nat?.height) return;

      let monH = 800, monW = 1280;
      try {
        const mon = await currentMonitor();
        const dpr = window.devicePixelRatio || 1;
        if (mon) { monH = mon.size.height / dpr; monW = mon.size.width / dpr; }
      } catch { /* use fallback */ }

      const size = computeWindowSize((monH * INIT_HEIGHT_FRAC) / nat.height);
      if (!size) return;
      const newX = Math.round((monW - size.w) / 2);
      const newY = Math.round((monH - size.h) / 2);

      const initScale = (monH * INIT_HEIGHT_FRAC) / nat.height;
      await win.setSize(new LogicalSize(size.w, size.h));
      await win.setPosition(new LogicalPosition(newX, newY));
      await rAF(); await rAF();
      screenSizeRef.current = { w: monW, h: monH };
      ctrls.setScale(initScale);
      await rAF();

      if (IS_LINUX_DESKTOP) {
        // Seed virtualScaleRef so handleScale has the correct baseline.
        virtualScaleRef.current = initScale;
        natAspectRef.current = nat.width / nat.height;
        overlayPosRef.current = { x: newX, y: newY, w: size.w, h: size.h };
        await invoke('show_live2d_overlay', { x: newX, y: newY, width: size.w, height: size.h, natAspect: natAspectRef.current });
      } else {
        await win.show();
      }
    } catch (err) {
      console.error('[Live2DWindowApp] handleReady error:', err);
      if (!IS_LINUX_DESKTOP) win.show().catch(() => {});
    }
  }, [computeWindowSize]); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Frame sender (Linux only) ─────────────────────────────────────────
  // Live2DCanvas pre-packs [width u32 LE][height u32 LE][RGBA pixels…] into
  // a single reused buffer and calls this with it directly — no extra copy.
  // Tauri sends it as application/octet-stream (InvokeBody::Raw on Rust side).
  const handleFrame = useCallback(
    (packed: Uint8Array) => invoke('send_live2d_frame', packed),
    [],
  );

  // ── Button style (non-Linux HTML buttons) ─────────────────────────────
  const btnStyle = (bg: string): React.CSSProperties => ({
    width: BTN_SIZE, height: BTN_SIZE,
    borderRadius: '50%', border: 'none', cursor: 'pointer',
    background: bg, color: '#fff', fontSize: 16, fontWeight: 700,
    display: 'flex', alignItems: 'center', justifyContent: 'center',
    flexShrink: 0, boxShadow: '0 2px 6px rgba(0,0,0,0.45)',
    userSelect: 'none', WebkitUserSelect: 'none',
  });

  return (
    <div style={{ position: 'fixed', inset: 0, overflow: 'hidden', background: 'transparent' }}>
      {modelUrl ? (
        <Live2DCanvas
          modelUrl={modelUrl}
          controlsRef={controlsRef}
          onReady={handleReady}
          onFrame={IS_LINUX_DESKTOP ? handleFrame : undefined}
        />
      ) : (
        <div style={{
          width: '100%', height: '100%', display: 'flex', flexDirection: 'column',
          alignItems: 'center', justifyContent: 'center',
          color: 'rgba(255,255,255,0.8)', fontSize: 13,
          background: 'rgba(0,0,0,0.55)', borderRadius: 10, textAlign: 'center',
        }}>
          <div>No model selected</div>
          <div style={{ fontSize: 9, marginTop: 4, opacity: 0.5 }}>
            Import a model in Settings → Live2D Characters
          </div>
        </div>
      )}

      {/* × close button: HTML on non-Linux; GTK cairo on Linux */}
      {!IS_LINUX_DESKTOP && (
        <button
          data-live2d-btn="1"
          style={{
            ...btnStyle('rgba(180,45,45,0.88)'),
            position: 'fixed', right: BTN_RIGHT, top: BTN_RIGHT,
            opacity: hovered ? 1 : 0,
            pointerEvents: hovered ? 'auto' : 'none',
            transition: 'opacity 0.15s ease',
          }}
          onClick={handleClose} title="Close"
        >×</button>
      )}
    </div>
  );
}
