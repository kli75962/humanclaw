import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOllamaChat } from './hooks/useOllamaChat';
import { TopBar } from './components/TopBar';
import { WelcomeScreen } from './components/WelcomeScreen';
import { ChatMessage } from './components/ChatMessage';
import { InputBar } from './components/InputBar';
import { SettingsScreen } from './components/SettingsScreen';

const DEFAULT_MODEL = 'kimi-k2.5:cloud';

function App() {
  const [input, setInput] = useState('');
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const { messages, isThinking, agentStatus, error, handleSend } = useOllamaChat(model);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Fetch available Ollama models on first load
  useEffect(() => {
    invoke<{ name: string }[]>('list_models')
      .then((models) => {
        const names = models.map((m) => m.name);
        setAvailableModels(names);
        // Auto-select first model if default is not available
        if (names.length > 0 && !names.includes(DEFAULT_MODEL)) {
          setModel(names[0]);
        }
      })
      .catch(() => {
        // Ollama unreachable — keep default, user will see error on send
      });
  }, []);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, agentStatus]);

  const onSend = (text: string) => {
    handleSend(text);
    setInput('');
  };

  if (showSettings) {
    return (
      <SettingsScreen
        model={model}
        availableModels={availableModels}
        onModelChange={setModel}
        onBack={() => setShowSettings(false)}
      />
    );
  }

  return (
    <div className="flex flex-col h-screen bg-[#131314] text-[#E3E3E3] font-sans">
      <TopBar model={model} onSettingsOpen={() => setShowSettings(true)} />

      {/* Main Content Area */}
      <div className="flex-1 min-h-0 overflow-y-auto px-4 custom-scrollbar">
        {messages.length === 0 && <WelcomeScreen onSend={onSend} />}

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
      </div>

      <InputBar
        value={input}
        isThinking={isThinking}
        onChange={setInput}
        onSend={onSend}
      />
    </div>
  );
}

export default App;

