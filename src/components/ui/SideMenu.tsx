import { memo, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { BookMarked, ChevronRight, Link2, Menu, PenSquare, Settings, Trash2, UserPlus, X } from 'lucide-react';
import { useSession } from '../../hooks/useSession';
import { GeneralTab } from '../settings/SettingsGeneralTab';
import { ConnectTab } from '../settings/SettingsConnectTab';
import { CreateFriendInline } from '../character/CreateFriendInline';
import { PersonaBuildNotice } from '../persona/PersonaBuildNotice';
import type { SideMenuProps } from '../../types';
import '../../style/SideMenu.css';
import '../../style/SettingsScreen.css';

function SettingsPanel({
  model,
  onModelChange,
  onOllamaEndpointChanged,
  chatMode,
  onChatModeChange,
  igMode,
  onIgModeChange,
}: {
  model: string;
  onModelChange: (m: string) => void;
  onOllamaEndpointChanged: () => void;
  chatMode: boolean;
  onChatModeChange: (v: boolean) => void;
  igMode: boolean;
  onIgModeChange: (v: boolean) => void;
}) {
  const { session, setOllamaEndpoint, listPersonas, setPersona, setPcPermissions } = useSession();

  return (
    <div className="side-menu-settings">
      <div className="side-menu-settings-body">
        <div className="side-menu-settings-content">
          <GeneralTab
            model={model}
            availableModels={[]}
            onModelChange={onModelChange}
            session={session}
            listPersonas={listPersonas}
            setPersona={setPersona}
            setOllamaEndpoint={setOllamaEndpoint}
            onOllamaEndpointChanged={onOllamaEndpointChanged}
            chatMode={chatMode}
            onChatModeChange={onChatModeChange}
            igMode={igMode}
            onIgModeChange={onIgModeChange}
            setPcPermissions={setPcPermissions}
          />
        </div>
      </div>
    </div>
  );
}

function MemosPanel({
  activeMemoId,
  onSelectMemo,
}: {
  activeMemoId: string | null;
  onSelectMemo: (id: string) => void;
}) {
  const [memos, setMemos] = useState<import('../../types').MemoMeta[]>([]);
  const [confirmId, setConfirmId] = useState<string | null>(null);

  function refresh() {
    invoke<import('../../types').MemoMeta[]>('list_memos').then(setMemos).catch(() => {});
  }

  useEffect(() => {
    refresh();
    document.addEventListener('memo-saved', refresh);
    return () => document.removeEventListener('memo-saved', refresh);
  }, []);

  async function handleDelete(id: string) {
    await invoke('delete_memo', { id }).catch(() => {});
    setMemos((prev) => prev.filter((m) => m.id !== id));
    setConfirmId(null);
  }

  return (
    <div className="side-menu-memos">
      {memos.length === 0 ? (
        <p className="side-menu-memos-empty">No saved memos yet.<br />Select text in chat and click "Explain more".</p>
      ) : (
        memos.map((memo) => (
          <div key={memo.id} className="side-menu-item-wrap">
            <div className="side-menu-chat-item">
              <button
                onClick={() => onSelectMemo(memo.id)}
                className={`side-menu-chat-btn${memo.id === activeMemoId ? ' side-menu-chat-btn--active' : ''}`}
              >
                {memo.title}
              </button>
              <button
                className="side-menu-delete-btn"
                onClick={(e) => { e.stopPropagation(); setConfirmId(memo.id); }}
              >
                <Trash2 size={14} />
              </button>
            </div>
            {confirmId === memo.id && (
              <div className="side-menu-confirm">
                <span className="side-menu-confirm-text">Delete this memo?</span>
                <div className="side-menu-confirm-btns">
                  <button className="side-menu-confirm-btn side-menu-confirm-btn--no" onClick={() => setConfirmId(null)}>No</button>
                  <button className="side-menu-confirm-btn side-menu-confirm-btn--yes" onClick={() => handleDelete(memo.id)}>Yes</button>
                </div>
              </div>
            )}
          </div>
        ))
      )}
    </div>
  );
}

function ConnectPanel() {
  const [peerStatus, setPeerStatus] = useState<Record<string, boolean>>({});
  const { session, refresh, removeLinkedDevice } = useSession();
  const isAndroid = session?.device.device_type === 'android';

  useEffect(() => {
    if ((session?.paired_devices ?? []).length === 0) return;
    invoke<Array<{ device_id: string; online: boolean }>>('get_all_peer_status')
      .then((list) => setPeerStatus(Object.fromEntries(list.map((p) => [p.device_id, p.online]))))
      .catch(() => {});
    const unlisten = listen<Array<{ device_id: string; online: boolean }>>('peer-status-changed', (event) => {
      setPeerStatus(Object.fromEntries(event.payload.map((p) => [p.device_id, p.online])));
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [session?.paired_devices]);

  return (
    <div className="side-menu-settings">
      <div className="side-menu-settings-body">
        <div className="side-menu-settings-content">
          <ConnectTab
            session={session}
            isAndroid={isAndroid}
            peerStatus={peerStatus}
            removeLinkedDevice={removeLinkedDevice}
            onPaired={refresh}
          />
        </div>
      </div>
    </div>
  );
}

export const SideMenu = memo(function SideMenu({
  view, onSwitchView,
  onNewChat, chats, activeChatId, onSelectChat, onDeleteChat,
  model, onModelChange, onOllamaEndpointChanged,
  isMobileOpen, onCloseSide,
  chatMode, onChatModeChange,
  characters, activeCharacterId, onSelectCharacter, onCreateCharacter, onDeleteCharacter,
  igMode, onIgModeChange,
  activeMemoId, onSelectMemo,
  personaNotice, onPersonaNoticeClose,
}: SideMenuProps) {
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [confirmId, setConfirmId] = useState<string | null>(null);

  return (
    <div className="side-menu-panel">
      {/* Top nav */}
      <div className="side-menu-top">
        <button
          className={`top-nav-btn${view === 'history' ? ' top-nav-btn--active' : ''}`}
          onClick={() => onSwitchView('history')}
          aria-label="Chat history"
        >
          <Menu size={22} />
        </button>
        <button
          className={`top-nav-btn${view === 'memos' ? ' top-nav-btn--active' : ''}`}
          onClick={() => onSwitchView('memos')}
          aria-label="Memos"
        >
          <BookMarked size={22} />
        </button>
        <button
          className={`top-nav-btn top-nav-btn--right${view === 'connect' ? ' top-nav-btn--active' : ''}`}
          onClick={() => onSwitchView('connect')}
          aria-label="Connect"
        >
          <Link2 size={22} />
        </button>
        <button
          className={`top-nav-btn${view === 'settings' ? ' top-nav-btn--active' : ''}`}
          onClick={() => onSwitchView('settings')}
          aria-label="Settings"
        >
          <Settings size={22} />
        </button>
        {isMobileOpen && (
          <button
            className="top-nav-btn top-nav-btn--back"
            onClick={onCloseSide}
            aria-label="Back to chat"
          >
            <ChevronRight size={22} />
          </button>
        )}
      </div>

      {/* Scrollable content */}
      <div className="side-menu-scroll">
        {view === 'history' && !chatMode && (
          <>
            <button onClick={onNewChat} className="side-menu-new-chat-btn">
              <PenSquare size={17} style={{ marginRight: 12, color: 'var(--color-text-2)', flexShrink: 0 }} />
              Start New Chat
            </button>

            <span className="side-menu-history-label">History</span>

            {chats.map((chat) => (
              <div key={chat.id} className="side-menu-item-wrap">
                <div className="side-menu-chat-item">
                  <button
                    onClick={() => onSelectChat(chat.id)}
                    className={`side-menu-chat-btn${chat.id === activeChatId ? ' side-menu-chat-btn--active' : ''}`}
                  >
                    {chat.title}
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); setConfirmId(chat.id); }}
                    className="side-menu-delete-btn"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
                {confirmId === chat.id && (
                  <div className="side-menu-confirm">
                    <span className="side-menu-confirm-text">Remove this chat?</span>
                    <div className="side-menu-confirm-btns">
                      <button className="side-menu-confirm-btn side-menu-confirm-btn--no" onClick={() => setConfirmId(null)}>No</button>
                      <button className="side-menu-confirm-btn side-menu-confirm-btn--yes" onClick={() => { onDeleteChat(chat.id); setConfirmId(null); }}>Yes</button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </>
        )}

        {view === 'history' && chatMode && (
          <>
            <button
              onClick={() => setShowCreateForm((v) => !v)}
              className={`side-menu-new-chat-btn${showCreateForm ? ' side-menu-new-chat-btn--active' : ''}`}
            >
              <UserPlus size={17} style={{ marginRight: 12, color: 'var(--color-text-2)', flexShrink: 0 }} />
              Add New Friend
            </button>

            {showCreateForm && (
              <CreateFriendInline
                defaultModel={model}
                onSave={(data) => { onCreateCharacter(data); setShowCreateForm(false); }}
                onCancel={() => setShowCreateForm(false)}
              />
            )}

            <span className="side-menu-history-label">Friends</span>

            {characters.map((char) => (
              <div key={char.id} className="side-menu-item-wrap">
                <div className="side-menu-chat-item">
                  <button
                    onClick={() => onSelectCharacter(char.id)}
                    className={`side-menu-chat-btn side-menu-friend-btn${char.id === activeCharacterId ? ' side-menu-chat-btn--active' : ''}`}
                  >
                    {char.icon ? (
                      <img src={char.icon} className="side-menu-char-avatar" alt="" />
                    ) : (
                      <span className="side-menu-char-avatar-placeholder">
                        {char.name.charAt(0).toUpperCase()}
                      </span>
                    )}
                    <span className="side-menu-friend-name">{char.name}</span>
                  </button>
                  <button
                    onClick={(e) => { e.stopPropagation(); setConfirmId(char.id); }}
                    className="side-menu-delete-btn"
                  >
                    <X size={16} />
                  </button>
                </div>
                {confirmId === char.id && (
                  <div className="side-menu-confirm">
                    <span className="side-menu-confirm-text">Remove this friend?</span>
                    <div className="side-menu-confirm-btns">
                      <button className="side-menu-confirm-btn side-menu-confirm-btn--no" onClick={() => setConfirmId(null)}>No</button>
                      <button className="side-menu-confirm-btn side-menu-confirm-btn--yes" onClick={() => { onDeleteCharacter(char.id); setConfirmId(null); }}>Yes</button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </>
        )}

        {view === 'settings' && (
          <SettingsPanel
            model={model}
            onModelChange={onModelChange}
            onOllamaEndpointChanged={onOllamaEndpointChanged}
            chatMode={chatMode}
            onChatModeChange={onChatModeChange}
            igMode={igMode}
            onIgModeChange={onIgModeChange}
          />
        )}

        {view === 'connect' && <ConnectPanel />}

        {view === 'memos' && <MemosPanel activeMemoId={activeMemoId} onSelectMemo={onSelectMemo} />}

      </div>

      {personaNotice && (
        <PersonaBuildNotice
          status={personaNotice.status}
          displayName={personaNotice.displayName}
          onClose={() => onPersonaNoticeClose?.()}
        />
      )}
    </div>
  );
});
