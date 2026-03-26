import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useOllamaChat } from './hooks/useOllamaChat';
import { useCharacters } from './hooks/useCharacters';
import { usePosts } from './hooks/usePosts';
import { usePostGeneration } from './hooks/usePostGeneration';
import { useStt } from './hooks/useStt';
import { WelcomeScreen } from './components/WelcomeScreen';
import { ChatMessage } from './components/ChatMessage';
import { InputBar } from './components/InputBar';
import { SideMenu } from './components/SideMenu';
import { AccessibilityDialog } from './components/AccessibilityDialog';
import { PermissionRequest } from './components/PermissionDialog';
import type { PermissionRequest as PermissionRequestData } from './components/PermissionDialog';
import { ExplainPopup } from './components/ExplainPopup';
import { MemoChatView } from './components/MemoChatView';
import type { WizardAnswers, MemoMeta } from './types';
import { Bot, ChevronLeft, LayoutGrid, MessageCircle, Users } from 'lucide-react';
import { PostFeed } from './components/PostFeed';
import type { ChatMeta, InputBarHandle, Message, Post } from './types';
import './style/themes.css';
import './style/App.css';

const DEFAULT_MODEL = 'kimi-k2.5:cloud';
const MODEL_STORAGE_KEY = 'phoneclaw_model';
const SIDE_WIDTH_KEY = 'phoneclaw_side_width';
const CHAT_MODE_KEY = 'phoneclaw_chat_mode';
const IG_MODE_KEY = 'phoneclaw_ig_mode';
const MIN_SIDE = 200;
const MAX_SIDE_RATIO = 0.6;

function App() {
  const [model, setModel] = useState(
    () => localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL
  );
  const [chatMode, setChatMode] = useState(
    () => localStorage.getItem(CHAT_MODE_KEY) === 'true'
  );
  const [igMode, setIgMode] = useState(
    () => localStorage.getItem(IG_MODE_KEY) === 'true'
  );
  const [activeCharacterId, setActiveCharacterId] = useState<string | null>(null);
  const { characters, addCharacter, deleteCharacter } = useCharacters();
  const { posts, likedPostIds, toggleLike, deletePost, addPost, refresh: refreshPosts } = usePosts();
  const [quotedPost, setQuotedPost] = useState<Post | null>(null);
  const [permRequest, setPermRequest] = useState<PermissionRequestData | null>(null);

  usePostGeneration({
    characters,
    igMode,
    chatMode,
    onPostGenerated: () => refreshPosts(),
  });

  const activeCharacter = useMemo(
    () => characters.find((c) => c.id === activeCharacterId) ?? null,
    [characters, activeCharacterId],
  );
  const characterModel = activeCharacter?.model ?? model;

  const [mainTab, setMainTab] = useState<'chat' | 'posts'>('chat');
  const [sideView, setSideView] = useState<'history' | 'settings' | 'connect' | 'memos'>('history');
  const [sideOpen, setSideOpen] = useState(false);

  const handleSwitchView = useCallback((v: 'history' | 'settings' | 'connect' | 'memos') => {
    setSideView(v);
    setSideOpen(true);
  }, []);

  // Explain popup
  const [explainText, setExplainText] = useState('');
  const [showExplain, setShowExplain] = useState(false);
  const [floatBtn, setFloatBtn] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    function onMouseUp() {
      const sel = window.getSelection();
      if (!sel || sel.isCollapsed || !sel.toString().trim()) {
        setFloatBtn(null);
        return;
      }
      const range = sel.getRangeAt(0);
      const messagesEl = document.querySelector('.app-messages');
      if (!messagesEl?.contains(range.commonAncestorContainer)) {
        setFloatBtn(null);
        return;
      }
      const rect = range.getBoundingClientRect();
      setFloatBtn({ x: rect.left + rect.width / 2, y: rect.bottom + 8 });
    }
    document.addEventListener('mouseup', onMouseUp);
    return () => document.removeEventListener('mouseup', onMouseUp);
  }, []);

  const handleExplainClick = useCallback(() => {
    const sel = window.getSelection();
    const text = sel?.toString().trim() ?? '';
    if (!text) return;
    setExplainText(text);
    setShowExplain(true);
    setFloatBtn(null);
    sel?.removeAllRanges();
  }, []);

  // ── Memo chat state ──────────────────────────────────────────────────────
  const [activeMemoId, setActiveMemoId] = useState<string | null>(null);
  const [memoMessages, setMemoMessages] = useState<Message[]>([]);
  const [memoStreaming, setMemoStreaming] = useState(false);
  const [memoStreamContent, setMemoStreamContent] = useState('');
  const memoStreamBuf = useRef('');
  const activeMemoIdRef = useRef<string | null>(null);
  activeMemoIdRef.current = activeMemoId;

  const handleSelectMemo = useCallback(async (id: string) => {
    const msgs = await invoke<Message[]>('load_memo_messages', { id }).catch(() => []);
    setActiveMemoId(id);
    setMemoMessages(msgs);
    setMainTab('chat');
    setSideOpen(false);
  }, []);

  const handleSaveMemo = useCallback(async (title: string, msgs: Message[]): Promise<string | null> => {
    const meta = await invoke<MemoMeta>('create_memo', { title, messages: msgs }).catch(() => null);
    document.dispatchEvent(new Event('memo-saved'));
    return meta?.id ?? null;
  }, []);

  const handleOpenMemo = useCallback((id: string) => {
    invoke<Message[]>('load_memo_messages', { id }).then((msgs) => {
      setActiveMemoId(id);
      setMemoMessages(msgs);
      setMainTab('chat');
      setSideView('memos');
    }).catch(() => {});
  }, []);

  const handleMemoSend = useCallback(async (text: string) => {
    if (memoStreaming) return;
    const userMsg: Message = { role: 'user', content: text };
    const updatedMsgs = [...memoMessages, userMsg];
    setMemoMessages(updatedMsgs);
    setMemoStreaming(true);
    setMemoStreamContent('');
    memoStreamBuf.current = '';

    let unlistenFn: (() => void) | null = null;
    const unlistenPromise = listen<{ content: string; done: boolean }>('explain-stream', (e) => {
      if (e.payload.done) {
        const finalContent = memoStreamBuf.current;
        const assistantMsg: Message = { role: 'assistant', content: finalContent };
        const finalMsgs = [...updatedMsgs, assistantMsg];
        setMemoMessages(finalMsgs);
        setMemoStreaming(false);
        setMemoStreamContent('');
        memoStreamBuf.current = '';
        const id = activeMemoIdRef.current;
        if (id) invoke('save_memo_messages', { id, messages: finalMsgs }).catch(() => {});
        unlistenFn?.();
      } else {
        memoStreamBuf.current += e.payload.content;
        setMemoStreamContent(memoStreamBuf.current);
      }
    });
    unlistenPromise.then((fn) => { unlistenFn = fn; });

    const apiMsgs = updatedMsgs.map((m) => ({ role: m.role, content: m.content }));
    invoke('explain_text', { messages: apiMsgs, model }).catch((err: unknown) => {
      setMemoMessages((prev) => [...prev, { role: 'assistant', content: `Error: ${err}` }]);
      setMemoStreaming(false);
      unlistenPromise.then((fn) => fn());
    });
  }, [memoStreaming, memoMessages, model]);

  const handleIgModeChange = useCallback((enabled: boolean) => {
    setIgMode(enabled);
    localStorage.setItem(IG_MODE_KEY, String(enabled));
    if (!enabled) setMainTab('chat');
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

  // Only show normal chats (exclude character threads prefixed with "char_")
  const visibleChatMetas = useMemo(
    () => chatMetas.filter((m) => !m.id.startsWith('char_')),
    [chatMetas],
  );

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
    characterModel, activeChatId, initMessages, onChatCreated, onSave, activeCharacter,
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

  const handleChatModeChange = useCallback((enabled: boolean) => {
    setChatMode(enabled);
    localStorage.setItem(CHAT_MODE_KEY, String(enabled));
    setActiveChatId(null);
    setInitMessages([]);
    setActiveCharacterId(null);
    setActiveMemoId(null);
    if (!enabled) setMainTab('chat');
  }, []);

  const selectCharacter = useCallback((id: string) => {
    setActiveMemoId(null);
    const charChatId = `char_${id}`;
    setActiveCharacterId(id);
    setActiveChatId(charChatId);
    setMainTab('chat');
    invoke<Message[]>('load_chat_messages', { id: charChatId })
      .then((msgs) => { setInitMessages(msgs); })
      .catch(() => { setInitMessages([]); });
  }, []);

  // Clear the quoted post whenever the user navigates away from that character's chat.
  useEffect(() => {
    if (quotedPost && activeChatId !== `char_${quotedPost.characterId}`) {
      setQuotedPost(null);
    }
  }, [activeChatId, quotedPost]);

  const handleCreateUserPost = useCallback(async (text: string) => {
    const post = await addPost({ characterId: 'user', text });
    invoke<{ characterId: string; text: string }[]>('react_to_user_post', { postId: post.id })
      .then(async (dms) => {
        refreshPosts();
        for (const dm of dms) {
          const chatId = `char_${dm.characterId}`;
          const msgs = await invoke<{ role: string; content: string }[]>('load_chat_messages', { id: chatId }).catch(() => []);
          const updated = [...msgs, { role: 'assistant', content: dm.text }];
          await invoke('save_chat_messages', { id: chatId, messages: updated }).catch(() => {});
          // Refresh active chat if the user is already in it
          if (activeChatId === chatId) {
            setInitMessages(updated as Message[]);
          }
        }
      })
      .catch(() => {});
  }, [addPost, refreshPosts, activeChatId]);

  const startNewChat = useCallback(() => {
    setActiveChatId(null);
    setInitMessages([]);
    setActiveMemoId(null);
  }, []);

  const switchChat = useCallback((id: string) => {
    setActiveMemoId(null);
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
    if (quotedPost) {
      const character = characters.find((c) => c.id === quotedPost.characterId);
      const authorName = character?.name ?? 'Unknown';
      handleSend(`[postquote:${authorName}]${quotedPost.text}[/postquote]\n${text}`);
      setQuotedPost(null);
    } else {
      handleSend(text);
    }
  }, [handleSend, isListening, stopListening, quotedPost, characters]);

  const handleAddPersona = useCallback((answers: WizardAnswers) => {
    if (chatMode) handleChatModeChange(false);
    setMainTab('chat');
    setSideOpen(false);
    startNewChat();
    handleSend(
      `Create a new persona using the create_skill tool based on these preferences:\n` +
      `- Gender: ${answers.sex}\n` +
      `- Personality: ${answers.personality}\n` +
      `- Profession: ${answers.profession}\n` +
      `- Name: ${answers.personaName}\n\n` +
      `Follow the persona skill creation guide. The skill name must start with "persona_".`
    );
  }, [chatMode, handleChatModeChange, startNewChat, handleSend]);

  useEffect(() => {
    const unlisten = listen<PermissionRequestData>('pc-permission-request', (e) => {
      setPermRequest(e.payload);
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

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

      {floatBtn && (
        <button
          className="explain-float-btn"
          style={{ left: floatBtn.x, top: floatBtn.y }}
          onMouseDown={(e) => { e.preventDefault(); handleExplainClick(); }}
        >
          Explain more
        </button>
      )}

      {showExplain && (
        <ExplainPopup
          selectedText={explainText}
          model={model}
          contextMessages={messages}
          onClose={() => setShowExplain(false)}
          onSaveMemo={handleSaveMemo}
          onOpenMemo={handleOpenMemo}
        />
      )}

      {/* ── Tab bar (leftmost column) — mode switch ── */}
      <div className="app-tab-bar">
        <button
          className={`tab-bar-btn${!chatMode ? ' tab-bar-btn--active' : ''}`}
          onClick={() => { if (chatMode) handleChatModeChange(false); }}
          aria-label="Normal mode"
        >
          <Bot size={26} />
        </button>
        <button
          className={`tab-bar-btn${chatMode ? ' tab-bar-btn--active' : ''}`}
          onClick={() => { if (!chatMode) handleChatModeChange(true); }}
          aria-label="Chat mode"
        >
          <Users size={26} />
        </button>
      </div>

      {/* ── Mobile nav overlay (phone only, shown when side panel is closed) ── */}
      <div className="mobile-nav">
        <button
          className="top-nav-btn"
          onClick={() => setSideOpen(true)}
          aria-label="Open menu"
        >
          <ChevronLeft size={22} />
        </button>
      </div>

      {/* ── Left panel ── */}
      <div className="app-left" style={{ width: sideWidth }}>
        <SideMenu
          view={sideView}
          onSwitchView={handleSwitchView}
          onNewChat={startNewChat}
          chats={visibleChatMetas}
          activeChatId={activeChatId}
          onSelectChat={switchChat}
          onDeleteChat={deleteChat}
          model={model}
          onModelChange={handleModelChange}
          onOllamaEndpointChanged={handleOllamaEndpointChanged}
          isMobileOpen={sideOpen}
          onCloseSide={() => setSideOpen(false)}
          chatMode={chatMode}
          onChatModeChange={handleChatModeChange}
          characters={characters}
          activeCharacterId={activeCharacterId}
          onSelectCharacter={selectCharacter}
          onCreateCharacter={addCharacter}
          onDeleteCharacter={deleteCharacter}
          igMode={igMode}
          onIgModeChange={handleIgModeChange}
          onAddPersona={handleAddPersona}
          activeMemoId={activeMemoId}
          onSelectMemo={handleSelectMemo}
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

      {/* ── Right: main content ── */}
      <div className="app-right">
        {/* Content tabs — only when both chatMode and igMode are active */}
        {chatMode && igMode && (
          <div className="app-content-tabs">
            <button
              className={`content-tab-btn${mainTab === 'chat' ? ' content-tab-btn--active' : ''}`}
              onClick={() => setMainTab('chat')}
              aria-label="Chat"
            >
              <MessageCircle size={22} />
            </button>
            <button
              className={`content-tab-btn${mainTab === 'posts' ? ' content-tab-btn--active' : ''}`}
              onClick={() => setMainTab('posts')}
              aria-label="Posts"
            >
              <LayoutGrid size={22} />
            </button>
          </div>
        )}

        {mainTab === 'posts' ? (
          <div className="app-content custom-scrollbar">
            <div className="app-posts-feed">
              <PostFeed
                posts={posts}
                characters={characters}
                likedPostIds={likedPostIds}
                onLike={toggleLike}
                onDelete={deletePost}
                onCreatePost={handleCreateUserPost}
              />
            </div>
          </div>
        ) : activeMemoId ? (
          <MemoChatView
            key={activeMemoId}
            messages={memoMessages}
            streaming={memoStreaming}
            streamContent={memoStreamContent}
            onSend={handleMemoSend}
          />
        ) : (
          <>
            <div className="app-content custom-scrollbar">
              {messages.length === 0 ? (
                activeCharacter ? (
                  <div className="app-friend-empty">Start to chat with your new friend</div>
                ) : (
                  <WelcomeScreen onSend={onSend} />
                )
              ) : (
                <div className="app-messages">
                  {messageList}

                  {agentStatus && (
                    <div className="app-agent-status">
                      <span className="app-agent-dot" />
                      {agentStatus}
                    </div>
                  )}

                  {permRequest && (
                    <PermissionRequest
                      request={permRequest}
                      onDone={() => setPermRequest(null)}
                    />
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
              quotedPost={quotedPost}
              onClearQuote={() => setQuotedPost(null)}
            />
          </>
        )}
      </div>
    </div>
  );
}

export default App;
