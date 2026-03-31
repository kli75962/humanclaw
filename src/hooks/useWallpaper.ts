import { useState, useEffect, useCallback } from 'react';

const WALLPAPER_KEY = 'phoneclaw_wallpaper';
const BLUR_KEY = 'phoneclaw_wallpaper_blur';
const DIM_KEY = 'phoneclaw_wallpaper_dim';

function readStorage() {
  return {
    url: localStorage.getItem(WALLPAPER_KEY) ?? '',
    blur: Number(localStorage.getItem(BLUR_KEY) ?? '0'),
    dim: Number(localStorage.getItem(DIM_KEY) ?? '0'),
  };
}

function loadImageAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = reject;
    reader.readAsDataURL(file);
  });
}

export function useWallpaper() {
  const [state, setState] = useState(readStorage);
  const [cropperSrc, setCropperSrc] = useState('');
  const [onCropperConfirm, setOnCropperConfirm] = useState<((data: string) => void) | null>(null);

  useEffect(() => {
    function onChanged() { setState(readStorage()); }
    document.addEventListener('wallpaper-changed', onChanged);
    return () => document.removeEventListener('wallpaper-changed', onChanged);
  }, []);

  const handleCropperConfirm = useCallback((croppedDataUrl: string) => {
    // Ensure data URL is valid before calling callback
    if (croppedDataUrl && croppedDataUrl.startsWith('data:')) {
      if (onCropperConfirm) {
        onCropperConfirm(croppedDataUrl);
        setOnCropperConfirm(null);
      }
    }
    setCropperSrc('');
  }, [onCropperConfirm]);

  async function loadWallpaperFile(file: File) {
    const dataUrl = await loadImageAsDataUrl(file);
    setCropperSrc(dataUrl);

    return new Promise<void>((resolve) => {
      setOnCropperConfirm(() => (croppedUrl: string) => {
        // Store PNG directly - no additional compression
        try { localStorage.setItem(WALLPAPER_KEY, croppedUrl); } catch { /* quota */ }
        document.dispatchEvent(new CustomEvent('wallpaper-changed'));
        resolve();
      });
    });
  }

  function clearWallpaper() {
    localStorage.removeItem(WALLPAPER_KEY);
    document.dispatchEvent(new CustomEvent('wallpaper-changed'));
  }

  function setBlur(val: number) {
    localStorage.setItem(BLUR_KEY, String(val));
    document.dispatchEvent(new CustomEvent('wallpaper-changed'));
  }

  function setDim(val: number) {
    localStorage.setItem(DIM_KEY, String(val));
    document.dispatchEvent(new CustomEvent('wallpaper-changed'));
  }

  return {
    ...state,
    loadWallpaperFile,
    clearWallpaper,
    setBlur,
    setDim,
    cropperSrc,
    onCropperConfirm: handleCropperConfirm,
    onCropperCancel: () => setCropperSrc(''),
  };
}
