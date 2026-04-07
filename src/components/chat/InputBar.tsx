import { forwardRef, memo, useImperativeHandle, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Send, PhoneCall, X, Paperclip, File, Loader } from 'lucide-react';
import type { InputBarProps, InputBarHandle } from '../../types';
import '../../style/InputBar.css';

const ACCEPTED_TYPES = '.pdf,.doc,.docx,.odt,.ods,.odp,.md,.txt,.csv,.json,.xml,.html,.rtf';
const TEXT_EXTS = new Set(['.txt', '.md', '.csv', '.json', '.xml', '.html', '.htm', '.rtf', '.log', '.yaml', '.yml', '.toml', '.ini', '.cfg']);
const OFFICE_EXTS = new Set(['.docx', '.pptx', '.xlsx', '.odt', '.odp', '.ods']);

interface AttachedFile {
  id: string;
  name: string;
  content: string | null;   // null = unreadable / still loading
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

/** Read content from a browser File object. */
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

/** Read content from an OS filesystem path (drag-drop). */
async function readPathFile(path: string): Promise<string | null> {
  return invoke<string>('read_file_text', { path }).catch(() => null);
}

export const InputBar = memo(forwardRef<InputBarHandle, InputBarProps>(function InputBar(
  { isThinking, isListening, sttError, onSend, onSttToggle, onStop, quotedPost, onClearQuote },
  ref,
) {
  const [value, setValue] = useState('');
  const valueRef = useRef(value);
  valueRef.current = value;
  const [attachedFiles, setAttachedFiles] = useState<AttachedFile[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);

  /** Add a chip immediately, then resolve content asynchronously. */
  function addWithRead(name: string, readFn: () => Promise<string | null>) {
    const id = uid();
    setAttachedFiles((prev) => [...prev, { id, name, content: null, loading: true }]);
    readFn().then((content) => {
      setAttachedFiles((prev) =>
        prev.map((f) => f.id === id ? { ...f, content, loading: false } : f),
      );
    });
  }

  useImperativeHandle(ref, () => ({
    setInput:       (text: string) => setValue(text),
    getInput:       () => valueRef.current,
    attachFile:     (file: File)   => addWithRead(file.name, () => readBrowserFile(file)),
    attachFilePath: (path: string) => addWithRead(basename(path), () => readPathFile(path)),
  }), []);

  const handleSend = () => {
    if ((!value.trim() && attachedFiles.length === 0) || isThinking) return;

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
      const names = noContent.map((f) => f.name).join(', ');
      msg += `\n[Note: could not read content of: ${names}]`;
    }

    onSend(msg);
    setValue('');
    setAttachedFiles([]);
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
            Array.from(files).forEach((file) => addWithRead(file.name, () => readBrowserFile(file)));
            e.target.value = '';
          }}
        />
        <input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && !isThinking && !stillLoading && handleSend()}
          placeholder={
            stillLoading    ? 'Reading file…' :
            isListening     ? 'Listening…' :
            isThinking      ? 'Waiting for response…' :
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
            disabled={(!value.trim() && attachedFiles.length === 0) || stillLoading}
            className="inputbar-btn inputbar-send-btn"
          >
            <Send size={22} />
          </button>
        )}
      </div>
    </div>
  );
}));
