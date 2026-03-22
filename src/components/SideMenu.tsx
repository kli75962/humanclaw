import { memo, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { ChevronRight, Menu, PenSquare, Settings, Trash2 } from 'lucide-react';
import { useSession } from '../hooks/useSession';
import { SegmentControl } from './SettingsUI';
import { GeneralTab } from './SettingsGeneralTab';
import { ConnectTab } from './SettingsConnectTab';
import type { SideMenuProps } from '../types';
import '../style/SideMenu.css';
import '../style/SettingsScreen.css';

type SettingsTab = 'general' | 'connect';

function SettingsPanel({
  model,
  onModelChange,
  onOllamaEndpointChanged,
}: {
  model: string;
  onModelChange: (m: string) => void;
  onOllamaEndpointChanged: () => void;
}) {
  const [tab, setTab] = useState<SettingsTab>('general');
  const [peerStatus, setPeerStatus] = useState<Record<string, boolean>>({});
  const { session, refresh, removeLinkedDevice, setOllamaEndpoint, listPersonas, setPersona } = useSession();
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
      <div className="side-menu-settings-tabs">
        <SegmentControl
          options={[
            { value: 'general' as const, label: 'General' },
            { value: 'connect' as const, label: 'Connect' },
          ]}
          value={tab}
          onChange={setTab}
        />
      </div>
      <div className="side-menu-settings-body custom-scrollbar">
        <div className="side-menu-settings-content">
          {tab === 'general' && (
            <GeneralTab
              model={model}
              availableModels={[]}
              onModelChange={onModelChange}
              session={session}
              listPersonas={listPersonas}
              setPersona={setPersona}
              setOllamaEndpoint={setOllamaEndpoint}
              onOllamaEndpointChanged={onOllamaEndpointChanged}
            />
          )}
          {tab === 'connect' && (
            <ConnectTab
              session={session}
              isAndroid={isAndroid}
              peerStatus={peerStatus}
              removeLinkedDevice={removeLinkedDevice}
              onPaired={refresh}
            />
          )}
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
}: SideMenuProps) {
  return (
    <div className="side-menu-panel">
      {/* Top nav */}
      <div className="side-menu-top">
        <button
          className={`top-nav-btn${view === 'settings' ? ' top-nav-btn--active' : ''}`}
          onClick={() => onSwitchView('settings')}
          aria-label="Settings"
        >
          <Settings size={22} />
        </button>
        <button
          className={`top-nav-btn${view === 'history' ? ' top-nav-btn--active' : ''}`}
          onClick={() => onSwitchView('history')}
          aria-label="Chat history"
        >
          <Menu size={22} />
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
        {view === 'history' && (
          <>
            <button onClick={onNewChat} className="side-menu-new-chat-btn">
              <PenSquare size={17} style={{ marginRight: 12, color: 'var(--color-text-2)', flexShrink: 0 }} />
              Start New Chat
            </button>

            <span className="side-menu-history-label">History</span>

            {chats.map((chat) => (
              <div key={chat.id} className="side-menu-chat-item">
                <button
                  onClick={() => onSelectChat(chat.id)}
                  className={`side-menu-chat-btn${chat.id === activeChatId ? ' side-menu-chat-btn--active' : ''}`}
                >
                  {chat.title}
                </button>
                <button
                  onClick={(e) => { e.stopPropagation(); onDeleteChat(chat.id); }}
                  className="side-menu-delete-btn"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            ))}
          </>
        )}

        {view === 'settings' && (
          <SettingsPanel
            model={model}
            onModelChange={onModelChange}
            onOllamaEndpointChanged={onOllamaEndpointChanged}
          />
        )}
      </div>
    </div>
  );
});
