import React from 'react';
import { Bot, ChevronLeft, LayoutGrid, MessageCircle, Users } from 'lucide-react';
import { AccessibilityDialog } from '../ui/AccessibilityDialog';
import { useWallpaper } from '../../hooks/useWallpaper';
import { SideMenu } from '../ui/SideMenu';
import { useDragSidebar } from '../../hooks/useDragSidebar';
import type { ChatMeta, Character } from '../../types';
import type { PersonaBuildNoticeStatus } from '../persona/PersonaBuildNotice';

interface AppLayoutProps {
  chatMode: boolean;
  igMode: boolean;
  mainTab: 'chat' | 'posts';
  setMainTab: (tab: 'chat' | 'posts') => void;
  handleChatModeChange: (enabled: boolean) => void;
  sideOpen: boolean;
  setSideOpen: (open: boolean) => void;
  // SideMenu Pass-through Props
  sideView: 'history' | 'settings' | 'connect' | 'memos';
  handleSwitchView: (v: 'history' | 'settings' | 'connect' | 'memos') => void;
  startNewChat: () => void;
  visibleChatMetas: ChatMeta[];
  activeChatId: string | null;
  switchChat: (id: string) => void;
  deleteChat: (id: string) => void;
  model: string;
  handleModelChange: (m: string) => void;
  handleOllamaEndpointChanged: () => void;
  characters: Character[];
  activeCharacterId: string | null;
  selectCharacter: (id: string) => void;
  addCharacter: (c: Omit<Character, 'id' | 'createdAt'>) => Promise<Character>;
  deleteCharacter: (id: string) => void;
  handleIgModeChange: (enabled: boolean) => void;
  activeMemoId: string | null;
  handleSelectMemo: (id: string) => void;
  personaNotice: { status: PersonaBuildNoticeStatus; displayName: string } | null;
  onPersonaNoticeClose: () => void;
  children: React.ReactNode;
}

export function AppLayout(props: AppLayoutProps) {
  const { url: wallpaperUrl, blur: wallpaperBlur, dim: wallpaperDim } = useWallpaper();
  const { sideWidth, handleDividerPointerDown, handleDividerPointerMove, handleDividerPointerUp } = useDragSidebar();

  const {
    chatMode, igMode, mainTab, setMainTab, handleChatModeChange, sideOpen, setSideOpen, children,
    // SideMenu specifics
    sideView, handleSwitchView, startNewChat, visibleChatMetas, activeChatId, switchChat, deleteChat,
    model, handleModelChange, handleOllamaEndpointChanged, characters, activeCharacterId, selectCharacter,
    addCharacter, deleteCharacter, handleIgModeChange, activeMemoId, handleSelectMemo, personaNotice, onPersonaNoticeClose
  } = props;

  return (
    <div className={`app-root${sideOpen ? ' side-open' : ''}${wallpaperUrl ? ' app-root--wallpaper' : ''}`}>
      {wallpaperUrl && (
        <>
          <div
            className="app-wallpaper-bg"
            style={{
              backgroundImage: `url(${wallpaperUrl})`,
              filter: wallpaperBlur > 0 ? `blur(${wallpaperBlur}px)` : undefined,
            }}
          />
          {wallpaperDim > 0 && (
            <div
              className="app-wallpaper-dim"
              style={{ background: `rgba(0,0,0,${wallpaperDim})` }}
            />
          )}
        </>
      )}
      <AccessibilityDialog />

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
          activeMemoId={activeMemoId}
          onSelectMemo={handleSelectMemo}
          personaNotice={personaNotice}
          onPersonaNoticeClose={onPersonaNoticeClose}
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
        {children}
      </div>
    </div>
  );
}
