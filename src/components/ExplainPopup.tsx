import { Fragment, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { BookMarked, MessageSquarePlus, X } from 'lucide-react';
import { formatAssistantText } from './ChatMessage';
import type { Message } from '../types';
import '../style/ExplainPopup.css';

interface ExplainPopupProps {
  selectedText: string;
  model: string;
  contextMessages: Message[];
  onClose: () => void;
  onSaveMemo: (title: string, messages: Message[]) => Promise<string | null>;
  onOpenMemo: (id: string) => void;
}

export function ExplainPopup({
  selectedText, model, contextMessages, onClose, onSaveMemo, onOpenMemo,
}: ExplainPopupProps) {
  const [explanation, setExplanation] = useState('');
  const [done, setDone] = useState(false);
  const [savedId, setSavedId] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const explanationRef = useRef('');

  // Draggable position
  const [pos, setPos] = useState(() => ({
    x: Math.max(20, Math.round(window.innerWidth / 2 - 270)),
    y: 72,
  }));
  const dragOffset = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    function onMove(e: MouseEvent) {
      if (!dragOffset.current) return;
      setPos({
        x: Math.max(0, Math.min(window.innerWidth - 540, e.clientX - dragOffset.current.x)),
        y: Math.max(0, Math.min(window.innerHeight - 120, e.clientY - dragOffset.current.y)),
      });
    }
    function onUp() { dragOffset.current = null; }
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    return () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
  }, []);

  // Streaming
  useEffect(() => {
    let cancelled = false;
    explanationRef.current = '';
    setExplanation('');
    setDone(false);
    setSavedId(null);

    const unlistenPromise = listen<{ content: string; done: boolean }>('explain-stream', (e) => {
      if (cancelled) return;
      if (e.payload.done) {
        setDone(true);
      } else {
        explanationRef.current += e.payload.content;
        setExplanation(explanationRef.current);
      }
    });

    const apiMessages = [
      ...contextMessages
        .filter((m) => m.role === 'user' || m.role === 'assistant')
        .slice(-6)
        .map((m) => ({ role: m.role, content: m.content })),
      {
        role: 'user',
        content: `Based on the conversation above, explain what this specific term or phrase means in that context: "${selectedText}"`,
      },
    ];

    invoke('explain_text', { messages: apiMessages, model }).catch((err: unknown) => {
      if (!cancelled) {
        explanationRef.current = `Error: ${err}`;
        setExplanation(explanationRef.current);
        setDone(true);
      }
    });

    return () => {
      cancelled = true;
      unlistenPromise.then((fn) => fn());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedText, model]);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [explanation]);

  function buildMemoMessages(): Message[] {
    return [
      { role: 'user', content: `Explain this: "${selectedText}"` },
      { role: 'assistant', content: explanationRef.current },
    ];
  }

  async function handleSave() {
    if (savedId) return;
    const id = await onSaveMemo(selectedText.slice(0, 80), buildMemoMessages());
    if (id) { setSavedId(id); document.dispatchEvent(new Event('memo-saved')); }
  }

  async function handleSaveAndOpen() {
    let id = savedId;
    if (!id) {
      id = await onSaveMemo(selectedText.slice(0, 80), buildMemoMessages());
      if (id) { setSavedId(id); document.dispatchEvent(new Event('memo-saved')); }
    }
    if (id) { onOpenMemo(id); onClose(); }
  }

  const formatted = formatAssistantText(explanation);

  return (
    <div
      className="explain-popup"
      style={{ left: pos.x, top: pos.y }}
    >
      {/* Drag handle = header */}
      <div
        className="explain-header"
        onMouseDown={(e) => {
          if ((e.target as HTMLElement).closest('button')) return;
          dragOffset.current = { x: e.clientX - pos.x, y: e.clientY - pos.y };
        }}
      >
        <div className="explain-header-left">
          <button
            className={`explain-icon-btn${savedId ? ' explain-icon-btn--done' : ''}`}
            onClick={handleSave}
            disabled={!done || !!savedId}
            title={savedId ? 'Saved' : 'Save as memo'}
          >
            <BookMarked size={15} />
          </button>
          <button
            className="explain-icon-btn"
            onClick={handleSaveAndOpen}
            disabled={!done}
            title={savedId ? 'Open memo chat' : 'Save & open as chat'}
          >
            <MessageSquarePlus size={15} />
          </button>
        </div>

        <span className="explain-title">Explanation</span>

        <button className="explain-icon-btn explain-icon-btn--close" onClick={onClose}>
          <X size={15} />
        </button>
      </div>

      <div className="explain-selected">
        <span className="explain-selected-label">Selected text</span>
        <blockquote className="explain-selected-text">{selectedText}</blockquote>
      </div>

      <div className="explain-body">
        {explanation ? (
          formatted.split('\n').map((line, i, arr) => (
            <Fragment key={i}>
              {line}
              {i < arr.length - 1 && <br />}
            </Fragment>
          ))
        ) : (
          !done && <span className="explain-thinking">Thinking…</span>
        )}
        <div ref={scrollRef} />
      </div>
    </div>
  );
}
