import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Message, StreamPayload, AgentStatusPayload } from '../types';

interface UseOllamaChatReturn {
  messages: Message[];
  isThinking: boolean;
  agentStatus: string | null;
  error: string | null;
  handleSend: (text: string) => Promise<void>;
}

/**
 * Custom hook that manages the full Ollama agentic chat lifecycle:
 * - Sends messages via the `chat_ollama` Tauri command
 * - Subscribes to `ollama-stream` events and appends tokens to the last assistant message
 * - Subscribes to `agent-status` events for tool execution status
 * - Cleans up event listeners on unmount or when a new request starts
 */
export function useOllamaChat(model: string): UseOllamaChatReturn {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isThinking, setIsThinking] = useState(false);
  const [agentStatus, setAgentStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const unlistenStreamRef = useRef<UnlistenFn | null>(null);
  const unlistenStatusRef = useRef<UnlistenFn | null>(null);

  // Clean up listeners when the hook unmounts
  useEffect(() => {
    return () => {
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();
    };
  }, []);

  const handleSend = async (text: string) => {
    if (!text.trim() || isThinking) return;

    setError(null);
    setAgentStatus(null);

    const userMessage: Message = { role: 'user', content: text };
    const updatedMessages = [...messages, userMessage];

    setMessages([...updatedMessages, { role: 'assistant', content: '' }]);
    setIsThinking(true);

    try {
      // Tear down any previous listeners before starting new ones
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();

      // Listen for streamed tokens
      unlistenStreamRef.current = await listen<StreamPayload>('ollama-stream', (event) => {
        const { content, done } = event.payload;

        if (content) {
          setMessages((prev) => {
            const copy = [...prev];
            const last = copy[copy.length - 1];
            if (last?.role === 'assistant') {
              copy[copy.length - 1] = { ...last, content: last.content + content };
            }
            return copy;
          });
        }

        if (done) {
          setIsThinking(false);
          setAgentStatus(null);
          unlistenStreamRef.current?.();
          unlistenStatusRef.current?.();
        }
      });

      // Listen for agent tool-execution status updates
      unlistenStatusRef.current = await listen<AgentStatusPayload>('agent-status', (event) => {
        setAgentStatus(event.payload.message);
      });

      // Only pass user/assistant messages (not system) — Rust adds the system prompt
      const historyMessages = updatedMessages.filter(
        (m) => m.role === 'user' || m.role === 'assistant',
      );
      await invoke('chat_ollama', { messages: historyMessages, model });
    } catch (err) {
      setIsThinking(false);
      setAgentStatus(null);
      setError(String(err));
      // Remove empty assistant placeholder on failure
      setMessages((prev) => {
        const copy = [...prev];
        if (copy[copy.length - 1]?.content === '') copy.pop();
        return copy;
      });
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();
    }
  };

  return { messages, isThinking, agentStatus, error, handleSend };
}

