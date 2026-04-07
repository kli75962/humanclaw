import { Fragment, memo, useMemo } from 'react';
import { Sparkles, RotateCcw, File } from 'lucide-react';
import type { ChatMessageProps } from '../../types';
import '../../style/ChatMessage.css';

/** Extract <file name="...">...</file> blocks from a user message.
 *  Returns the filenames and the remaining text (file content stripped). */
function parseFileAttachments(content: string): { files: string[]; text: string } {
  const files: string[] = [];
  const cleaned = content.replace(/<file name="([^"]+)">([\s\S]*?)<\/file>/g, (_m, name: string) => {
    files.push(name);
    return '';
  });
  return { files, text: cleaned.trim() };
}

export function formatAssistantText(raw: string): string {
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

  const attachments = useMemo(
    () => isUser ? parseFileAttachments(message.content) : null,
    [isUser, message.content],
  );

  const contentAfterFiles = attachments ? attachments.text : message.content;
  const parsed = useMemo(() => isUser ? parseQuoteBlock(contentAfterFiles) : null, [isUser, contentAfterFiles]);
  const bodyContent = parsed ? parsed.rest : contentAfterFiles;

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
        {attachments && attachments.files.length > 0 && (
          <div className="chat-file-attachments">
            {attachments.files.map((name, i) => (
              <div key={i} className="chat-file-chip">
                <File size={14} />
                <span className="chat-file-chip-name">{name}</span>
              </div>
            ))}
          </div>
        )}
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
        {isStreaming && (
          <span className="chat-typing">
            <span /><span /><span />
          </span>
        )}
      </div>

      {onRetry && isUser && (
        <button onClick={onRetry} className="chat-retry-btn" title="Retry">
          <RotateCcw size={15} />
        </button>
      )}
    </div>
  );
});
