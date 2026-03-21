import { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown, Cpu } from 'lucide-react';
import type { SessionConfig } from '../types';
import { Card, SectionFooter, SectionHeader } from './SettingsUI';

const FALLBACK_PERSONAS = [
  'persona_default',
  'persona_jk',
  'persona_jobs_professional',
  'persona_mentor',
  'persona_concise',
];

function formatPersonaLabel(persona: string): string {
  return persona
    .replace(/^persona_/, '')
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

interface GeneralTabProps {
  model: string;
  availableModels: string[];
  onModelChange: (m: string) => void;
  session: SessionConfig | null;
  listPersonas: () => Promise<string[]>;
  setPersona: (persona: string) => Promise<SessionConfig>;
}

export function GeneralTab({ model, availableModels, onModelChange, session, listPersonas, setPersona }: GeneralTabProps) {
  const [isModelMenuOpen, setIsModelMenuOpen] = useState(false);
  const [isPersonaMenuOpen, setIsPersonaMenuOpen] = useState(false);
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [personaSaveMsg, setPersonaSaveMsg] = useState('');
  const modelMenuRef = useRef<HTMLDivElement>(null);
  const personaMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listPersonas()
      .then((names) => { if (names.length > 0) setPersonas(names); })
      .catch(() => setPersonas(FALLBACK_PERSONAS));
  }, [listPersonas]);

  useEffect(() => {
    function onPointerDown(event: PointerEvent) {
      if (modelMenuRef.current && !modelMenuRef.current.contains(event.target as Node)) {
        setIsModelMenuOpen(false);
      }
      if (personaMenuRef.current && !personaMenuRef.current.contains(event.target as Node)) {
        setIsPersonaMenuOpen(false);
      }
    }
    window.addEventListener('pointerdown', onPointerDown);
    return () => window.removeEventListener('pointerdown', onPointerDown);
  }, []);

  return (
    <>
      <SectionHeader>Model</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <div className="settings-item-header">
            <div className="settings-icon-badge settings-icon-badge--indigo">
              <Cpu size={18} />
            </div>
            <div className="settings-item-info">
              <p className="settings-item-title">Active model</p>
              <p className="settings-item-subtitle">Select model for chat requests</p>
            </div>
          </div>

          <div ref={modelMenuRef} className="settings-model-menu">
            <button
              type="button"
              onClick={() => setIsModelMenuOpen((v) => !v)}
              className={`settings-model-trigger${isModelMenuOpen ? ' settings-model-trigger-open' : ''}`}
            >
              <span className="settings-model-trigger-label">{model || 'No model selected'}</span>
              <ChevronDown size={16} className={`settings-model-trigger-chevron${isModelMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
            </button>

            {isModelMenuOpen && (
              <div className="settings-model-dropdown">
                {availableModels.length === 0 ? (
                  <button type="button" className="settings-model-option" disabled>
                    <span>No models found</span>
                  </button>
                ) : (
                  availableModels.map((m) => (
                    <button
                      key={m}
                      type="button"
                      className={`settings-model-option${m === model ? ' settings-model-option-active' : ''}`}
                      onClick={() => { onModelChange(m); setIsModelMenuOpen(false); }}
                    >
                      <span>{m}</span>
                      {m === model && <Check size={14} />}
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
        </div>
      </Card>
      <SectionFooter>Models are loaded from your local Ollama instance.</SectionFooter>

      <SectionHeader>Persona</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <div className="settings-item-header">
            <div className="settings-icon-badge settings-icon-badge--emerald">
              <Cpu size={18} />
            </div>
            <div className="settings-item-info">
              <p className="settings-item-title">Assistant persona</p>
              <p className="settings-item-subtitle">Choose response style and character</p>
            </div>
          </div>

          <div ref={personaMenuRef} className="settings-model-menu">
            <button
              type="button"
              onClick={() => setIsPersonaMenuOpen((v) => !v)}
              className={`settings-model-trigger${isPersonaMenuOpen ? ' settings-model-trigger-open' : ''}`}
            >
              <span className="settings-model-trigger-label">
                {formatPersonaLabel(session?.persona || 'persona_default')}
              </span>
              <ChevronDown size={16} className={`settings-model-trigger-chevron${isPersonaMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
            </button>

            {isPersonaMenuOpen && (
              <div className="settings-model-dropdown">
                {personas.length === 0 ? (
                  <button type="button" className="settings-model-option" disabled>
                    <span>No personas found</span>
                  </button>
                ) : (
                  personas.map((persona) => (
                    <button
                      key={persona}
                      type="button"
                      className={`settings-model-option${persona === session?.persona ? ' settings-model-option-active' : ''}`}
                      onClick={async () => {
                        try {
                          await setPersona(persona);
                          setPersonaSaveMsg('Saved');
                          setTimeout(() => setPersonaSaveMsg(''), 1800);
                        } catch (e) {
                          setPersonaSaveMsg(e instanceof Error ? e.message : String(e));
                        } finally {
                          setIsPersonaMenuOpen(false);
                        }
                      }}
                    >
                      <span>{formatPersonaLabel(persona)}</span>
                      {persona === session?.persona && <Check size={14} />}
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
        </div>
      </Card>
      <SectionFooter>
        Persona controls the assistant character and tone used during tool-driven chat.
        {personaSaveMsg ? ` ${personaSaveMsg}` : ''}
      </SectionFooter>
    </>
  );
}
