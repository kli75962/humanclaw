import { useState, useRef, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOllamaChat } from './hooks/useOllamaChat';
import { useStt } from './hooks/useStt';
import { TopBar } from './components/TopBar';
import { WelcomeScreen } from './components/WelcomeScreen';
import { ChatMessage } from './components/ChatMessage';
import { InputBar } from './components/InputBar';
import { SettingsScreen } from './components/SettingsScreen';
import { SideMenu } from './components/SideMenu';
import { AccessibilityDialog } from './components/AccessibilityDialog';
import type { ChatMeta, Message } from './types';

const DEFAULT_MODEL = 'kimi-k2.5:cloud';
const MODEL_STORAGE_KEY = 'phoneclaw_model';

function App() {
  const [input, setInput] = useState('');
  const [model, setModel] = useState(
    () => localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL
  );
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [showMenu, setShowMenu] = useState(false);

  // Chat management
  const [chatMetas, setChatMetas] = useState<ChatMeta[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [initMessages, setInitMessages] = useState<Message[]>([]);

  // Load chat list on mount
  useEffect(() => {
    invoke<ChatMeta[]>('list_chats').then(setChatMetas).catch(() => {});
  }, []);

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

  const { messages, isThinking, agentStatus, error, handleSend } = useOllamaChat(
    model, activeChatId, initMessages, onChatCreated, onSave,
  );
  const scrollRef = useRef<HTMLDivElement>(null);

  // STT: save input text before recording starts so transcript appends after it
  const sttPrefixRef = useRef('');
  const inputRef = useRef(input);
  inputRef.current = input;

  const handleSttTranscript = useCallback((text: string) => {
    const prefix = sttPrefixRef.current;
    setInput(prefix ? `${prefix} ${text}` : text);
  }, []);

  const { isListening, sttError, startListening, stopListening } = useStt(handleSttTranscript);

  const handleSttToggle = useCallback(() => {
    if (isListening) {
      stopListening();
    } else {
      sttPrefixRef.current = inputRef.current;
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

  const deleteChat = useCallback((id: string) => {
    invoke('delete_chat', { id }).catch(() => {});
    setChatMetas((prev) => prev.filter((m) => m.id !== id));
    if (activeChatId === id) {
      setActiveChatId(null);
      setInitMessages([]);
    }
  }, [activeChatId]);

  // Fetch available Ollama models on first load
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
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, agentStatus]);

  const onSend = useCallback((text: string) => {
    if (isListening) stopListening();
    handleSend(text);
    setInput('');
  }, [handleSend, isListening, stopListening]);

  const handleMenuOpen = useCallback(() => setShowMenu((v) => !v), []);
  const handleMenuClose = useCallback(() => setShowMenu(false), []);
  const handleSettingsOpen = useCallback(() => setShowSettings(true), []);
  const handleSettingsBack = useCallback(() => setShowSettings(false), []);

  if (showSettings) {
    return (
      <SettingsScreen
        model={model}
        availableModels={availableModels}
        onModelChange={handleModelChange}
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
            {messages.map((msg, idx) => (
              <ChatMessage
                key={idx}
                message={msg}
                isLastMessage={idx === messages.length - 1}
                isThinking={isThinking}
              />
            ))}

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
        value={input}
        isThinking={isThinking}
        isListening={isListening}
        sttError={sttError}
        onChange={setInput}
        onSend={onSend}
        onSttToggle={handleSttToggle}
      />
    </div>
  );
}

export default App;

