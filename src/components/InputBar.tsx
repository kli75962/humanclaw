import { forwardRef, memo, useImperativeHandle, useRef, useState } from 'react';
import { Send, PhoneCall } from 'lucide-react';
import type { InputBarProps, InputBarHandle } from '../types';

/**
 * Fixed bottom input bar.
 * - Enter (without Shift) sends the message.
 * - STT (PhoneCall) button is always visible, left of the send button.
 * - Send button turns into a square stop button while the AI is running.
 *
 * Input state is owned internally to avoid re-rendering the entire App on
 * every keystroke.  Parent can read/write via the imperative handle.
 */
export const InputBar = memo(forwardRef<InputBarHandle, InputBarProps>(function InputBar(
  { isThinking, isListening, sttError, onSend, onSttToggle, onStop },
  ref,
) {
  const [value, setValue] = useState('');
  const valueRef = useRef(value);
  valueRef.current = value;

  useImperativeHandle(ref, () => ({
    setInput: (text: string) => setValue(text),
    getInput: () => valueRef.current,
  }), []);

  const handleSend = () => {
    if (!value.trim() || isThinking) return;
    onSend(value);
    setValue('');
  };

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
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && !isThinking && handleSend()}
          placeholder={
            isListening ? 'Listening…' : isThinking ? 'Waiting for response…' : 'Enter a prompt here'
          }
          disabled={isThinking}
          className="flex-1 bg-transparent text-white placeholder-gray-400 px-4 outline-none h-full disabled:opacity-50"
        />

        {/* STT button — always visible */}
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

        {/* Send / Stop button */}
        {isThinking ? (
          <button
            onClick={onStop}
            className="p-3 hover:bg-[#2C2C2C] rounded-full text-red-400 transition-colors"
          >
            {/* Square stop icon */}
            <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor">
              <rect x="4" y="4" width="12" height="12" rx="2" />
            </svg>
          </button>
        ) : (
          <button
            onClick={handleSend}
            disabled={!value.trim()}
            className="p-3 hover:bg-[#2C2C2C] rounded-full text-blue-400 transition-colors disabled:opacity-40"
          >
            <Send size={22} />
          </button>
        )}
      </div>
    </div>
  );
}));
