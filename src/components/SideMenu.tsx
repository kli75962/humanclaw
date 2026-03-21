import { memo } from 'react';
import { Menu, PenSquare, Trash2 } from 'lucide-react';
import type { SideMenuProps } from '../types';
import '../style/SideMenu.css';

export const SideMenu = memo(function SideMenu({ open, onClose, onNewChat, chats, activeChatId, onSelectChat, onDeleteChat }: SideMenuProps) {
  if (!open) return null;

  return (
    <>
      <div className="side-menu-backdrop" onClick={onClose} />

      <div className="side-menu-panel">
        <button onClick={onClose} className="side-menu-close-btn">
          <Menu size={22} />
        </button>

        <button onClick={onNewChat} className="side-menu-new-chat-btn">
          <PenSquare size={17} style={{ marginRight: 12, color: '#9CA3AF', flexShrink: 0 }} />
          Start New Chat
        </button>

        <span className="side-menu-history-label">History</span>

        {chats.map((chat) => (
          <div key={chat.id} className="side-menu-chat-item">
            <button
              onClick={() => onSelectChat(chat.id)}
              className="side-menu-chat-btn"
              style={{
                background: chat.id === activeChatId ? '#2C2C2C' : undefined,
                color: chat.id === activeChatId ? '#E3E3E3' : '#9CA3AF',
              }}
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
      </div>
    </>
  );
});
