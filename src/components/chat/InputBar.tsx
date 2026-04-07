import { forwardRef, memo, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Send, PhoneCall, X, Paperclip, File, Loader, AlertCircle } from 'lucide-react';
import type { InputBarProps, InputBarHandle } from '../../types';
import '../../style/InputBar.css';

const ACCEPTED_TYPES = '.pdf,.doc,.docx,.odt,.ods,.odp,.md,.txt,.csv,.json,.xml,.html,.rtf,image/*';
const TEXT_EXTS  = new Set(['.txt', '.md', '.csv', '.json', '.xml', '.html', '.htm', '.rtf', '.log', '.yaml', '.yml', '.toml', '.ini', '.cfg']);
const OFFICE_EXTS = new Set(['.docx', '.pptx', '.xlsx', '.odt', '.odp', '.ods']);
const EXTRA_EXTS  = new Set(['.pdf', '.doc']);   // in ACCEPTED_TYPES but not text/office
const IMAGE_EXTS  = new Set(['.png', '.jpg', '.jpeg', '.gif', '.webp', '.bmp', '.avif', '.tiff', '.tif', '.svg']);

const EXT_TO_MIME: Record<string, string> = {
  '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
  '.gif': 'image/gif', '.webp': 'image/webp', '.bmp': 'image/bmp',
  '.avif': 'image/avif', '.tiff': 'image/tiff', '.tif': 'image/tiff',
  '.svg': 'image/svg+xml',
};

function isAcceptedExt(ext: string): boolean {
  return TEXT_EXTS.has(ext) || OFFICE_EXTS.has(ext) || IMAGE_EXTS.has(ext) || EXTRA_EXTS.has(ext);
}

function typeLabel(ext: string): string {
  return ext.replace('.', '').toUpperCase() || ext;
}

interface AttachedImage {
  id: string;
  name: string;
  dataUrl: string;
}

interface AttachedFile {
  id: string;
  name: string;
  content: string | null;
  loading: boolean;
}

function extOf(name: string) {
  return '.' + (name.split('.').pop() ?? '').toLowerCase();
}

function basename(path: string) {
  return path.replace(/\\/g, '/').split('/').pop() ?? path;
}

function uid() {
  return Math.random().toString(36).slice(2);
}

async function readBrowserFile(file: File): Promise<string | null> {
  const ext = extOf(file.name);
  if (TEXT_EXTS.has(ext)) {
    return file.text().catch(() => null);
  }
  if (OFFICE_EXTS.has(ext)) {
    try {
      const buf = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      return await invoke<string>('extract_file_text_from_bytes', { bytes, filename: file.name });
    } catch {
      return null;
    }
  }
  return null;
}

function readBrowserImageFile(file: File): Promise<AttachedImage> {
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = () => {
      resolve({ id: uid(), name: file.name, dataUrl: reader.result as string });
    };
    reader.readAsDataURL(file);
  });
}

async function readPathFile(path: string): Promise<string | null> {
  return invoke<string>('read_file_text', { path }).catch(() => null);
}

async function readPathImageFile(path: string): Promise<AttachedImage | null> {
  try {
    const name = basename(path);
    const ext  = extOf(name);
    const mime = EXT_TO_MIME[ext] ?? 'image/png';
    const b64  = await invoke<string>('read_file_as_base64', { path });
    return { id: uid(), name, dataUrl: `data:${mime};base64,${b64}` };
  } catch {
    return null;
  }
}

export const InputBar = memo(forwardRef<InputBarHandle, InputBarProps>(function InputBar(
  { isThinking, isListening, sttError, onSend, onSttToggle, onStop, quotedPost, onClearQuote },
  ref,
) {
  const [value, setValue] = useState('');
  const valueRef = useRef(value);
  valueRef.current = value;
  const [attachedFiles, setAttachedFiles]   = useState<AttachedFile[]>([]);
  const [attachedImages, setAttachedImages] = useState<AttachedImage[]>([]);
  const [alertMsg, setAlertMsg]             = useState<string | null>(null);
  const alertTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const fileInputRef  = useRef<HTMLInputElement>(null);

  function showAlert(msg: string) {
    if (alertTimerRef.current) clearTimeout(alertTimerRef.current);
    setAlertMsg(msg);
    alertTimerRef.current = setTimeout(() => setAlertMsg(null), 4000);
  }

  function addWithRead(name: string, readFn: () => Promise<string | null>) {
    const id = uid();
    setAttachedFiles((prev) => [...prev, { id, name, content: null, loading: true }]);
    readFn().then((content) => {
      setAttachedFiles((prev) =>
        prev.map((f) => f.id === id ? { ...f, content, loading: false } : f),
      );
    });
  }

  /** Route a browser File to image or text handler; shows alert for unsupported types. */
  function addBrowserFile(file: File) {
    const ext = extOf(file.name);
    if (file.type.startsWith('image/') || IMAGE_EXTS.has(ext)) {
      readBrowserImageFile(file).then((img) => setAttachedImages((prev) => [...prev, img]));
    } else if (isAcceptedExt(ext)) {
      addWithRead(file.name, () => readBrowserFile(file));
    } else {
      showAlert(`${typeLabel(ext)} file is not accepted`);
    }
  }

  /** Attach a clipboard image DataTransferItem. */
  function addClipboardImage(item: DataTransferItem) {
    const file = item.getAsFile();
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      const ext = item.type.split('/')[1]?.split('+')[0] ?? 'png';
      setAttachedImages((prev) => [...prev, { id: uid(), name: `image_${Date.now()}.${ext}`, dataUrl }]);
    };
    reader.readAsDataURL(file);
  }

  /** Process a list of OS file paths (from clipboard URI list or drag-drop). */
  function addFilePaths(paths: string[]) {
    for (const path of paths) {
      const name = basename(path);
      const ext  = extOf(name);
      if (IMAGE_EXTS.has(ext)) {
        readPathImageFile(path).then((img) => {
          if (img) setAttachedImages((prev) => [...prev, img]);
        });
      } else if (isAcceptedExt(ext)) {
        addWithRead(name, () => readPathFile(path));
      } else {
        showAlert(`${typeLabel(ext)} file is not accepted`);
      }
    }
  }

  useImperativeHandle(ref, () => ({
    setInput:        (text: string) => setValue(text),
    getInput:        () => valueRef.current,
    attachFile:      (file: File)   => addBrowserFile(file),
    attachFilePath:  (path: string) => addFilePaths([path]),
    attachImagePath: (path: string) => {
      readPathImageFile(path).then((img) => {
        if (img) setAttachedImages((prev) => [...prev, img]);
      });
    },
  }), []);

  useEffect(() => {
    // keydown Ctrl+V: handles images and file URIs on Linux (wl-paste/xclip).
    // We do NOT call preventDefault so that text paste still works normally.
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (!((e.ctrlKey || e.metaKey) && e.key === 'v')) return;

      // 1. Try clipboard image via Rust (Wayland/X11)
      try {
        const dataUrl = await invoke<string | null>('get_clipboard_image');
        if (dataUrl) {
          const mime = dataUrl.split(';')[0].split(':')[1] ?? 'image/png';
          const ext  = mime.split('/')[1]?.split('+')[0] ?? 'png';
          setAttachedImages((prev) => [...prev, { id: uid(), name: `image_${Date.now()}.${ext}`, dataUrl }]);
          return;
        }
      } catch { /* no image */ }

      // 2. Try file URI list via Rust (wl-paste text/uri-list on Wayland)
      try {
        const paths = await invoke<string[]>('get_clipboard_uri_list');
        if (paths.length > 0) {
          addFilePaths(paths);
        }
      } catch { /* wl-paste unavailable */ }
    };

    // paste DOM event: handles images and files on macOS/Windows via browser clipboard API.
    const handlePaste = (e: ClipboardEvent) => {
      // Files pasted from OS file manager (macOS/Windows)
      const files = e.clipboardData?.files;
      if (files && files.length > 0) {
        Array.from(files).forEach(addBrowserFile);
        return;
      }
      // Image blobs in clipboardData items (macOS/Windows)
      const items = e.clipboardData?.items;
      if (!items) return;
      for (const item of Array.from(items)) {
        if (item.type.startsWith('image/')) {
          addClipboardImage(item);
        }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('paste', handlePaste);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('paste', handlePaste);
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleSend = () => {
    if ((!value.trim() && attachedFiles.length === 0 && attachedImages.length === 0) || isThinking) return;

    let msg = value.trim();

    const withContent = attachedFiles.filter((f) => f.content);
    const noContent   = attachedFiles.filter((f) => !f.content && !f.loading);

    if (withContent.length > 0) {
      const blocks = withContent
        .map((f) => `<file name="${f.name}">\n${f.content}\n</file>`)
        .join('\n\n');
      msg = `${blocks}\n\n${msg}`.trim();
    }
    if (noContent.length > 0) {
      msg += `\n[Note: could not read content of: ${noContent.map((f) => f.name).join(', ')}]`;
    }
    if (attachedImages.length > 0) {
      const imgBlocks = attachedImages.map((img) => {
        const mime = img.dataUrl.split(';')[0].split(':')[1] ?? 'image/png';
        const b64  = img.dataUrl.split(',')[1] ?? '';
        return `<image name="${img.name}" mime="${mime}" data="${b64}"/>`;
      }).join('\n');
      msg = `${imgBlocks}\n\n${msg}`.trim();
    }

    onSend(msg);
    setValue('');
    setAttachedFiles([]);
    setAttachedImages([]);
  };

  const stillLoading = attachedFiles.some((f) => f.loading);

  return (
    <div className="inputbar">
      {sttError && (
        <div className="inputbar-error">{sttError}</div>
      )}
      {quotedPost && (
        <div className="inputbar-quote-preview">
          <span className="inputbar-quote-text">{quotedPost.text}</span>
          <button className="inputbar-quote-clear" onClick={onClearQuote} aria-label="Clear quote">
            <X size={14} />
          </button>
        </div>
      )}
      {attachedImages.length > 0 && (
        <div className="inputbar-images">
          {attachedImages.map((img) => (
            <div key={img.id} className="inputbar-image-preview">
              <img src={img.dataUrl} alt={img.name} className="inputbar-image-thumb" />
              <button
                className="inputbar-image-remove"
                onClick={() => setAttachedImages((prev) => prev.filter((x) => x.id !== img.id))}
                aria-label="Remove image"
              >
                <X size={11} />
              </button>
            </div>
          ))}
        </div>
      )}
      {attachedFiles.length > 0 && (
        <div className="inputbar-files">
          {attachedFiles.map((f) => (
            <div
              key={f.id}
              className={`inputbar-file-chip${f.loading ? ' inputbar-file-chip--loading' : ''}${!f.loading && !f.content ? ' inputbar-file-chip--error' : ''}`}
            >
              {f.loading
                ? <Loader size={12} className="inputbar-file-spin" />
                : <File size={12} />
              }
              <span className="inputbar-file-name">{f.name}</span>
              <button
                className="inputbar-file-remove"
                onClick={() => setAttachedFiles((prev) => prev.filter((x) => x.id !== f.id))}
                aria-label="Remove file"
              >
                <X size={11} />
              </button>
            </div>
          ))}
        </div>
      )}
      {alertMsg && (
        <div className="inputbar-alert">
          <AlertCircle size={13} className="inputbar-alert-icon" />
          <span className="inputbar-alert-text">{alertMsg}</span>
          <button
            className="inputbar-alert-close"
            onClick={() => { if (alertTimerRef.current) clearTimeout(alertTimerRef.current); setAlertMsg(null); }}
            aria-label="Close alert"
          >
            <X size={12} />
          </button>
        </div>
      )}
      <div className="inputbar-row">
        <button
          type="button"
          onClick={() => fileInputRef.current?.click()}
          disabled={isThinking}
          className="inputbar-btn inputbar-attach-btn"
          aria-label="Attach file"
        >
          <Paperclip size={20} />
        </button>
        <input
          ref={fileInputRef}
          type="file"
          accept={ACCEPTED_TYPES}
          multiple
          style={{ display: 'none' }}
          onChange={(e) => {
            const files = e.target.files;
            if (!files || files.length === 0) return;
            Array.from(files).forEach(addBrowserFile);
            e.target.value = '';
          }}
        />
        <input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && !isThinking && !stillLoading && handleSend()}
          placeholder={
            stillLoading ? 'Reading file…' :
            isListening  ? 'Listening…' :
            isThinking   ? 'Waiting for response…' :
                           'Enter a prompt here'
          }
          disabled={isThinking}
          className="inputbar-input"
        />
        <button
          onClick={onSttToggle}
          disabled={isThinking}
          className={`inputbar-btn inputbar-stt-btn${isListening ? ' inputbar-stt-btn--listening' : ''}`}
        >
          <PhoneCall size={22} />
        </button>
        {isThinking ? (
          <button onClick={onStop} className="inputbar-btn inputbar-stop-btn">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor">
              <rect x="4" y="4" width="12" height="12" rx="2" />
            </svg>
          </button>
        ) : (
          <button
            onClick={handleSend}
            disabled={(!value.trim() && attachedFiles.length === 0 && attachedImages.length === 0) || stillLoading}
            className="inputbar-btn inputbar-send-btn"
          >
            <Send size={22} />
          </button>
        )}
      </div>
    </div>
  );
}));
