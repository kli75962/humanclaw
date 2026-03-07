import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Message, StreamPayload, AgentStatusPayload, UseOllamaChatReturn } from '../types';

/**
 * Manages the full Ollama agentic chat lifecycle.
 * Persists messages per chat ID; creates a new chat on first send when chatId is null.
 */
export function useOllamaChat(
  model: string,
  chatId: string | null,
  initialMessages: Message[],
  onChatCreated: (id: string, title: string) => void,
  onSave: (id: string, messages: Message[]) => void,
): UseOllamaChatReturn {
  const [messages, setMessages] = useState<Message[]>(initialMessages);
  const [isThinking, setIsThinking] = useState(false);
  const [agentStatus, setAgentStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const unlistenStreamRef = useRef<UnlistenFn | null>(null);
  const unlistenStatusRef = useRef<UnlistenFn | null>(null);

  // Ref that tracks the active chat ID inside async callbacks
  const currentChatIdRef = useRef<string | null>(chatId);
  // Mirror messages state for use in async callbacks without stale closure
  const messagesRef = useRef<Message[]>(initialMessages);
  // Flag: true when chatId prop change was triggered internally by this hook (first-send create)
  const internalCreateRef = useRef(false);
  // Keep callback refs fresh to avoid stale closures
  const onChatCreatedRef = useRef(onChatCreated);
  const onSaveRef = useRef(onSave);
  onChatCreatedRef.current = onChatCreated;
  onSaveRef.current = onSave;

  // Reset conversation when switching to a different chat (externally)
  useEffect(() => {
    if (internalCreateRef.current) {
      // ID change was caused by this hook creating a new chat — don't wipe messages
      internalCreateRef.current = false;
      currentChatIdRef.current = chatId;
      return;
    }
    unlistenStreamRef.current?.();
    unlistenStatusRef.current?.();
    currentChatIdRef.current = chatId;
    setMessages(initialMessages);
    messagesRef.current = initialMessages;
    setIsThinking(false);
    setError(null);
    setAgentStatus(null);
  }, [chatId]);

  useEffect(() => {
    return () => {
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();
    };
  }, []);

  const handleSend = async (text: string) => {
    if (!text.trim() || isThinking) return;

    // Create a new chat ID on first message of a new chat
    if (currentChatIdRef.current === null) {
      const newId = crypto.randomUUID();
      currentChatIdRef.current = newId;
      internalCreateRef.current = true;
      onChatCreatedRef.current(newId, text.slice(0, 50));
    }

    const activeChatId = currentChatIdRef.current!;

    setError(null);
    setAgentStatus(null);

    const userMessage: Message = { role: 'user', content: text };
    const updatedMessages = [...messagesRef.current, userMessage];
    const withPlaceholder = [...updatedMessages, { role: 'assistant' as const, content: '' }];

    setMessages(withPlaceholder);
    messagesRef.current = withPlaceholder;
    setIsThinking(true);

    try {
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();

      unlistenStreamRef.current = await listen<StreamPayload>('ollama-stream', (event) => {
        const { content, done } = event.payload;

        if (content) {
          setMessages((prev) => {
            const copy = [...prev];
            const last = copy[copy.length - 1];
            if (last?.role === 'assistant') {
              copy[copy.length - 1] = { ...last, content: last.content + content };
            }
            messagesRef.current = copy;
            return copy;
          });
        }

        if (done) {
          setIsThinking(false);
          setAgentStatus(null);
          onSaveRef.current(activeChatId, messagesRef.current);
          unlistenStreamRef.current?.();
          unlistenStatusRef.current?.();
        }
      });

      unlistenStatusRef.current = await listen<AgentStatusPayload>('agent-status', (event) => {
        setAgentStatus(event.payload.message);
      });

      const historyMessages = updatedMessages.filter(
        (m) => m.role === 'user' || m.role === 'assistant',
      );
      await invoke('chat_ollama', { messages: historyMessages, model });
    } catch (err) {
      setIsThinking(false);
      setAgentStatus(null);
      setError(String(err));
      setMessages((prev) => {
        const copy = [...prev];
        if (copy[copy.length - 1]?.content === '') copy.pop();
        messagesRef.current = copy;
        return copy;
      });
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();
    }
  };

  return { messages, isThinking, agentStatus, error, handleSend };
}

