import { Fragment } from 'react';
import { Sparkles } from 'lucide-react';
import type { ChatMessageProps } from '../types';

/**
 * Normalize assistant text so it always displays with proper line breaks,
 * regardless of whether the LLM actually included newlines.
 *
 * Strategy: tokenize on bullets and "Day N" markers, then heuristically
 * detect section headers (short text ending with ： that appears before
 * a bullet cluster or Day marker).
 */
function formatAssistantText(raw: string): string {
  if (!raw) return raw;

  // Already has real newlines — just normalize excess blank lines
  if (/\n/.test(raw)) {
    return raw.replace(/\n{3,}/g, '\n\n').trim();
  }

  // Insert sentinel newlines at reliable split points before processing
  let text = raw
    // Each bullet gets its own line
    .replace(/\s*[•·]\s*/g, '\n•\u00a0')
    // "Day N" / "Day N -" always starts a new block
    .replace(/\s*(Day\s+\d+)/g, '\n\n$1');

  // Now split into lines
  const rawLines = text.split('\n').map(l => l.trim()).filter(Boolean);

  const out: string[] = [];
  let prevWasBullet = false;

  for (const line of rawLines) {
    const isBullet = line.startsWith('•');

    if (isBullet) {
      out.push(line);
      prevWasBullet = true;
    } else {
      // Non-bullet: check if it contains a section header pattern.
      // A section header ends with ：and the text before ：is ≤ 15 chars.
      // Split on ： only at header boundaries (not inside bullet content).
      const headerMatch = line.match(/^(.{1,15}[^(（\d])：([\s\S]*)$/);

      if (headerMatch) {
        const header = headerMatch[1].trim() + '：';
        const rest = headerMatch[2].trim();
        // Blank line before section header
        out.push('');
        out.push(header);
        if (rest) out.push(rest);
      } else {
        // Plain paragraph — add blank line before if previous was a bullet group
        if (prevWasBullet) out.push('');
        out.push(line);
      }
      prevWasBullet = false;
    }
  }

  return out.join('\n').replace(/\n{3,}/g, '\n\n').trim();
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
  const displayContent = isUser ? message.content : formatAssistantText(message.content);

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
        className={`max-w-[85%] text-[16px] leading-7 ${
          isUser
            ? 'bg-[#2C2C2C] px-5 py-3 rounded-3xl rounded-tr-sm'
            : 'text-gray-100 px-0'
        }`}
      >
        {displayContent.split('\n').map((line, i, arr) => (
          <Fragment key={i}>
            {line}
            {i < arr.length - 1 && <br />}
          </Fragment>
        ))}
        {/* Blinking cursor while the assistant is streaming */}
        {isStreaming && (
          <span className="inline-block w-[2px] h-[1em] bg-blue-400 ml-0.5 animate-pulse align-middle" />
        )}
      </div>
    </div>
  );
}
