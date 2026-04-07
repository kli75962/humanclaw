import { useEffect, useRef } from 'react';
import { ChatMessage } from '../chat/ChatMessage';
import { InputBar } from '../chat/InputBar';
import type { Message } from '../../types';

interface MemoChatViewProps {
  messages: Message[];
  streaming: boolean;
  streamContent: string;
  onSend: (text: string) => void;
}

/** Renders memo conversation in the main area with a send bar for follow-ups. */
export function MemoChatView({ messages, streaming, streamContent, onSend }: MemoChatViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // Build the display list: confirmed messages + live streaming message if active
  const display: Message[] = streaming
    ? [...messages, { role: 'assistant', content: streamContent }]
    : messages;

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages.length, streamContent]);

  return (
    <>
      <div className="app-content custom-scrollbar">
        <div className="app-messages">
          {display.map((msg, idx) => (
            <ChatMessage
              key={idx}
              message={msg}
              isLastMessage={idx === display.length - 1}
              isThinking={streaming && idx === display.length - 1}
            />
          ))}
          <div ref={scrollRef} />
        </div>
      </div>
      <InputBar
        isThinking={streaming}
        isListening={false}
        sttError={null}
        onSend={onSend}
        onSttToggle={() => {}}
        onStop={() => {}}
      />
    </>
  );
}
