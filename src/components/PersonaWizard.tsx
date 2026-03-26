import { useState } from 'react';
import type { WizardAnswers } from '../types';
import '../style/PersonaWizard.css';

interface StepOption {
  label: string;
  value: string;
  isTextInput?: boolean;
}

interface StepConfig {
  key: keyof WizardAnswers;
  question: string;
  options: StepOption[];
}

const STEPS: StepConfig[] = [
  {
    key: 'sex',
    question: 'Gender of this persona?',
    options: [
      { label: 'Male', value: 'male' },
      { label: 'Female', value: 'female' },
      { label: 'Random', value: 'random' },
    ],
  },
  {
    key: 'personality',
    question: 'Personality type?',
    options: [
      { label: 'Introvert', value: 'introvert' },
      { label: 'Extroverted', value: 'extroverted' },
      { label: 'Neutral', value: 'neutral' },
      { label: 'Random', value: 'random' },
    ],
  },
  {
    key: 'profession',
    question: 'Profession?',
    options: [
      { label: 'Random', value: 'random' },
      { label: 'Type it...', value: '__text__', isTextInput: true },
    ],
  },
  {
    key: 'personaName',
    question: 'Persona name?',
    options: [
      { label: 'Let LLM decide', value: 'random' },
      { label: 'Type it...', value: '__text__', isTextInput: true },
    ],
  },
];

export function PersonaWizard({ onComplete }: { onComplete: (answers: WizardAnswers) => void }) {
  const [currentIdx, setCurrentIdx] = useState(0);
  const [answers, setAnswers] = useState<Partial<WizardAnswers>>({});
  const [textValue, setTextValue] = useState('');
  const [textActive, setTextActive] = useState(false);

  const step = STEPS[currentIdx];

  function commit(value: string) {
    const next = { ...answers, [step.key]: value };
    setAnswers(next);
    setTextValue('');
    setTextActive(false);

    if (currentIdx < STEPS.length - 1) {
      setCurrentIdx(currentIdx + 1);
    } else {
      onComplete({
        sex: next.sex ?? 'random',
        personality: next.personality ?? 'random',
        profession: next.profession ?? 'random',
        personaName: next.personaName ?? 'random',
      });
    }
  }

  function handleOption(opt: StepOption) {
    if (opt.isTextInput) {
      setTextActive(true);
    } else {
      commit(opt.value);
    }
  }

  return (
    <div className="persona-wizard">
      {/* Completed steps */}
      {STEPS.slice(0, currentIdx).map((s) => (
        <div key={s.key} className="wizard-done-row">
          <span className="wizard-done-q">{s.question}</span>
          <span className="wizard-done-a">{answers[s.key]}</span>
        </div>
      ))}

      {/* Active step bubble */}
      <div className="wizard-bubble">
        <p className="wizard-question">{step.question}</p>
        <div className="wizard-options">
          {step.options.map((opt) =>
            opt.isTextInput ? (
              textActive ? (
                <div key={opt.value} className="wizard-text-row">
                  <input
                    autoFocus
                    className="wizard-text-input"
                    value={textValue}
                    onChange={(e) => setTextValue(e.target.value)}
                    onKeyDown={(e) => e.key === 'Enter' && textValue.trim() && commit(textValue.trim())}
                    placeholder="Type here..."
                  />
                  <button
                    className="wizard-confirm-btn"
                    disabled={!textValue.trim()}
                    onClick={() => commit(textValue.trim())}
                  >
                    OK
                  </button>
                </div>
              ) : (
                <button key={opt.value} className="wizard-option-btn" onClick={() => handleOption(opt)}>
                  {opt.label}
                </button>
              )
            ) : (
              <button key={opt.value} className="wizard-option-btn" onClick={() => handleOption(opt)}>
                {opt.label}
              </button>
            )
          )}
        </div>
      </div>
    </div>
  );
}
