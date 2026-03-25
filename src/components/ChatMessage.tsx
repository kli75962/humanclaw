import { Fragment, memo, useMemo } from 'react';
import { Sparkles, RotateCcw } from 'lucide-react';
import type { ChatMessageProps } from '../types';
import '../style/ChatMessage.css';

function formatAssistantText(raw: string): string {
  if (!raw) return raw;

  if (/\n/.test(raw)) {
    return raw.replace(/\n{3,}/g, '\n\n').trim();
  }

  let text = raw
    .replace(/\s*[•·]\s*/g, '\n•\u00a0')
    .replace(/\s*(Day\s+\d+)/g, '\n\n$1');

  const rawLines = text.split('\n').map(l => l.trim()).filter(Boolean);
  const out: string[] = [];
  let prevWasBullet = false;

  for (const line of rawLines) {
    const isBullet = line.startsWith('•');

    if (isBullet) {
      out.push(line);
      prevWasBullet = true;
    } else {
      const headerMatch = line.match(/^(.{1,15}[^(（\d])：([\s\S]*)$/);

      if (headerMatch) {
        const header = headerMatch[1].trim() + '：';
        const rest = headerMatch[2].trim();
        out.push('');
        out.push(header);
        if (rest) out.push(rest);
      } else {
        if (prevWasBullet) out.push('');
        out.push(line);
      }
      prevWasBullet = false;
    }
  }

  return out.join('\n').replace(/\n{3,}/g, '\n\n').trim();
}

function parseQuoteBlock(content: string): { authorName: string; quoteText: string; rest: string } | null {
  const match = content.match(/^\[postquote:([^\]]*)\]([\s\S]*?)\[\/postquote\]\n?([\s\S]*)$/);
  if (match) return { authorName: match[1], quoteText: match[2], rest: match[3] };
  return null;
}

export const ChatMessage = memo(function ChatMessage({ message, isLastMessage, isThinking, onRetry }: ChatMessageProps) {
  const isUser = message.role === 'user';
  const isStreaming = isThinking && isLastMessage && !isUser;

  const parsed = useMemo(() => isUser ? parseQuoteBlock(message.content) : null, [isUser, message.content]);
  const bodyContent = parsed ? parsed.rest : message.content;

  const displayContent = useMemo(
    () => (isUser ? bodyContent : formatAssistantText(bodyContent)),
    [isUser, bodyContent],
  );

  return (
    <div className={`chat-message${isUser ? ' chat-message--user' : ''}`}>
      {!isUser && (
        <div className={`chat-avatar${isStreaming ? ' chat-avatar--spinning' : ''}`}>
          <Sparkles size={24} />
        </div>
      )}

      <div className={`chat-bubble${isUser ? ' chat-bubble--user' : ' chat-bubble--assistant'}`}>
        {parsed && (
          <div className="chat-post-quote">
            <span className="chat-post-quote-author">{parsed.authorName}</span>
            <span className="chat-post-quote-text">{parsed.quoteText}</span>
          </div>
        )}
        {displayContent.split('\n').map((line, i, arr) => (
          <Fragment key={i}>
            {line}
            {i < arr.length - 1 && <br />}
          </Fragment>
        ))}
        {isStreaming && <span className="chat-cursor" />}
      </div>

      {onRetry && isUser && (
        <button onClick={onRetry} className="chat-retry-btn" title="Retry">
          <RotateCcw size={15} />
        </button>
      )}
    </div>
  );
});
