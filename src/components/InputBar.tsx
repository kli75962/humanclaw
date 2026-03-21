import { forwardRef, memo, useImperativeHandle, useRef, useState } from 'react';
import { Send, PhoneCall } from 'lucide-react';
import type { InputBarProps, InputBarHandle } from '../types';
import '../style/InputBar.css';

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
    <div className="inputbar">
      {sttError && (
        <div className="inputbar-error">{sttError}</div>
      )}
      <div className="inputbar-row">
        <input
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && !isThinking && handleSend()}
          placeholder={
            isListening ? 'Listening…' : isThinking ? 'Waiting for response…' : 'Enter a prompt here'
          }
          disabled={isThinking}
          className="inputbar-input"
        />

        <button
          onClick={onSttToggle}
          disabled={isThinking}
          className={`inputbar-btn inputbar-stt-btn${isListening ? ' inputbar-stt-btn--listening' : ''}`}
        >
          <PhoneCall size={22} />
        </button>

        {isThinking ? (
          <button onClick={onStop} className="inputbar-btn inputbar-stop-btn">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="currentColor">
              <rect x="4" y="4" width="12" height="12" rx="2" />
            </svg>
          </button>
        ) : (
          <button
            onClick={handleSend}
            disabled={!value.trim()}
            className="inputbar-btn inputbar-send-btn"
          >
            <Send size={22} />
          </button>
        )}
      </div>
    </div>
  );
}));
