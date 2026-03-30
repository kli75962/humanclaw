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
  const [followUpInput, setFollowUpInput] = useState('');
  const [messages, setMessages] = useState<Array<{ role: 'user' | 'assistant'; content: string }>>([]);
  const [isLoadingFollowUp, setIsLoadingFollowUp] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const explanationRef = useRef('');

  // Draggable position and resizable size
  const [pos, setPos] = useState(() => {
    try {
      const saved = localStorage.getItem('explain-popup-pos');
      if (saved) {
        const parsed = JSON.parse(saved);
        return {
          x: Math.max(0, Math.min(window.innerWidth - 300, parsed.x)),
          y: Math.max(0, Math.min(window.innerHeight - 120, parsed.y)),
        };
      }
    } catch {}
    return {
      x: Math.max(20, Math.round(window.innerWidth / 2 - 270)),
      y: 72,
    };
  });

  const [size, setSize] = useState(() => {
    try {
      const saved = localStorage.getItem('explain-popup-size');
      if (saved) {
        const parsed = JSON.parse(saved);
        return {
          width: Math.max(300, Math.min(window.innerWidth - pos.x, parsed.width)),
          height: Math.max(300, Math.min(window.innerHeight - pos.y, parsed.height)),
        };
      }
    } catch {}
    return { width: 540, height: 400 };
  });

  const dragOffset = useRef<{ x: number; y: number } | null>(null);
  const resizeOffset = useRef<{
    startX: number;
    startY: number;
    startWidth: number;
    startHeight: number;
    startPosX: number;
    startPosY: number;
    direction: 'n' | 's' | 'e' | 'w' | 'nw' | 'ne' | 'sw' | 'se';
  } | null>(null);

  useEffect(() => {
    function onMove(e: MouseEvent) {
      if (dragOffset.current) {
        const newPos = {
          x: Math.max(0, Math.min(window.innerWidth - size.width, e.clientX - dragOffset.current.x)),
          y: Math.max(0, Math.min(window.innerHeight - 120, e.clientY - dragOffset.current.y)),
        };
        setPos(newPos);
        localStorage.setItem('explain-popup-pos', JSON.stringify(newPos));
      }
      if (resizeOffset.current) {
        const deltaX = e.clientX - resizeOffset.current.startX;
        const deltaY = e.clientY - resizeOffset.current.startY;
        const dir = resizeOffset.current.direction;

        let newWidth = resizeOffset.current.startWidth;
        let newHeight = resizeOffset.current.startHeight;
        let newPosX = resizeOffset.current.startPosX;
        let newPosY = resizeOffset.current.startPosY;

        // Handle horizontal resize
        if (dir.includes('e')) {
          newWidth = Math.max(300, resizeOffset.current.startWidth + deltaX);
        } else if (dir.includes('w')) {
          newWidth = Math.max(300, resizeOffset.current.startWidth - deltaX);
          newPosX = resizeOffset.current.startPosX + deltaX;
        }

        // Handle vertical resize
        if (dir.includes('s')) {
          newHeight = Math.max(300, resizeOffset.current.startHeight + deltaY);
        } else if (dir.includes('n')) {
          newHeight = Math.max(300, resizeOffset.current.startHeight - deltaY);
          newPosY = resizeOffset.current.startPosY + deltaY;
        }

        // Constrain to screen bounds
        newPosX = Math.max(0, Math.min(window.innerWidth - newWidth, newPosX));
        newPosY = Math.max(0, Math.min(window.innerHeight - 40, newPosY));

        const newSize = { width: newWidth, height: newHeight };
        const newPos = { x: newPosX, y: newPosY };

        setSize(newSize);
        setPos(newPos);
        localStorage.setItem('explain-popup-size', JSON.stringify(newSize));
        localStorage.setItem('explain-popup-pos', JSON.stringify(newPos));
      }
    }
    function onUp() {
      dragOffset.current = null;
      resizeOffset.current = null;
    }
    document.addEventListener('mousemove', onMove);
    document.addEventListener('mouseup', onUp);
    return () => {
      document.removeEventListener('mousemove', onMove);
      document.removeEventListener('mouseup', onUp);
    };
  }, [pos, size]);

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

  async function handleSendFollowUp() {
    if (!followUpInput.trim() || isLoadingFollowUp) return;

    const userMessage = followUpInput;
    setFollowUpInput('');
    setMessages((prev) => [...prev, { role: 'user', content: userMessage }]);
    setIsLoadingFollowUp(true);

    const apiMessages = [
      ...contextMessages
        .filter((m) => m.role === 'user' || m.role === 'assistant')
        .slice(-6)
        .map((m) => ({ role: m.role, content: m.content })),
      { role: 'user', content: `Explain this: "${selectedText}"` },
      { role: 'assistant', content: explanationRef.current },
      ...messages,
      { role: 'user', content: userMessage },
    ];

    try {
      let response = '';
      const unlistenPromise = listen<{ content: string; done: boolean }>('explain-stream', (e) => {
        if (e.payload.done) {
          setMessages((prev) => [...prev, { role: 'assistant', content: response }]);
          setIsLoadingFollowUp(false);
        } else {
          response += e.payload.content;
        }
      });

      invoke('explain_text', { messages: apiMessages, model }).catch((err: unknown) => {
        setMessages((prev) => [...prev, { role: 'assistant', content: `Error: ${err}` }]);
        setIsLoadingFollowUp(false);
      });

      await unlistenPromise.then((fn) => fn());
    } catch (err) {
      setMessages((prev) => [...prev, { role: 'assistant', content: `Error: ${err}` }]);
      setIsLoadingFollowUp(false);
    }
  }

  const formatted = formatAssistantText(explanation);

  return (
    <div
      className="explain-popup"
      style={{
        left: pos.x,
        top: pos.y,
        width: size.width,
        height: size.height,
      }}
    >
      {/* Header */}
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

        {messages.map((msg, idx) => (
          <div key={idx} className={`explain-message explain-message--${msg.role}`}>
            {msg.role === 'assistant' ? (
              formatAssistantText(msg.content).split('\n').map((line, i, arr) => (
                <Fragment key={i}>
                  {line}
                  {i < arr.length - 1 && <br />}
                </Fragment>
              ))
            ) : (
              msg.content
            )}
          </div>
        ))}

        {isLoadingFollowUp && <span className="explain-thinking">Thinking…</span>}
      </div>

      <div className="explain-footer">
        <input
          type="text"
          className="explain-input"
          placeholder="Ask a follow-up question…"
          value={followUpInput}
          onChange={(e) => setFollowUpInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              handleSendFollowUp();
            }
          }}
          disabled={!done || isLoadingFollowUp}
        />
        <button
          className="explain-send-btn"
          onClick={handleSendFollowUp}
          disabled={!done || !followUpInput.trim() || isLoadingFollowUp}
        >
          Send
        </button>
      </div>

      {/* Resize handles */}
      {/* Top */}
      <div
        className="explain-resize-handle explain-resize-handle--n"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'n',
          };
        }}
      />
      {/* Bottom */}
      <div
        className="explain-resize-handle explain-resize-handle--s"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 's',
          };
        }}
      />
      {/* Left */}
      <div
        className="explain-resize-handle explain-resize-handle--w"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'w',
          };
        }}
      />
      {/* Right */}
      <div
        className="explain-resize-handle explain-resize-handle--e"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'e',
          };
        }}
      />
      {/* Corners */}
      {/* Top-left */}
      <div
        className="explain-resize-handle explain-resize-handle--nw"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'nw',
          };
        }}
      />
      {/* Top-right */}
      <div
        className="explain-resize-handle explain-resize-handle--ne"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'ne',
          };
        }}
      />
      {/* Bottom-left */}
      <div
        className="explain-resize-handle explain-resize-handle--sw"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'sw',
          };
        }}
      />
      {/* Bottom-right */}
      <div
        className="explain-resize-handle explain-resize-handle--se"
        onMouseDown={(e) => {
          e.preventDefault();
          resizeOffset.current = {
            startX: e.clientX,
            startY: e.clientY,
            startWidth: size.width,
            startHeight: size.height,
            startPosX: pos.x,
            startPosY: pos.y,
            direction: 'se',
          };
        }}
      />
    </div>
  );
}
