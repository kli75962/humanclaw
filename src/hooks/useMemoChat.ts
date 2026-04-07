import { useState, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Message, MemoMeta } from '../types';

export function useMemoChat(model: string, setMainTab: (tab: 'chat'|'posts') => void, setSideOpen: (open: boolean) => void, setSideView: (v: 'history'|'settings'|'connect'|'memos') => void) {
  const [activeMemoId, setActiveMemoId] = useState<string | null>(null);
  const [memoMessages, setMemoMessages] = useState<Message[]>([]);
  const [memoStreaming, setMemoStreaming] = useState(false);
  const [memoStreamContent, setMemoStreamContent] = useState('');
  const memoStreamBuf = useRef('');
  const activeMemoIdRef = useRef<string | null>(null);
  activeMemoIdRef.current = activeMemoId;

  const handleSelectMemo = useCallback(async (id: string) => {
    const msgs = await invoke<Message[]>('load_memo_messages', { id }).catch(() => []);
    setActiveMemoId(id);
    setMemoMessages(msgs);
    setMainTab('chat');
    setSideOpen(false);
  }, [setMainTab, setSideOpen]);

  const handleSaveMemo = useCallback(async (title: string, msgs: Message[]): Promise<string | null> => {
    const meta = await invoke<MemoMeta>('create_memo', { title, messages: msgs }).catch(() => null);
    document.dispatchEvent(new Event('memo-saved'));
    return meta?.id ?? null;
  }, []);

  const handleOpenMemo = useCallback((id: string) => {
    invoke<Message[]>('load_memo_messages', { id }).then((msgs) => {
      setActiveMemoId(id);
      setMemoMessages(msgs);
      setMainTab('chat');
      setSideView('memos');
    }).catch(() => {});
  }, [setMainTab, setSideView]);

  const handleMemoSend = useCallback(async (text: string) => {
    if (memoStreaming) return;
    const userMsg: Message = { role: 'user', content: text };
    const updatedMsgs = [...memoMessages, userMsg];
    setMemoMessages(updatedMsgs);
    setMemoStreaming(true);
    setMemoStreamContent('');
    memoStreamBuf.current = '';

    let unlistenFn: (() => void) | null = null;
    const unlistenPromise = listen<{ content: string; done: boolean }>('explain-stream', (e) => {
      if (e.payload.done) {
        const finalContent = memoStreamBuf.current;
        const assistantMsg: Message = { role: 'assistant', content: finalContent };
        const finalMsgs = [...updatedMsgs, assistantMsg];
        setMemoMessages(finalMsgs);
        setMemoStreaming(false);
        setMemoStreamContent('');
        memoStreamBuf.current = '';
        const id = activeMemoIdRef.current;
        if (id) invoke('save_memo_messages', { id, messages: finalMsgs }).catch(() => {});
        unlistenFn?.();
      } else {
        memoStreamBuf.current += e.payload.content;
        setMemoStreamContent(memoStreamBuf.current);
      }
    });
    unlistenPromise.then((fn) => { unlistenFn = fn; });

    const apiMsgs = updatedMsgs.map((m) => ({ role: m.role, content: m.content }));
    invoke('explain_text', { messages: apiMsgs, model }).catch((err: unknown) => {
      setMemoMessages((prev) => [...prev, { role: 'assistant', content: `Error: ${err}` }]);
      setMemoStreaming(false);
      unlistenPromise.then((fn) => fn());
    });
  }, [memoStreaming, memoMessages, model]);

  return {
    activeMemoId,
    setActiveMemoId,
    memoMessages,
    memoStreaming,
    memoStreamContent,
    handleSelectMemo,
    handleSaveMemo,
    handleOpenMemo,
    handleMemoSend
  };
}
