import { useRef, useEffect, useState, useCallback } from 'react';
import { X } from 'lucide-react';
import '../../style/ImageCropperModal.css';

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface ImageCropperModalProps {
  src: string;
  aspectRatio?: number;
  onConfirm: (croppedDataUrl: string) => void;
  onCancel: () => void;
  title?: string;
}

export function ImageCropperModal({
  src,
  aspectRatio = 16 / 9,
  onConfirm,
  onCancel,
  title = 'Crop Image',
}: ImageCropperModalProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const lastDragEndTimeRef = useRef(0);
  const draggingHandleRef = useRef<string | null>(null);
  const dragStartPosRef = useRef({ x: 0, y: 0 });
  const cropRectRef = useRef<Rect>({ x: 0, y: 0, width: 0, height: 0 });
  const imgDimsRef = useRef({ width: 0, height: 0 });
  const [imgDims, setImgDims] = useState({ width: 0, height: 0 });
  const [cropRect, setCropRect] = useState<Rect>({ x: 0, y: 0, width: 0, height: 0 });

  // Initialize crop rectangle when image loads
  const handleImageLoad = useCallback((e: React.SyntheticEvent<HTMLImageElement>) => {
    const img = e.currentTarget;
    let w = img.naturalWidth;
    let h = img.naturalHeight;

    // Scale down aggressively for display: max 700px width
    const maxDisplayWidth = 700;
    const maxDisplayHeight = 700;

    if (w > maxDisplayWidth || h > maxDisplayHeight) {
      const ratio = Math.min(maxDisplayWidth / w, maxDisplayHeight / h);
      w = Math.floor(w * ratio);
      h = Math.floor(h * ratio);
    }

    setImgDims({ width: w, height: h });

    // For wallpaper: use 16:9 aspect ratio with 1920x1080 default
    // For icon: use 1:1 aspect ratio with 512x512 default
    const targetWidth = aspectRatio === 1 ? 512 : 1920;
    const targetHeight = aspectRatio === 1 ? 512 : 1080;

    // Center the crop rect with target dimensions
    let rect = {
      x: Math.max(0, Math.floor((w - targetWidth) / 2)),
      y: Math.max(0, Math.floor((h - targetHeight) / 2)),
      width: targetWidth,
      height: targetHeight,
    };

    // Clamp height to image bounds, adjust width to maintain aspect ratio
    if (rect.height > h - rect.y) {
      rect.height = h - rect.y;
      rect.width = rect.height * aspectRatio;
      // Recenter if width changed
      rect.x = Math.max(0, Math.floor((w - rect.width) / 2));
    }

    // For icon, also clamp width
    if (aspectRatio === 1) {
      if (rect.width > w - rect.x) {
        rect.width = w - rect.x;
        rect.height = rect.width;
        // Recenter if height changed
        rect.y = Math.max(0, Math.floor((h - rect.height) / 2));
      }
    }

    setCropRect(rect);
  }, [aspectRatio]);

  const handleMouseDown = useCallback((e: React.MouseEvent, handle: string) => {
    e.preventDefault();
    e.stopPropagation();
    lastDragEndTimeRef.current = Date.now();
    draggingHandleRef.current = handle;
    dragStartPosRef.current = { x: e.clientX, y: e.clientY };
    cropRectRef.current = cropRect;
    imgDimsRef.current = imgDims;
  }, [cropRect, imgDims]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!draggingHandleRef.current) return;

    const handle = draggingHandleRef.current;
    const deltaX = e.clientX - dragStartPosRef.current.x;
    const deltaY = e.clientY - dragStartPosRef.current.y;
    const { height: imgH, width: imgW } = imgDimsRef.current;
    const minSize = 50;
    const prev = cropRectRef.current;

    let r = { ...prev };

    // Calculate new rect based on handle
    switch (handle) {
      case 'nw': {
        const newW = Math.max(minSize, prev.width - deltaX);
        const newH = newW / aspectRatio;
        r.x = prev.x + prev.width - newW;
        r.y = prev.y + prev.height - newH;
        r.width = newW;
        r.height = newH;
        break;
      }
      case 'ne': {
        const newW = Math.max(minSize, prev.width + deltaX);
        const newH = newW / aspectRatio;
        r.y = prev.y + prev.height - newH;
        r.width = newW;
        r.height = newH;
        break;
      }
      case 'sw': {
        const newW = Math.max(minSize, prev.width - deltaX);
        const newH = newW / aspectRatio;
        r.x = prev.x + prev.width - newW;
        r.width = newW;
        r.height = newH;
        break;
      }
      case 'se': {
        const newW = Math.max(minSize, prev.width + deltaX);
        const newH = newW / aspectRatio;
        r.width = newW;
        r.height = newH;
        break;
      }
      case 'move': {
        r.x = prev.x + deltaX;
        r.y = prev.y + deltaY;
        break;
      }
    }

    // Apply constraints to keep area completely within image bounds
    // First, constrain dimensions to not exceed image
    r.width = Math.min(r.width, imgW);
    r.height = Math.min(r.height, imgH);

    // Re-adjust width to maintain aspect ratio if height was constrained
    if (r.height * aspectRatio !== r.width) {
      r.width = r.height * aspectRatio;
      // If width exceeds image, constrain it
      if (r.width > imgW) {
        r.width = imgW;
        r.height = r.width / aspectRatio;
      }
    }

    // Constrain position to ensure area stays within bounds
    r.x = Math.max(0, Math.min(r.x, imgW - r.width));
    r.y = Math.max(0, Math.min(r.y, imgH - r.height));

    setCropRect(r);
  }, [aspectRatio]);

  const handleMouseUp = useCallback(() => {
    lastDragEndTimeRef.current = Date.now();
    draggingHandleRef.current = null;
  }, []);

  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => handleMouseMove(e as any);
    const onMouseUp = () => handleMouseUp();

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
    return () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
    };
  }, [handleMouseMove, handleMouseUp]);

  const handleConfirm = () => {
    const canvas = canvasRef.current;
    const img = imgRef.current;
    if (!canvas || !img) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Use source image's natural dimensions to crop from original
    const scale = img.naturalWidth / imgDims.width;
    const srcX = Math.round(cropRect.x * scale);
    const srcY = Math.round(cropRect.y * scale);
    const srcW = Math.round(cropRect.width * scale);
    const srcH = Math.round(cropRect.height * scale);

    // Scale output to reasonable size (max 1920x1920 for wallpaper, 512x512 for icon)
    const maxOutputSize = aspectRatio === 1 ? 512 : 1920;
    const outputScale = Math.min(maxOutputSize / srcW, maxOutputSize / srcH, 1);
    const outW = Math.round(srcW * outputScale);
    const outH = Math.round(srcH * outputScale);

    canvas.width = outW;
    canvas.height = outH;

    ctx.drawImage(
      img,
      srcX,
      srcY,
      srcW,
      srcH,
      0,
      0,
      outW,
      outH,
    );

    // Use high quality PNG to preserve patterns
    const dataUrl = canvas.toDataURL('image/png');
    onConfirm(dataUrl);
  };

  return (
    <div className="image-cropper-modal-overlay" onClick={() => {
      // Ignore clicks that happen immediately after a drag ended (within 100ms)
      if (Date.now() - lastDragEndTimeRef.current < 100) {
        return;
      }
      onCancel();
    }}>
      <div className="image-cropper-modal" onClick={(e) => e.stopPropagation()}>
        <div className="image-cropper-header">
          <h2>{title}</h2>
          <button className="image-cropper-close" onClick={onCancel}>
            <X size={20} />
          </button>
        </div>

        <div className="image-cropper-body">
          <div
            ref={containerRef}
            className="image-cropper-canvas-wrap"
            style={{ width: imgDims.width, height: imgDims.height }}
          >
            <img
              ref={imgRef}
              src={src}
              alt="Crop"
              onLoad={handleImageLoad}
              style={{ width: imgDims.width, height: imgDims.height, display: 'block' }}
            />

            {imgDims.width > 0 && (
              <>
                {/* Darkened areas outside crop */}
                <div className="image-cropper-overlay" />

                {/* Crop selection box */}
                <div
                  className="image-cropper-selection"
                  style={{
                    left: cropRect.x,
                    top: cropRect.y,
                    width: cropRect.width,
                    height: cropRect.height,
                  }}
                  onMouseDown={(e) => {
                    if ((e.target as HTMLElement).classList.contains('image-cropper-selection')) {
                      handleMouseDown(e, 'move');
                    }
                  }}
                >
                  {/* Corner handles */}
                  <div
                    className="image-cropper-handle image-cropper-handle-nw"
                    onMouseDown={(e) => handleMouseDown(e, 'nw')}
                  />
                  <div
                    className="image-cropper-handle image-cropper-handle-ne"
                    onMouseDown={(e) => handleMouseDown(e, 'ne')}
                  />
                  <div
                    className="image-cropper-handle image-cropper-handle-sw"
                    onMouseDown={(e) => handleMouseDown(e, 'sw')}
                  />
                  <div
                    className="image-cropper-handle image-cropper-handle-se"
                    onMouseDown={(e) => handleMouseDown(e, 'se')}
                  />

                  {/* Edge lines */}
                  <div className="image-cropper-edge image-cropper-edge-h" />
                  <div className="image-cropper-edge image-cropper-edge-v" />
                </div>
              </>
            )}
          </div>
        </div>

        <div className="image-cropper-actions">
          <button className="image-cropper-btn image-cropper-btn-cancel" onClick={onCancel}>
            Cancel
          </button>
          <button className="image-cropper-btn image-cropper-btn-confirm" onClick={handleConfirm}>
            Confirm
          </button>
        </div>
      </div>

      <canvas ref={canvasRef} style={{ display: 'none' }} />
    </div>
  );
}
