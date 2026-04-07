import { useState, useRef, useCallback } from 'react';

const SIDE_WIDTH_KEY = 'phoneclaw_side_width';
const MIN_SIDE = 200;
const MAX_SIDE_RATIO = 0.6;

export function useDragSidebar() {
  const [sideWidth, setSideWidth] = useState(() => {
    const saved = localStorage.getItem(SIDE_WIDTH_KEY);
    return saved ? Number(saved) : Math.floor(window.innerWidth * 0.33);
  });

  const dragging = useRef(false);
  const dragStartX = useRef(0);
  const dragStartW = useRef(0);

  const handleDividerPointerDown = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    dragging.current = true;
    dragStartX.current = e.clientX;
    dragStartW.current = sideWidth;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }, [sideWidth]);

  const handleDividerPointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragging.current) return;
    const delta = e.clientX - dragStartX.current;
    const maxSide = Math.floor(window.innerWidth * MAX_SIDE_RATIO);
    const next = Math.max(MIN_SIDE, Math.min(maxSide, dragStartW.current + delta));
    setSideWidth(next);
  }, []);

  const handleDividerPointerUp = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    setSideWidth((w) => { localStorage.setItem(SIDE_WIDTH_KEY, String(w)); return w; });
  }, []);

  return {
    sideWidth,
    handleDividerPointerDown,
    handleDividerPointerMove,
    handleDividerPointerUp
  };
}
