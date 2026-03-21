import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useOllamaChat } from './hooks/useOllamaChat';
import { useStt } from './hooks/useStt';
import { TopBar } from './components/TopBar';
import { WelcomeScreen } from './components/WelcomeScreen';
import { ChatMessage } from './components/ChatMessage';
import { InputBar } from './components/InputBar';
import { SettingsScreen } from './components/SettingsScreen';
import { SideMenu } from './components/SideMenu';
import { AccessibilityDialog } from './components/AccessibilityDialog';
import type { ChatMeta, InputBarHandle, Message } from './types';

const DEFAULT_MODEL = 'kimi-k2.5:cloud';
const MODEL_STORAGE_KEY = 'phoneclaw_model';

function App() {
  const [model, setModel] = useState(
    () => localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL
  );
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [showMenu, setShowMenu] = useState(false);
  const [ollamaEndpointRevision, setOllamaEndpointRevision] = useState(0);

  // Chat management
  const [chatMetas, setChatMetas] = useState<ChatMeta[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [initMessages, setInitMessages] = useState<Message[]>([]);

  // InputBar imperative handle — lets us read/write input without owning the state
  const inputBarRef = useRef<InputBarHandle>(null);

  // Load chat list on mount
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

    return () => {
      unlisten.then((fn) => fn());
    };
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

  // STT: save input text before recording starts so transcript appends after it
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
    setShowMenu(false);
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
    setShowMenu(false);
  }, []);

  // Use ref to avoid recreating callback when activeChatId changes
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

  // Fetch available Ollama models on first load and when endpoint changes.
  useEffect(() => {
    invoke<{ name: string }[]>('list_models')
      .then((models) => {
        const names = models.map((m) => m.name);
        setAvailableModels(names);
        // Auto-select first model if saved/default model is not available
        const saved = localStorage.getItem(MODEL_STORAGE_KEY);
        if (names.length > 0 && saved && !names.includes(saved)) {
          setModel(names[0]);
          localStorage.setItem(MODEL_STORAGE_KEY, names[0]);
        } else if (names.length > 0 && !saved && !names.includes(DEFAULT_MODEL)) {
          setModel(names[0]);
          localStorage.setItem(MODEL_STORAGE_KEY, names[0]);
        }
      })
      .catch(() => {
        // Ollama unreachable — keep default, user will see error on send
      });
  }, [ollamaEndpointRevision, showSettings]);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, agentStatus]);

  const onSend = useCallback((text: string) => {
    if (isListening) stopListening();
    handleSend(text);
  }, [handleSend, isListening, stopListening]);

  const handleMenuOpen = useCallback(() => setShowMenu((v) => !v), []);
  const handleMenuClose = useCallback(() => setShowMenu(false), []);
  const handleSettingsOpen = useCallback(() => setShowSettings(true), []);
  const handleSettingsBack = useCallback(() => setShowSettings(false), []);
  const handleOllamaEndpointChanged = useCallback(() => {
    setOllamaEndpointRevision((v) => v + 1);
  }, []);

  // Memoize the message list to avoid O(n) lastUserMsgIdx computation on every render
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

  if (showSettings) {
    return (
      <SettingsScreen
        model={model}
        availableModels={availableModels}
        onModelChange={handleModelChange}
        onOllamaEndpointChanged={handleOllamaEndpointChanged}
        onBack={handleSettingsBack}
      />
    );
  }

  return (
    <div className="flex flex-col h-screen bg-[#131314] text-[#E3E3E3] font-sans">
      <AccessibilityDialog />
      <SideMenu
        open={showMenu}
        onClose={handleMenuClose}
        onNewChat={startNewChat}
        chats={chatMetas}
        activeChatId={activeChatId}
        onSelectChat={switchChat}
        onDeleteChat={deleteChat}
      />
      <TopBar model={model} onMenuOpen={handleMenuOpen} onSettingsOpen={handleSettingsOpen} />

      {/* Main Content Area */}
      <div className="flex-1 min-h-0 overflow-y-auto px-4 custom-scrollbar">
        {messages.length === 0 ? (
          <WelcomeScreen onSend={onSend} />
        ) : (
          <div className="max-w-3xl mx-auto space-y-8 mt-4">
            {messageList}

          {/* Agent tool-execution status */}
          {agentStatus && (
            <div className="flex items-center gap-2 text-xs text-blue-400 animate-pulse px-1">
              <span className="w-1.5 h-1.5 rounded-full bg-blue-400 inline-block" />
              {agentStatus}
            </div>
          )}

          {error && (
            <div className="bg-red-900/30 border border-red-700 text-red-300 text-sm px-4 py-3 rounded-xl">
              {error}
            </div>
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
  );
}

export default App;
