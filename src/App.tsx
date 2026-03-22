import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useOllamaChat } from './hooks/useOllamaChat';
import { useStt } from './hooks/useStt';
import { WelcomeScreen } from './components/WelcomeScreen';
import { ChatMessage } from './components/ChatMessage';
import { InputBar } from './components/InputBar';
import { SideMenu } from './components/SideMenu';
import { AccessibilityDialog } from './components/AccessibilityDialog';
import { Menu, Settings } from 'lucide-react';
import type { ChatMeta, InputBarHandle, Message } from './types';
import './style/themes.css';
import './style/App.css';

const DEFAULT_MODEL = 'kimi-k2.5:cloud';
const MODEL_STORAGE_KEY = 'phoneclaw_model';
const SIDE_WIDTH_KEY = 'phoneclaw_side_width';
const MIN_SIDE = 200;
const MAX_SIDE_RATIO = 0.6;

function App() {
  const [model, setModel] = useState(
    () => localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL
  );
  const [sideView, setSideView] = useState<'history' | 'settings'>('history');
  const [sideOpen, setSideOpen] = useState(false);

  const handleSwitchView = useCallback((v: 'history' | 'settings') => {
    setSideView(v);
    setSideOpen(true);
  }, []);
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

  // Chat management
  const [chatMetas, setChatMetas] = useState<ChatMeta[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [initMessages, setInitMessages] = useState<Message[]>([]);

  const inputBarRef = useRef<InputBarHandle>(null);

  useEffect(() => {
    invoke<ChatMeta[]>('list_chats').then(setChatMetas).catch(() => {});
  }, []);

  useEffect(() => {
    const unlisten = listen('chat-sync-updated', async () => {
      const metas = await invoke<ChatMeta[]>('list_chats').catch(() => []);
      setChatMetas(metas);

      if (!activeChatId) return;
      const msgs = await invoke<Message[]>('load_chat_messages', { id: activeChatId }).catch(() => []);
      setInitMessages(msgs);
    });

    return () => { unlisten.then((fn) => fn()); };
  }, [activeChatId]);

  const onChatCreated = useCallback((id: string, title: string) => {
    const createdAt = new Date().toISOString();
    invoke('create_chat', { id, title, createdAt }).catch(() => {});
    const meta: ChatMeta = { id, title, createdAt };
    setActiveChatId(id);
    setChatMetas((prev) => [meta, ...prev]);
  }, []);

  const onSave = useCallback((id: string, messages: Message[]) => {
    invoke('save_chat_messages', { id, messages }).catch(() => {});
  }, []);

  const { messages, isThinking, agentStatus, error, handleSend, handleRetry, handleStop } = useOllamaChat(
    model, activeChatId, initMessages, onChatCreated, onSave,
  );
  const scrollRef = useRef<HTMLDivElement>(null);

  const sttPrefixRef = useRef('');

  const handleSttTranscript = useCallback((text: string) => {
    const prefix = sttPrefixRef.current;
    inputBarRef.current?.setInput(prefix ? `${prefix} ${text}` : text);
  }, []);

  const { isListening, sttError, startListening, stopListening } = useStt(handleSttTranscript);

  const handleSttToggle = useCallback(() => {
    if (isListening) {
      stopListening();
    } else {
      sttPrefixRef.current = inputBarRef.current?.getInput() ?? '';
      startListening();
    }
  }, [isListening, startListening, stopListening]);

  const handleModelChange = useCallback((m: string) => {
    setModel(m);
    localStorage.setItem(MODEL_STORAGE_KEY, m);
  }, []);

  const startNewChat = useCallback(() => {
    setActiveChatId(null);
    setInitMessages([]);
  }, []);

  const switchChat = useCallback((id: string) => {
    invoke<Message[]>('load_chat_messages', { id })
      .then((msgs) => {
        setActiveChatId(id);
        setInitMessages(msgs);
      })
      .catch(() => {
        setActiveChatId(id);
        setInitMessages([]);
      });
  }, []);

  const activeChatIdRef = useRef(activeChatId);
  activeChatIdRef.current = activeChatId;

  const deleteChat = useCallback((id: string) => {
    invoke('delete_chat', { id }).catch(() => {});
    setChatMetas((prev) => prev.filter((m) => m.id !== id));
    if (activeChatIdRef.current === id) {
      setActiveChatId(null);
      setInitMessages([]);
    }
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, agentStatus]);

  const onSend = useCallback((text: string) => {
    if (isListening) stopListening();
    handleSend(text);
  }, [handleSend, isListening, stopListening]);

  const handleOllamaEndpointChanged = useCallback(() => {}, []);

  const messageList = useMemo(() => {
    const lastUserMsgIdx = messages.reduce(
      (acc, m, i) => (m.role === 'user' ? i : acc),
      -1,
    );
    return messages.map((msg, idx) => (
      <ChatMessage
        key={idx}
        message={msg}
        isLastMessage={idx === messages.length - 1}
        isThinking={isThinking}
        onRetry={idx === lastUserMsgIdx && !isThinking ? handleRetry : undefined}
      />
    ));
  }, [messages, isThinking, handleRetry]);

  return (
    <div className={`app-root${sideOpen ? ' side-open' : ''}`}>
      <AccessibilityDialog />

      {/* ── Mobile nav overlay (phone only, shown when side panel is closed) ── */}
      <div className="mobile-nav">
        <button
          className={`top-nav-btn${sideView === 'settings' ? ' top-nav-btn--active' : ''}`}
          onClick={() => handleSwitchView('settings')}
          aria-label="Settings"
        >
          <Settings size={22} />
        </button>
        <button
          className={`top-nav-btn${sideView === 'history' ? ' top-nav-btn--active' : ''}`}
          onClick={() => handleSwitchView('history')}
          aria-label="Chat history"
        >
          <Menu size={22} />
        </button>
      </div>

      {/* ── Left panel ── */}
      <div className="app-left" style={{ width: sideWidth }}>
        <SideMenu
          view={sideView}
          onSwitchView={handleSwitchView}
          onNewChat={startNewChat}
          chats={chatMetas}
          activeChatId={activeChatId}
          onSelectChat={switchChat}
          onDeleteChat={deleteChat}
          model={model}
          onModelChange={handleModelChange}
          onOllamaEndpointChanged={handleOllamaEndpointChanged}
          isMobileOpen={sideOpen}
          onCloseSide={() => setSideOpen(false)}
        />
      </div>

      {/* ── Draggable divider ── */}
      <div
        className="app-divider"
        onPointerDown={handleDividerPointerDown}
        onPointerMove={handleDividerPointerMove}
        onPointerUp={handleDividerPointerUp}
        onPointerCancel={handleDividerPointerUp}
      />

      {/* ── Right: chat ── */}
      <div className="app-right">
        <div className="app-content custom-scrollbar">
          {messages.length === 0 ? (
            <WelcomeScreen onSend={onSend} />
          ) : (
            <div className="app-messages">
              {messageList}

              {agentStatus && (
                <div className="app-agent-status">
                  <span className="app-agent-dot" />
                  {agentStatus}
                </div>
              )}

              {error && (
                <div className="app-error">{error}</div>
              )}

              <div ref={scrollRef} />
            </div>
          )}
        </div>

        <InputBar
          ref={inputBarRef}
          isThinking={isThinking}
          isListening={isListening}
          sttError={sttError}
          onSend={onSend}
          onSttToggle={handleSttToggle}
          onStop={handleStop}
        />
      </div>
    </div>
  );
}

export default App;
