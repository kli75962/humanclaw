import { useState, useRef, KeyboardEvent } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChevronLeft, ChevronRight, Send } from 'lucide-react';
import '../../style/AskUserBubble.css';

export interface AskQuestion {
  question: string;
  options: string[];
}

interface AskUserBubbleProps {
  id: string;
  questions: AskQuestion[];
  onDone: () => void;
}

export function AskUserBubble({ id, questions, onDone }: AskUserBubbleProps) {
  const [currentIdx, setCurrentIdx] = useState(0);
  const [answers, setAnswers] = useState<Record<number, string>>({});
  const [inputText, setInputText] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  const total = questions.length;
  const current = questions[currentIdx];
  if (!current) return null;

  // Button options = all except last; last becomes the text input hint
  const buttonOptions = current.options.slice(0, -1);
  const inputPlaceholder = current.options[current.options.length - 1] ?? 'Type your answer...';

  const isAnswered = (idx: number) => answers[idx] !== undefined;
  const currentAnswered = isAnswered(currentIdx);
  const allAnswered = questions.every((_, i) => isAnswered(i));

  async function submitAll(finalAnswers: Record<number, string>) {
    await invoke('respond_ask_user', { id, answers: finalAnswers });
    onDone();
  }

  function selectOption(option: string) {
    const next = { ...answers, [currentIdx]: option };
    setAnswers(next);
    setInputText('');

    if (currentIdx < total - 1) {
      setCurrentIdx(currentIdx + 1);
    } else {
      // Last question answered via button — auto-submit
      submitAll(next);
    }
  }

  function commitInput() {
    const text = inputText.trim();
    if (!text) return;
    const next = { ...answers, [currentIdx]: text };
    setAnswers(next);
    setInputText('');

    if (currentIdx < total - 1) {
      setCurrentIdx(currentIdx + 1);
    } else {
      submitAll(next);
    }
  }

  function handleInputKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commitInput();
    }
  }

  function goBack() {
    if (currentIdx > 0) {
      setCurrentIdx(currentIdx - 1);
      setInputText('');
    }
  }

  function goForward() {
    if (currentAnswered && currentIdx < total - 1) {
      setCurrentIdx(currentIdx + 1);
      setInputText('');
    }
  }

  return (
    <div className="ask-user-bubble">
      {/* Navigation header */}
      <div className="ask-user-header">
        <button
          className="ask-user-nav-btn"
          onClick={goBack}
          disabled={currentIdx === 0}
          aria-label="Previous question"
        >
          <ChevronLeft size={16} />
        </button>

        <span className="ask-user-question">{current.question}</span>

        <button
          className="ask-user-nav-btn"
          onClick={goForward}
          disabled={!currentAnswered || currentIdx === total - 1}
          aria-label="Next question"
        >
          <ChevronRight size={16} />
        </button>
      </div>

      {/* Progress dots (only when multiple questions) */}
      {total > 1 && (
        <div className="ask-user-dots">
          {questions.map((_, i) => (
            <span
              key={i}
              className={`ask-user-dot${i === currentIdx ? ' ask-user-dot--active' : ''}${isAnswered(i) ? ' ask-user-dot--done' : ''}`}
            />
          ))}
        </div>
      )}

      {/* Option buttons */}
      {buttonOptions.length > 0 && (
        <div className="ask-user-options">
          {buttonOptions.map((opt, i) => (
            <button
              key={i}
              className={`ask-user-opt-btn${answers[currentIdx] === opt ? ' ask-user-opt-btn--selected' : ''}`}
              onClick={() => selectOption(opt)}
            >
              {opt}
            </button>
          ))}
        </div>
      )}

      {/* Text input (always shown, last option as placeholder) */}
      <div className="ask-user-input-row">
        <input
          ref={inputRef}
          className="ask-user-input"
          type="text"
          placeholder={inputPlaceholder}
          value={inputText}
          onChange={(e) => setInputText(e.target.value)}
          onKeyDown={handleInputKey}
        />
        <button
          className="ask-user-send-btn"
          onClick={commitInput}
          disabled={!inputText.trim()}
          aria-label="Submit answer"
        >
          <Send size={15} />
        </button>
      </div>

      {/* Current answer preview */}
      {answers[currentIdx] !== undefined && (
        <div className="ask-user-answer-preview">
          Selected: <strong>{answers[currentIdx]}</strong>
        </div>
      )}

      {/* Submit button when all answered and last question uses text input flow */}
      {allAnswered && (
        <button className="ask-user-submit-btn" onClick={() => submitAll(answers)}>
          Submit all answers
        </button>
      )}
    </div>
  );
}
