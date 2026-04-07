import { useState } from 'react';
import type { WizardAnswers } from '../../types';
import '../../style/PersonaWizard.css';

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
    question: 'Gender?',
    options: [
      { label: 'Male', value: 'male' },
      { label: 'Female', value: 'female' },
      { label: 'Random', value: 'random' },
    ],
  },
  {
    key: 'ageRange',
    question: 'Age range?',
    options: [
      { label: 'Teen', value: 'teen' },
      { label: '20s', value: '20s' },
      { label: '30s', value: '30s' },
      { label: '40s+', value: '40s+' },
      { label: 'Random', value: 'random' },
    ],
  },
  {
    key: 'vibe',
    question: 'Overall vibe?',
    options: [
      { label: 'Chill & quiet', value: 'chill and quiet' },
      { label: 'Warm & chatty', value: 'warm and chatty' },
      { label: 'Sharp & witty', value: 'sharp and witty' },
      { label: 'Deep & intense', value: 'deep and intense' },
      { label: 'Random', value: 'random' },
    ],
  },
  {
    key: 'world',
    question: 'Their world?',
    options: [
      { label: 'Student life', value: 'student life' },
      { label: 'Working adult', value: 'working adult' },
      { label: 'Creative', value: 'creative' },
      { label: 'Otaku', value: 'otaku' },
      { label: 'Random', value: 'random' },
      { label: 'Type it...', value: '__text__', isTextInput: true },
    ],
  },
  {
    key: 'connectsBy',
    question: 'How do they connect with people?',
    options: [
      { label: 'Caring & supportive', value: 'caring and supportive' },
      { label: 'Teasing & playful', value: 'teasing and playful' },
      { label: 'Direct & honest', value: 'direct and honest' },
      { label: 'Curious & questioning', value: 'curious and questioning' },
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
        ageRange: next.ageRange ?? 'random',
        vibe: next.vibe ?? 'random',
        world: next.world ?? 'random',
        connectsBy: next.connectsBy ?? 'random',
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
