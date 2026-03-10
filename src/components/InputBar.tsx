import { memo } from 'react';
import { Send, PhoneCall } from 'lucide-react';
import type { InputBarProps } from '../types';

/**
 * Fixed bottom input bar.
 * - Pressing Enter (without Shift) sends the message.
 * - PhoneCall icon (right side) toggles STT recording.
 * - Shows a pulsing light-blue background while listening.
 */
export const InputBar = memo(function InputBar({
  value, isThinking, isListening, sttError, onChange, onSend, onSttToggle,
}: InputBarProps) {
  return (
    <div className="w-full bg-[#131314] p-4 pb-6 shrink-0">
      {sttError && (
        <div className="max-w-3xl mx-auto mb-2 text-xs text-red-400 px-4">
          {sttError}
        </div>
      )}
      <div className="max-w-3xl mx-auto bg-[#1E1F20] rounded-full flex items-center px-2 py-2 border border-transparent focus-within:border-gray-600 transition-colors">
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && onSend(value)}
          placeholder={
            isListening ? 'Listening…' : isThinking ? 'Waiting for response…' : 'Enter a prompt here'
          }
          disabled={isThinking}
          className="flex-1 bg-transparent text-white placeholder-gray-400 px-4 outline-none h-full disabled:opacity-50"
        />

        {value.trim() ? (
          <button
            onClick={() => onSend(value)}
            disabled={isThinking}
            className="p-3 hover:bg-[#2C2C2C] rounded-full text-blue-400 transition-colors disabled:opacity-40"
          >
            <Send size={22} />
          </button>
        ) : (
          <button
            onClick={onSttToggle}
            disabled={isThinking}
            className={`p-3 rounded-full transition-colors ${
              isListening
                ? 'text-blue-400 bg-blue-400/15 animate-pulse'
                : 'text-gray-400 hover:bg-[#2C2C2C]'
            } disabled:opacity-40`}
          >
            <PhoneCall size={22} />
          </button>
        )}
      </div>
    </div>
  );
});
