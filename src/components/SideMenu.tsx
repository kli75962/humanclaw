import { Menu, PenSquare, Trash2 } from 'lucide-react';
import type { SideMenuProps } from '../types';

export function SideMenu({ open, onClose, onNewChat, chats, activeChatId, onSelectChat, onDeleteChat }: SideMenuProps) {
  if (!open) return null;

  return (
    <>
      {/* Dim backdrop */}
      <div
        style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)', zIndex: 50 }}
        onClick={onClose}
      />

      {/* Panel */}
      <div
        style={{ position: 'fixed', top: 0, left: 0, height: '100%', width: '16rem', background: '#1C1C1E', zIndex: 60, overflowY: 'auto' }}
        className="flex flex-col p-4 gap-2"
      >
        {/* Close button — same position as the TopBar menu button */}
        <button
          onClick={onClose}
          className="p-2 hover:bg-[#2C2C2C] rounded-full transition-colors self-start -ml-0"
        >
          <Menu size={22} className="text-gray-400" />
        </button>

        <button
          onClick={onNewChat}
          className="flex items-center px-3 py-2 rounded-xl hover:bg-[#2C2C2C] transition-colors text-sm font-medium"
          style={{ color: '#E3E3E3' }}
        >
          <PenSquare size={17} className="text-gray-400 shrink-0" style={{ marginRight: '12px' }} />
          Start New Chat
        </button>

        <span className="px-3 pt-2 pb-1 text-xs text-gray-500 font-medium tracking-wider uppercase">
          History
        </span>

        {chats.map((chat) => (
          <div
            key={chat.id}
            className="flex items-center rounded-xl"
            style={{ background: chat.id === activeChatId ? '#2C2C2C' : undefined }}
          >
            <button
              onClick={() => onSelectChat(chat.id)}
              className="flex-1 text-left px-3 py-2 text-sm truncate rounded-xl hover:bg-[#2C2C2C] transition-colors"
              style={{ color: chat.id === activeChatId ? '#E3E3E3' : '#9CA3AF' }}
            >
              {chat.title}
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); onDeleteChat(chat.id); }}
              onMouseEnter={(e) => (e.currentTarget.style.background = '#3C3C3C')}
              onMouseLeave={(e) => (e.currentTarget.style.background = '')}
              className="p-2 rounded-full transition-colors shrink-0"
              style={{ color: '#9CA3AF' }}
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
      </div>
    </>
  );
}
