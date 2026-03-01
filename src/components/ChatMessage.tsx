import { Sparkles } from 'lucide-react';
import type { Message } from '../types';

interface ChatMessageProps {
  message: Message;
  isLastMessage: boolean;
  isThinking: boolean;
}

/**
 * Renders a single chat bubble.
 * - User messages: grey bubble, right-aligned
 * - Assistant messages: plain text, left-aligned with a Sparkles avatar
 *   A blinking cursor is shown on the last assistant message while streaming.
 */
export function ChatMessage({ message, isLastMessage, isThinking }: ChatMessageProps) {
  const isUser = message.role === 'user';
  const isStreaming = isThinking && isLastMessage && !isUser;

  return (
    <div className={`flex gap-4 ${isUser ? 'flex-row-reverse' : ''}`}>
      {/* Avatar — only for assistant */}
      {!isUser && (
        <div className="w-8 h-8 shrink-0 mt-1">
          <Sparkles
            className={`text-blue-400 ${isStreaming ? 'animate-spin' : ''}`}
            size={24}
          />
        </div>
      )}

      {/* Bubble */}
      <div
        className={`max-w-[85%] text-[16px] leading-7 whitespace-pre-wrap ${
          isUser
            ? 'bg-[#2C2C2C] px-5 py-3 rounded-3xl rounded-tr-sm'
            : 'text-gray-100 px-0'
        }`}
      >
        {message.content}
        {/* Blinking cursor while the assistant is streaming */}
        {isStreaming && (
          <span className="inline-block w-[2px] h-[1em] bg-blue-400 ml-0.5 animate-pulse align-middle" />
        )}
      </div>
    </div>
  );
}
