import { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Message, StreamPayload, AgentStatusPayload, UseOllamaChatReturn } from '../types';

type CharacterOverride = { id?: string; name: string; persona: string; background: string };

/**
 * Manages the full Ollama agentic chat lifecycle.
 * Persists messages per chat ID; creates a new chat on first send when chatId is null.
 *
 * Listens for both local stream events (this device triggered the chat) and
 * remote events broadcast by a paired peer via SSE — events whose `chat_id`
 * matches the active chat are applied in real time so the two devices stay
 * mirrored while an LLM is talking on either side.
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
    currentChatIdRef.current = chatId;
    setMessages(initialMessages);
    messagesRef.current = initialMessages;
    setIsThinking(false);
    setError(null);
    setAgentStatus(null);
  }, [chatId]);

  // ── Global listeners (mounted once for the hook's lifetime) ───────────────
  // The listeners stay active across multiple `runChat` calls so that streams
  // broadcast by a paired peer (where this device isn't the one driving the
  // chat loop) still update the UI in real time.
  useEffect(() => {
    let cancelled = false;

    const handleStream = (payload: StreamPayload) => {
      const { content, done, brief, chat_id, remote } = payload;
      const targetChatId = chat_id ?? currentChatIdRef.current;
      // Ignore events for other chats (the active one is the only one we render).
      if (targetChatId !== currentChatIdRef.current) return;

      if (content) {
        setMessages((prev) => {
          const copy = [...prev];
          const last = copy[copy.length - 1];
          if (last?.role === 'assistant') {
            copy[copy.length - 1] = { ...last, content: last.content + content };
          } else {
            // Remote stream began without a local placeholder — append one.
            copy.push({ role: 'assistant', content });
          }
          messagesRef.current = copy;
          return copy;
        });
        if (remote) {
          // Mirror the "thinking" indicator while the peer is generating so
          // the UI doesn't look idle.
          setIsThinking(true);
        }
      }

      if (done) {
        setMessages((prev) => {
          const copy = [...prev];
          const last = copy[copy.length - 1];
          if (last?.role === 'assistant' && last.content === '') {
            copy[copy.length - 1] = { ...last, content: '(無回應)' };
          }
          if (brief) {
            for (let i = copy.length - 1; i >= 0; i--) {
              if (copy[i].role === 'assistant') {
                copy[i] = { ...copy[i], brief };
                break;
              }
            }
          }
          messagesRef.current = copy;
          // Only the originating device saves to disk; the peer relies on the
          // post-completion chat sync push to overwrite local state.
          if (!remote && targetChatId) {
            onSaveRef.current(targetChatId, copy);
          }
          return copy;
        });
        setIsThinking(false);
        setAgentStatus(null);
      }
    };

    const handleStatus = (payload: AgentStatusPayload) => {
      const targetChatId = payload.chat_id ?? currentChatIdRef.current;
      if (targetChatId !== currentChatIdRef.current) return;
      setAgentStatus(payload.message);
    };

    const handleInjected = (content: string) => {
      setMessages((prev) => {
        const copy = [...prev];
        const last = copy[copy.length - 1];
        if (last?.role === 'assistant' && last.content === '') {
          copy[copy.length - 1] = { role: 'assistant', content };
        } else {
          copy.push({ role: 'assistant', content });
        }
        copy.push({ role: 'assistant', content: '' });
        messagesRef.current = copy;
        return copy;
      });
    };

    (async () => {
      const stream = await listen<StreamPayload>('ollama-stream', (e) => handleStream(e.payload));
      const status = await listen<AgentStatusPayload>('agent-status', (e) => handleStatus(e.payload));
      const injected = await listen<{ content: string }>('ollama-injected-message',
        (e) => handleInjected(e.payload.content));
      if (cancelled) {
        stream(); status(); injected();
        return;
      }
      unlistenStreamRef.current = stream;
      unlistenStatusRef.current = status;
      unlistenInjectedRef.current = injected;
    })();

    return () => {
      cancelled = true;
      unlistenStreamRef.current?.();
      unlistenStatusRef.current?.();
      unlistenInjectedRef.current?.();
      unlistenStreamRef.current = null;
      unlistenStatusRef.current = null;
      unlistenInjectedRef.current = null;
    };
  }, []);

  // Shared stream runner — invokes chat_ollama / chat_claude.
  // Listener wiring lives in the mount-time effect above so peer-driven
  // streams continue to flow even when this device isn't the driver.
  const runChat = async (historyMessages: Message[], activeChatId: string) => {
    setError(null);
    setAgentStatus(null);
    setIsThinking(true);

    try {
      const provider = localStorage.getItem('phoneclaw_provider') ?? 'ollama';
      const command = provider === 'claude' ? 'chat_claude' : 'chat_ollama';
      await invoke(command, {
        chatId: activeChatId,
        messages: historyMessages,
        model,
        character: character ?? null,
      });
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

    // Persist user message immediately so it survives a failed LLM response.
    // On success, the done handler overwrites with the full conversation.
    onSaveRef.current(activeChatId, updatedMessages);

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
