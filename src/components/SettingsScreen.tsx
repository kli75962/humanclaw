import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft, BookOpen, Brain, ChevronDown, Check, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';

interface Memory {
  id: string;
  content: string;
  created_at: number;
}

/** General phone navigation knowledge shared across all users. */
interface NavKnowledge {
  id: string;
  content: string;
  created_at: number;
}

interface SettingsScreenProps {
  model: string;
  availableModels: string[];
  onModelChange: (model: string) => void;
  onBack: () => void;
}

/** Full-screen settings page styled like a native mobile settings app. */
export function SettingsScreen({ model, availableModels, onModelChange, onBack }: SettingsScreenProps) {
  const [modelOpen, setModelOpen] = useState(false);
  const [memories, setMemories] = useState<Memory[]>([]);
  const [knowledge, setKnowledge] = useState<NavKnowledge[]>([]);

  // Load memories and navigation knowledge when the settings screen opens
  useEffect(() => {
    invoke<Memory[]>('get_memories')
      .then(setMemories)
      .catch(() => setMemories([]));
    invoke<NavKnowledge[]>('get_knowledge')
      .then(setKnowledge)
      .catch(() => setKnowledge([]));
  }, []);

  async function deleteMemory(id: string) {
    await invoke('delete_memory_cmd', { id });
    setMemories((prev) => prev.filter((m) => m.id !== id));
  }

  async function clearAllMemories() {
    await invoke('clear_memories_cmd');
    setMemories([]);
  }

  async function deleteKnowledge(id: string) {
    await invoke('delete_knowledge_cmd', { id });
    setKnowledge((prev) => prev.filter((k) => k.id !== id));
  }

  async function clearAllKnowledge() {
    await invoke('clear_knowledge_cmd');
    setKnowledge([]);
  }

  return (
    <div className="flex flex-col h-screen bg-[#131314] text-[#E3E3E3]">
      {/* Header */}
      <div className="flex items-center gap-3 px-2 py-3 border-b border-[#2C2C2C]">
        <button
          onClick={onBack}
          className="p-2 hover:bg-[#2C2C2C] rounded-full transition-colors"
        >
          <ArrowLeft size={22} className="text-gray-400" />
        </button>
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 overflow-y-auto">

        {/* Section: Model */}
        <div className="mt-6">
          <p className="px-5 pb-2 text-xs font-semibold text-gray-500 uppercase tracking-widest">
            Model
          </p>

          <div className="bg-[#1E1F20] border-y border-[#2C2C2C]">
            {/* Row: current model — tap to open dropdown */}
            <button
              onClick={() => setModelOpen((v) => !v)}
              className="w-full flex items-center justify-between px-5 py-4 text-sm hover:bg-[#252526] transition-colors"
            >
              <span className="text-gray-300">Active model</span>
              <span className="flex items-center gap-2 text-gray-400 font-mono text-xs">
                <span className="truncate max-w-[160px]">{model || 'None'}</span>
                <ChevronDown
                  size={15}
                  className={`shrink-0 transition-transform ${modelOpen ? 'rotate-180' : ''}`}
                />
              </span>
            </button>

            {/* Expanded model list */}
            {modelOpen && (
              <>
                <div className="border-t border-[#2C2C2C]" />
                {availableModels.length === 0 ? (
                  <p className="px-5 py-4 text-sm text-gray-500">
                    No models found — is Ollama running?
                  </p>
                ) : (
                  availableModels.map((m, i) => (
                    <button
                      key={m}
                      onClick={() => { onModelChange(m); setModelOpen(false); }}
                      className={`w-full flex items-center justify-between px-5 py-3.5 text-sm font-mono transition-colors hover:bg-[#252526] ${
                        i < availableModels.length - 1 ? 'border-b border-[#2C2C2C]' : ''
                      }`}
                    >
                      <span className={m === model ? 'text-purple-400' : 'text-gray-300'}>
                        {m}
                      </span>
                      {m === model && <Check size={14} className="text-purple-400 shrink-0" />}
                    </button>
                  ))
                )}
              </>
            )}
          </div>

          <p className="px-5 pt-2 text-xs text-gray-600">
            Models are loaded from your local Ollama instance.
          </p>
        </div>

        {/* Section: Navigation Knowledge */}
        <div className="mt-6">
          <div className="flex items-center justify-between px-5 pb-2">
            <p className="text-xs font-semibold text-gray-500 uppercase tracking-widest">
              Navigation Knowledge
            </p>
            {knowledge.length > 0 && (
              <button
                onClick={clearAllKnowledge}
                className="flex items-center gap-1 text-xs text-red-400 hover:text-red-300 transition-colors"
              >
                <Trash2 size={12} />
                Clear all
              </button>
            )}
          </div>

          <div className="bg-[#1E1F20] border-y border-[#2C2C2C]">
            {knowledge.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-8 text-gray-600">
                <BookOpen size={24} />
                <p className="text-sm">No navigation knowledge saved yet</p>
              </div>
            ) : (
              knowledge.map((kn, i) => (
                <div
                  key={kn.id}
                  className={`flex items-start justify-between gap-3 px-5 py-3.5 ${
                    i < knowledge.length - 1 ? 'border-b border-[#2C2C2C]' : ''
                  }`}
                >
                  <p className="text-sm text-gray-300 flex-1 leading-snug font-mono text-xs">{kn.content}</p>
                  <button
                    onClick={() => deleteKnowledge(kn.id)}
                    className="mt-0.5 shrink-0 text-gray-600 hover:text-red-400 transition-colors"
                    aria-label="Delete knowledge"
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))
            )}
          </div>

          <p className="px-5 pt-2 text-xs text-gray-600">
            UI navigation paths learned automatically — shared across all users.
          </p>
        </div>

        {/* Section: Memory */}
        <div className="mt-6 mb-8">
          <div className="flex items-center justify-between px-5 pb-2">
            <p className="text-xs font-semibold text-gray-500 uppercase tracking-widest">
              Memory
            </p>
            {memories.length > 0 && (
              <button
                onClick={clearAllMemories}
                className="flex items-center gap-1 text-xs text-red-400 hover:text-red-300 transition-colors"
              >
                <Trash2 size={12} />
                Clear all
              </button>
            )}
          </div>

          <div className="bg-[#1E1F20] border-y border-[#2C2C2C]">
            {memories.length === 0 ? (
              <div className="flex flex-col items-center gap-2 py-8 text-gray-600">
                <Brain size={24} />
                <p className="text-sm">No memories saved yet</p>
              </div>
            ) : (
              memories.map((mem, i) => (
                <div
                  key={mem.id}
                  className={`flex items-start justify-between gap-3 px-5 py-3.5 ${
                    i < memories.length - 1 ? 'border-b border-[#2C2C2C]' : ''
                  }`}
                >
                  <p className="text-sm text-gray-300 flex-1 leading-snug">{mem.content}</p>
                  <button
                    onClick={() => deleteMemory(mem.id)}
                    className="mt-0.5 shrink-0 text-gray-600 hover:text-red-400 transition-colors"
                    aria-label="Delete memory"
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              ))
            )}
          </div>

          <p className="px-5 pt-2 text-xs text-gray-600">
            Preferences extracted automatically from your conversations.
          </p>
        </div>

      </div>
    </div>
  );
}
