import { useCallback, useEffect, useRef, useState } from "react";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

const LIVE2D_LABEL = "live2d";

const LIVE2D_URL = import.meta.env.DEV
  ? "http://localhost:1420/live2d.html"
  : "tauri://localhost/live2d.html";

// Mobile: userAgent contains android/iphone/ipad
export const isMobileDevice = /android|iphone|ipad/i.test(navigator.userAgent);

export function useLive2D() {
  const [isOpen, setIsOpen] = useState(false);
  const winRef = useRef<WebviewWindow | null>(null);

  // On mount: re-attach to a stale desktop window that survived hot-reload
  useEffect(() => {
    if (isMobileDevice) return;
    WebviewWindow.getByLabel(LIVE2D_LABEL).then((existing) => {
      if (existing) {
        winRef.current = existing;
        setIsOpen(true);
        existing.once("tauri://destroyed", () => {
          winRef.current = null;
          setIsOpen(false);
        });
      }
    });
  }, []);

  const toggle = useCallback(
    async (modelUrl?: string) => {
      // Mobile: pure state toggle — parent renders Live2DMobileView inline
      if (isMobileDevice) {
        setIsOpen((v) => !v);
        return;
      }

      // Desktop: close if already open
      if (isOpen && winRef.current) {
        await winRef.current.destroy();
        winRef.current = null;
        setIsOpen(false);
        return;
      }

      // Encode modelUrl as a query param so the window reads it on load — no event timing issues
      const url = new URL(LIVE2D_URL);
      if (modelUrl) url.searchParams.set("model", encodeURIComponent(modelUrl));

      const win = new WebviewWindow(LIVE2D_LABEL, {
        url: url.toString(),
        title: "Live2D",
        width: 400,
        height: 600,
        transparent: true,
        decorations: false,
        alwaysOnTop: true,
        resizable: true,
        skipTaskbar: true,
        visible: false, // shown by Live2DWindowApp after model loads to avoid ghost trail on first render
      });

      winRef.current = win;
      win.once("tauri://created", () => setIsOpen(true));
      win.once("tauri://destroyed", () => {
        winRef.current = null;
        setIsOpen(false);
      });
      win.once("tauri://error", () => {
        winRef.current = null;
        setIsOpen(false);
      });
    },
    [isOpen],
  );

  const close = useCallback(async () => {
    if (isMobileDevice) {
      setIsOpen(false);
      return;
    }
    if (winRef.current) {
      await winRef.current.destroy().catch(() => {});
      winRef.current = null;
    }
    setIsOpen(false);
  }, []);

  return { isOpen, toggle, close, isMobileDevice } as const;
}
