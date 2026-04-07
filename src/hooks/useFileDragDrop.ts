import { useState, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { InputBarHandle } from '../types';

export function useFileDragDrop(inputBarRef: React.RefObject<InputBarHandle | null>) {
  const [isDraggingFile, setIsDraggingFile] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let unlistenFn: (() => void) | null = null;
    getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type === 'enter') {
        setIsDraggingFile(true);
      } else if (event.payload.type === 'leave') {
        setIsDraggingFile(false);
      } else if (event.payload.type === 'drop') {
        setIsDraggingFile(false);
        for (const path of event.payload.paths) {
          inputBarRef.current?.attachFilePath(path);
        }
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlistenFn = fn;
    });
    return () => {
      cancelled = true;
      unlistenFn?.();
    };
  }, [inputBarRef]);

  return isDraggingFile;
}
