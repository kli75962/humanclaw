import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useOllamaChat } from './hooks/useOllamaChat';
import { useCharacters } from './hooks/useCharacters';
import { usePosts } from './hooks/usePosts';
import { usePostGeneration } from './hooks/usePostGeneration';
import { useStt } from './hooks/useStt';
import { useMemoChat } from './hooks/useMemoChat';
import { AppLayout } from './components/layout/AppLayout';
import { AppChat } from './components/chat/AppChat';
import { AppSocial } from './components/social/AppSocial';
import type { PersonaBuildNoticeStatus } from './components/persona/PersonaBuildNotice';
import type { PermissionRequest as PermissionRequestData } from './components/ui/PermissionDialog';
import type { AskQuestion } from './components/ui/AskUserBubble';
import type { ChatMeta, Message, Post } from './types';
import './style/themes.css';
import './style/App.css';

const DEFAULT_MODEL = 'kimi-k2.5:cloud';
const MODEL_STORAGE_KEY = 'phoneclaw_model';
const CHAT_MODE_KEY = 'phoneclaw_chat_mode';
const IG_MODE_KEY = 'phoneclaw_ig_mode';

function App() {
  const [model, setModel] = useState(() => localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL);
  const [chatMode, setChatMode] = useState(() => localStorage.getItem(CHAT_MODE_KEY) === 'true');
  const [igMode, setIgMode] = useState(() => localStorage.getItem(IG_MODE_KEY) === 'true');
  const [activeCharacterId, setActiveCharacterId] = useState<string | null>(null);
  
  const { characters, addCharacter, deleteCharacter } = useCharacters();
  const { posts, likedPostIds, toggleLike, deletePost, addPost, refresh: refreshPosts } = usePosts();
  
  const [quotedPost, setQuotedPost] = useState<Post | null>(null);
  const [permRequest, setPermRequest] = useState<PermissionRequestData | null>(null);
  const [askUserRequest, setAskUserRequest] = useState<{ id: string; questions: AskQuestion[] } | null>(null);
  
  const [personaNotice, setPersonaNotice] = useState<{ status: PersonaBuildNoticeStatus; displayName: string } | null>(null);
  const [mainTab, setMainTab] = useState<'chat' | 'posts'>('chat');
  const [sideView, setSideView] = useState<'history' | 'settings' | 'connect' | 'memos'>('history');
  const [sideOpen, setSideOpen] = useState(false);

  // Initialize hooks that span features
  const {
    activeMemoId, memoMessages, memoStreaming, memoStreamContent,
    handleSelectMemo, handleSaveMemo, handleOpenMemo, handleMemoSend, setActiveMemoId
  } = useMemoChat(model, setMainTab, setSideOpen, setSideView);

  // Resume persona build status
  useEffect(() => {
    invoke<{ status: string; displayName: string; model: string; sex: string; ageRange: string; vibe: string; world: string; connectsBy: string; personaName: string; } | null>('get_persona_build_status')
      .then((saved) => {
        if (!saved) return;
        if (saved.status === 'creating') {
          setPersonaNotice({ status: 'creating', displayName: saved.displayName });
          invoke('create_persona_background', saved)
            .then(() => {
              invoke<{ displayName: string } | null>('get_persona_build_status').then((s) => {
                setPersonaNotice({ status: 'done', displayName: s?.displayName ?? saved.displayName });
              }).catch(() => {});
            })
            .catch(() => setPersonaNotice({ status: 'interrupted', displayName: saved.displayName }));
        } else if (saved.status === 'done') {
          setPersonaNotice({ status: 'done', displayName: saved.displayName });
        } else if (saved.status === 'interrupted') {
          setPersonaNotice({ status: 'interrupted', displayName: saved.displayName });
        }
      }).catch(() => {});
  }, []);

  useEffect(() => {
    function onStart(e: Event) {
      const detail = (e as CustomEvent).detail as { displayName: string };
      setPersonaNotice({ status: 'creating', displayName: detail.displayName });
    }
    function onSettled() {
      invoke<{ status: string; displayName: string } | null>('get_persona_build_status').then((s) => {
        if (!s) return;
        setPersonaNotice({ status: s.status as PersonaBuildNoticeStatus, displayName: s.displayName });
      }).catch(() => {});
    }
    document.addEventListener('persona-build-start', onStart);
    document.addEventListener('persona-build-settled', onSettled);
    return () => {
      document.removeEventListener('persona-build-start', onStart);
      document.removeEventListener('persona-build-settled', onSettled);
    };
  }, []);

  usePostGeneration({ characters, igMode, chatMode, onPostGenerated: refreshPosts });

  const activeCharacter = useMemo(
    () => characters.find((c) => c.id === activeCharacterId) ?? null,
    [characters, activeCharacterId],
  );
  const characterModel = activeCharacter?.model ?? model;

  const handleSwitchView = useCallback((v: 'history' | 'settings' | 'connect' | 'memos') => {
    setSideView(v);
    setSideOpen(true);
  }, []);

  const handleIgModeChange = useCallback((enabled: boolean) => {
    setIgMode(enabled);
    localStorage.setItem(IG_MODE_KEY, String(enabled));
    if (!enabled) setMainTab('chat');
  }, []);

  // Chat management state
  const [chatMetas, setChatMetas] = useState<ChatMeta[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [initMessages, setInitMessages] = useState<Message[]>([]);

  const visibleChatMetas = useMemo(() => chatMetas.filter((m) => !m.id.startsWith('char_')), [chatMetas]);

  useEffect(() => { invoke<ChatMeta[]>('list_chats').then(setChatMetas).catch(() => {}); }, []);

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
    setChatMetas((prev) => [{ id, title, createdAt }, ...prev]);
    setActiveChatId(id);
  }, []);

  const onSave = useCallback((id: string, messages: Message[]) => { invoke('save_chat_messages', { id, messages }).catch(() => {}); }, []);

  const { messages, isThinking, agentStatus, error, handleSend, handleRetry, handleStop } = useOllamaChat(
    characterModel, activeChatId, initMessages, onChatCreated, onSave, activeCharacter,
  );

  const sttPrefixRef = useRef('');
  const handleSttTranscript = useCallback((text: string) => {
    const prefix = sttPrefixRef.current;
    // InputBar is deeply managed inside AppChat now, meaning we need AppChat to manage its own value.
    // However, AppChat passes `onSend` and handles stt prefix if we structure it properly. 
    // To minimize API changes, we'll dispatch an event that InputBar listens to.
    document.dispatchEvent(new CustomEvent('stt-transcript', { detail: prefix ? `${prefix} ${text}` : text }));
  }, []);

  const { isListening, sttError, startListening, stopListening } = useStt(handleSttTranscript);

  const handleSttToggle = useCallback(() => {
    if (isListening) stopListening();
    else {
      // InputBar sets prefix state globally
      document.dispatchEvent(new Event('stt-request-prefix'));
      setTimeout(() => startListening(), 50);
    }
  }, [isListening, startListening, stopListening]);

  useEffect(() => {
    function onPrefix(e: Event) { sttPrefixRef.current = (e as CustomEvent).detail; }
    document.addEventListener('stt-provide-prefix', onPrefix);
    return () => document.removeEventListener('stt-provide-prefix', onPrefix);
  }, []);

  const handleModelChange = useCallback((m: string) => {
    setModel(m);
    localStorage.setItem(MODEL_STORAGE_KEY, m);
  }, []);

  const handleChatModeChange = useCallback((enabled: boolean) => {
    setChatMode(enabled);
    localStorage.setItem(CHAT_MODE_KEY, String(enabled));
    setActiveChatId(null);
    setInitMessages([]);
    setActiveCharacterId(null);
    setActiveMemoId(null);
    if (!enabled) setMainTab('chat');
  }, [setActiveMemoId]);

  const selectCharacter = useCallback((id: string) => {
    setActiveMemoId(null);
    const charChatId = `char_${id}`;
    setMainTab('chat');
    invoke<Message[]>('load_chat_messages', { id: charChatId })
      .then((msgs) => { setActiveCharacterId(id); setActiveChatId(charChatId); setInitMessages(msgs); })
      .catch(() => { setActiveCharacterId(id); setActiveChatId(charChatId); setInitMessages([]); });
  }, [setActiveMemoId]);

  useEffect(() => {
    if (quotedPost && activeChatId !== `char_${quotedPost.characterId}`) setQuotedPost(null);
  }, [activeChatId, quotedPost]);

  const startNewChat = useCallback(() => { setActiveChatId(null); setInitMessages([]); setActiveMemoId(null); }, [setActiveMemoId]);

  const switchChat = useCallback((id: string) => {
    setActiveMemoId(null);
    invoke<Message[]>('load_chat_messages', { id })
      .then((msgs) => { setActiveChatId(id); setInitMessages(msgs); })
      .catch(() => { setActiveChatId(id); setInitMessages([]); });
  }, [setActiveMemoId]);

  const deleteChat = useCallback((id: string) => {
    invoke('delete_chat', { id }).catch(() => {});
    setChatMetas((prev) => prev.filter((m) => m.id !== id));
    if (activeChatId === id) { setActiveChatId(null); setInitMessages([]); }
  }, [activeChatId]);

  const onSendWrapper = useCallback((text: string) => {
    if (isListening) stopListening();
    if (quotedPost) {
      const authorName = characters.find((c) => c.id === quotedPost.characterId)?.name ?? 'Unknown';
      handleSend(`[postquote:${authorName}]${quotedPost.text}[/postquote]\n${text}`);
      setQuotedPost(null);
    } else handleSend(text);
  }, [handleSend, isListening, stopListening, quotedPost, characters]);

  useEffect(() => {
    const unlisten = listen<PermissionRequestData>('pc-permission-request', (e) => setPermRequest(e.payload));
    return () => { unlisten.then(fn => fn()); };
  }, []);

  useEffect(() => {
    const unlisten = listen<{ id: string; questions: AskQuestion[] }>('ask-user-request', (e) => setAskUserRequest(e.payload));
    return () => { unlisten.then(fn => fn()); };
  }, []);

  return (
    <AppLayout
      chatMode={chatMode} igMode={igMode} mainTab={mainTab} setMainTab={setMainTab} handleChatModeChange={handleChatModeChange} sideOpen={sideOpen} setSideOpen={setSideOpen}
      sideView={sideView} handleSwitchView={handleSwitchView} startNewChat={startNewChat} visibleChatMetas={visibleChatMetas} activeChatId={activeChatId} switchChat={switchChat}
      deleteChat={deleteChat} model={model} handleModelChange={handleModelChange} handleOllamaEndpointChanged={() => {}} characters={characters} activeCharacterId={activeCharacterId}
      selectCharacter={selectCharacter} addCharacter={addCharacter} deleteCharacter={deleteCharacter} handleIgModeChange={handleIgModeChange} activeMemoId={activeMemoId}
      handleSelectMemo={handleSelectMemo} personaNotice={personaNotice} onPersonaNoticeClose={() => { setPersonaNotice(null); invoke('clear_persona_build_status').catch(() => {}); }}
    >
      {mainTab === 'posts' ? (
        <AppSocial
          posts={posts} characters={characters} likedPostIds={likedPostIds} toggleLike={toggleLike} deletePost={deletePost}
          addPost={addPost} refreshPosts={refreshPosts} activeChatId={activeChatId} setInitMessages={setInitMessages}
        />
      ) : (
        <AppChat
          activeMemoId={activeMemoId} memoMessages={memoMessages} memoStreaming={memoStreaming} memoStreamContent={memoStreamContent} handleMemoSend={handleMemoSend}
          messages={messages} activeCharacter={activeCharacter} isThinking={isThinking} agentStatus={agentStatus} permRequest={permRequest} setPermRequest={setPermRequest}
          askUserRequest={askUserRequest} setAskUserRequest={setAskUserRequest} error={error} handleRetry={handleRetry} onSend={onSendWrapper} isListening={isListening}
          sttError={sttError} handleSttToggle={handleSttToggle} handleStop={handleStop} quotedPost={quotedPost} setQuotedPost={setQuotedPost} model={model}
          handleSaveMemo={handleSaveMemo} handleOpenMemo={handleOpenMemo}
        />
      )}
    </AppLayout>
  );
}

export default App;
