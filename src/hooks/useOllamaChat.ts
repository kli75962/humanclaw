import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Message, StreamPayload, AgentStatusPayload, UseOllamaChatReturn } from '../types';

type CharacterOverride = { id?: string; name: string; persona: string; background: string };

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
  character?: CharacterOverride | null,
): UseOllamaChatReturn {
  const [messages, setMessages] = useState<Message[]>(initialMessages);
  const [isThinking, setIsThinking] = useState(false);
  const [agentStatus, setAgentStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const unlistenStreamRef = useRef<UnlistenFn | null>(null);
  const unlistenStatusRef = useRef<UnlistenFn | null>(null);
  const unlistenInjectedRef = useRef<UnlistenFn | null>(null);

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
      internalCreateRef.current = false;
      currentChatIdRef.current = chatId;
      return;
    }
    unlistenStreamRef.current?.();
    unlistenStatusRef.current?.();
    unlistenInjectedRef.current?.();
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
      unlistenInjectedRef.current?.();
    };
  }, []);

  // Shared stream runner — sets up listeners and invokes chat_ollama.
  // Caller is responsible for updating messages state + messagesRef before calling.
  const runChat = async (historyMessages: Message[], activeChatId: string) => {
    setError(null);
    setAgentStatus(null);
    setIsThinking(true);

    try {
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();
      unlistenInjectedRef.current?.();

      unlistenStreamRef.current = await listen<StreamPayload>('ollama-stream', (event) => {
        const { content, done, brief } = event.payload;

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
          // Remove trailing empty assistant placeholder if LLM ended via send_message only
          setMessages((prev) => {
            const copy = [...prev];
            if (copy[copy.length - 1]?.role === 'assistant' && copy[copy.length - 1].content === '') {
              copy.pop();
            }
            // Attach brief to the last assistant message for history compression
            if (brief) {
              for (let i = copy.length - 1; i >= 0; i--) {
                if (copy[i].role === 'assistant') {
                  copy[i] = { ...copy[i], brief };
                  break;
                }
              }
            }
            messagesRef.current = copy;
            onSaveRef.current(activeChatId, copy);
            return copy;
          });
          setIsThinking(false);
          setAgentStatus(null);
          unlistenStreamRef.current?.();
          unlistenStatusRef.current?.();
          unlistenInjectedRef.current?.();
        }
      });

      unlistenStatusRef.current = await listen<AgentStatusPayload>('agent-status', (event) => {
        setAgentStatus(event.payload.message);
      });

      // Listen for messages injected mid-loop via the send_message tool.
      // Each injected message replaces the current empty placeholder and adds a new one.
      unlistenInjectedRef.current = await listen<{ content: string }>('ollama-injected-message', (event) => {
        const { content } = event.payload;
        setMessages((prev) => {
          const copy = [...prev];
          const last = copy[copy.length - 1];
          // Fill the current placeholder (or append if last isn't an empty placeholder)
          if (last?.role === 'assistant' && last.content === '') {
            copy[copy.length - 1] = { role: 'assistant', content };
          } else {
            copy.push({ role: 'assistant', content });
          }
          // Add a fresh placeholder for any subsequent streaming or injected messages
          copy.push({ role: 'assistant', content: '' });
          messagesRef.current = copy;
          return copy;
        });
      });

      const provider = localStorage.getItem('phoneclaw_provider') ?? 'ollama';
      const command = provider === 'claude' ? 'chat_claude' : 'chat_ollama';
      await invoke(command, { messages: historyMessages, model, character: character ?? null });
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
      unlistenInjectedRef.current?.();
    }
  };

  const handleSend = async (text: string) => {
    if (!text.trim() || isThinking) return;

    if (currentChatIdRef.current === null) {
      const newId = crypto.randomUUID();
      currentChatIdRef.current = newId;
      internalCreateRef.current = true;
      const titleText = text.replace(/<file name="[^"]*">[\s\S]*?<\/file>/g, '').trim();
      onChatCreatedRef.current(newId, titleText.slice(0, 50));
    }

    const activeChatId = currentChatIdRef.current!;

    const userMessage: Message = { role: 'user', content: text };
    const updatedMessages = [...messagesRef.current, userMessage];
    const withPlaceholder = [...updatedMessages, { role: 'assistant' as const, content: '' }];

    setMessages(withPlaceholder);
    messagesRef.current = withPlaceholder;

    const historyMessages: Message[] = updatedMessages.filter(
      (m) => m.role === 'user' || m.role === 'assistant',
    );
    await runChat(historyMessages, activeChatId);
  };

  const handleRetry = async () => {
    if (isThinking) return;

    const activeChatId = currentChatIdRef.current;
    if (!activeChatId) return;

    const lastUserIdx = messagesRef.current.reduce(
      (acc, m, i) => (m.role === 'user' ? i : acc),
      -1,
    );
    if (lastUserIdx === -1) return;

    const historyUpToUser = messagesRef.current.slice(0, lastUserIdx + 1);
    const withPlaceholder = [...historyUpToUser, { role: 'assistant' as const, content: '' }];

    setMessages(withPlaceholder);
    messagesRef.current = withPlaceholder;

    const historyMessages: Message[] = historyUpToUser.filter(
      (m) => m.role === 'user' || m.role === 'assistant',
    );
    await runChat(historyMessages, activeChatId);
  };

  const handleStop = () => {
    setIsThinking(false);
    setAgentStatus(null);
    invoke('cancel_chat').catch(() => {});
  };

  return { messages, isThinking, agentStatus, error, handleSend, handleRetry, handleStop };
}
