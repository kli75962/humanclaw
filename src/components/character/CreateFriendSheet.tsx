import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronDown, Image, RefreshCw, X } from 'lucide-react';
import type { Character } from '../../types';
import { SectionHeader } from '../settings/SettingsUI';
import { useLive2DModels } from '../../hooks/useLive2DModels';
import { Live2DPicker } from './Live2DPicker';
import '../../style/CreateFriendSheet.css';
import '../../style/Modal.css';

const PROVIDER_KEY = 'phoneclaw_provider';
const CLAUDE_MODEL_KEY = 'phoneclaw_claude_model';

type Provider = 'claude' | 'ollama';

const CLAUDE_MODELS = [
  { id: 'claude-haiku-4-5-20251001', label: 'Haiku 4.5' },
  { id: 'claude-sonnet-4-6',         label: 'Sonnet 4.6' },
  { id: 'claude-opus-4-6',           label: 'Opus 4.6' },
];

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

/** Resize an image data URL to max 256×256 preserving aspect ratio. */
function resizeImage(dataUrl: string, maxPx = 256): Promise<string> {
  return new Promise((resolve) => {
    const img = new window.Image();
    img.onload = () => {
      const ratio = Math.min(maxPx / img.width, maxPx / img.height, 1);
      const w = Math.round(img.width * ratio);
      const h = Math.round(img.height * ratio);
      const canvas = document.createElement('canvas');
      canvas.width = w;
      canvas.height = h;
      canvas.getContext('2d')!.drawImage(img, 0, 0, w, h);
      resolve(canvas.toDataURL('image/jpeg', 0.82));
    };
    img.src = dataUrl;
  });
}

interface CreateFriendSheetProps {
  onClose: () => void;
  onSave: (data: Omit<Character, 'id' | 'createdAt'>) => void;
  defaultModel?: string;
}

export function CreateFriendSheet({ onClose, onSave, defaultModel = '' }: CreateFriendSheetProps) {
  // ── Icon ──────────────────────────────────────────────────────────────────
  const [icon, setIcon] = useState<string | undefined>(undefined);
  const fileInputRef = useRef<HTMLInputElement>(null);

  function handleFileChange(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async (ev) => {
      const dataUrl = ev.target?.result as string;
      const resized = await resizeImage(dataUrl);
      setIcon(resized);
    };
    reader.readAsDataURL(file);
  }

  // ── Basic fields ──────────────────────────────────────────────────────────
  const [name, setName] = useState('');
  const [background, setBackground] = useState('');
  const [error, setError] = useState('');

  // ── Model picker ──────────────────────────────────────────────────────────
  const [provider, setProvider] = useState<Provider>(
    () => (localStorage.getItem(PROVIDER_KEY) as Provider) ?? 'ollama',
  );
  const [claudeModel, setClaudeModel] = useState(
    () => localStorage.getItem(CLAUDE_MODEL_KEY) ?? 'claude-sonnet-4-6',
  );
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaModel, setOllamaModel] = useState(defaultModel);
  const [ollamaLoading, setOllamaLoading] = useState(false);
  const [modelMenuOpen, setModelMenuOpen] = useState(false);
  const modelMenuRef = useRef<HTMLDivElement>(null);

  function selectedModel() {
    if (provider === 'claude') return claudeModel;
    return ollamaModel;
  }

  function selectedModelLabel() {
    if (provider === 'claude') {
      return CLAUDE_MODELS.find((m) => m.id === claudeModel)?.label ?? claudeModel;
    }
    return ollamaModel || 'Select model…';
  }

  async function loadOllamaModels() {
    setOllamaLoading(true);
    try {
      const models = await invoke<string[]>('list_models');
      setOllamaModels(models);
      if (models.length > 0 && !models.includes(ollamaModel)) setOllamaModel(models[0]);
    } catch {
      // silently ignore
    } finally {
      setOllamaLoading(false);
    }
  }

  useEffect(() => {
    if (provider === 'ollama') loadOllamaModels();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider]);

  useEffect(() => {
    function onDown(e: PointerEvent) {
      if (modelMenuRef.current && !modelMenuRef.current.contains(e.target as Node)) {
        setModelMenuOpen(false);
      }
    }
    window.addEventListener('pointerdown', onDown);
    return () => window.removeEventListener('pointerdown', onDown);
  }, []);

  // ── Live2D model ──────────────────────────────────────────────────────────
  const { models: live2dModels } = useLive2DModels();
  const [live2dModelId, setLive2dModelId] = useState<string | null>(null);

  // ── Persona dropdown ──────────────────────────────────────────────────────
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [persona, setPersona] = useState(FALLBACK_PERSONAS[0]);
  const [personaMenuOpen, setPersonaMenuOpen] = useState(false);
  const personaMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    invoke<string[]>('list_personas')
      .then((list) => { if (list.length > 0) setPersonas(list); })
      .catch(() => {});
  }, []);

  useEffect(() => {
    function onDown(e: PointerEvent) {
      if (personaMenuRef.current && !personaMenuRef.current.contains(e.target as Node)) {
        setPersonaMenuOpen(false);
      }
    }
    window.addEventListener('pointerdown', onDown);
    return () => window.removeEventListener('pointerdown', onDown);
  }, []);

  // ── Save ──────────────────────────────────────────────────────────────────
  function handleSave() {
    if (!name.trim()) { setError('Name is required.'); return; }
    const model = selectedModel();
    if (!model.trim()) { setError('Model is required.'); return; }
    onSave({
      name: name.trim(),
      icon,
      model: model.trim(),
      persona,
      background: background.trim(),
      live2dModelId: live2dModelId ?? null,
    });
  }

  const content = (
    <>
      <div className="friend-sheet-backdrop" onClick={onClose} />
      <div className="friend-sheet">
        <div className="friend-sheet-handle" />
        <div className="friend-sheet-header">
          <span className="friend-sheet-title">New Friend</span>
          <button className="pc-modal-close" onClick={onClose} aria-label="Close">
            <X size={14} className="pc-modal-close-icon" />
          </button>
        </div>

        <div className="friend-sheet-body custom-scrollbar">

          {/* ── Icon ── */}
          <SectionHeader>Icon (optional)</SectionHeader>
          <div className="friend-sheet-icon-row">
            {icon ? (
              <img src={icon} className="friend-sheet-icon-preview" alt="Character icon" />
            ) : (
              <div className="friend-sheet-icon-placeholder">
                <Image size={28} color="var(--color-text-3)" />
              </div>
            )}
            <div className="friend-sheet-icon-btns">
              <button
                type="button"
                className="settings-refresh-btn friend-sheet-icon-choose"
                onClick={() => fileInputRef.current?.click()}
              >
                {icon ? 'Change' : 'Choose image'}
              </button>
              {icon && (
                <button
                  type="button"
                  className="settings-refresh-btn"
                  onClick={() => setIcon(undefined)}
                >
                  Remove
                </button>
              )}
            </div>
            <input
              ref={fileInputRef}
              type="file"
              accept="image/*"
              style={{ display: 'none' }}
              onChange={handleFileChange}
            />
          </div>

          {/* ── Name ── */}
          <SectionHeader>Name</SectionHeader>
          <input
            className="settings-popup-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Hana"
          />

          {/* ── Model ── */}
          <SectionHeader>Model</SectionHeader>
          <div className="settings-provider-row" style={{ marginBottom: 8 }}>
            {(['claude', 'ollama'] as Provider[]).map((p) => (
              <button
                key={p}
                type="button"
                onClick={() => setProvider(p)}
                className={`settings-provider-btn${provider === p ? ' settings-provider-btn--active' : ''}`}
              >
                {p === 'claude' ? 'Claude' : 'Ollama'}
              </button>
            ))}
          </div>

          {provider === 'claude' && (
            <div ref={modelMenuRef} className={`settings-model-menu${modelMenuOpen ? ' settings-model-menu--open' : ''}`}>
              <button
                type="button"
                onClick={() => setModelMenuOpen((v) => !v)}
                className={`settings-model-trigger${modelMenuOpen ? ' settings-model-trigger-open' : ''}`}
              >
                <span className="settings-model-trigger-label">{selectedModelLabel()}</span>
                <ChevronDown size={16} className={`settings-model-trigger-chevron${modelMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
              </button>
              {modelMenuOpen && (
                <div className="settings-model-dropdown">
                  {CLAUDE_MODELS.map((m) => (
                    <button
                      key={m.id}
                      type="button"
                      className={`settings-model-option${claudeModel === m.id ? ' settings-model-option-active' : ''}`}
                      onClick={() => { setClaudeModel(m.id); setModelMenuOpen(false); }}
                    >
                      <span>{m.label}</span>
                      {claudeModel === m.id && <Check size={14} />}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {provider === 'ollama' && (
            <div style={{ display: 'flex', gap: 6, alignItems: 'flex-start' }}>
              {ollamaModels.length > 0 ? (
                <div ref={modelMenuRef} className={`settings-model-menu${modelMenuOpen ? ' settings-model-menu--open' : ''}`} style={{ flex: 1 }}>
                  <button
                    type="button"
                    onClick={() => setModelMenuOpen((v) => !v)}
                    className={`settings-model-trigger${modelMenuOpen ? ' settings-model-trigger-open' : ''}`}
                  >
                    <span className="settings-model-trigger-label">{ollamaModel || 'Select…'}</span>
                    <ChevronDown size={16} className={`settings-model-trigger-chevron${modelMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
                  </button>
                  {modelMenuOpen && (
                    <div className="settings-model-dropdown">
                      {ollamaModels.map((m) => (
                        <button
                          key={m}
                          type="button"
                          className={`settings-model-option${ollamaModel === m ? ' settings-model-option-active' : ''}`}
                          onClick={() => { setOllamaModel(m); setModelMenuOpen(false); }}
                        >
                          <span>{m}</span>
                          {ollamaModel === m && <Check size={14} />}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              ) : (
                <input
                  className="settings-popup-input"
                  style={{ flex: 1 }}
                  value={ollamaModel}
                  onChange={(e) => setOllamaModel(e.target.value)}
                  placeholder="llama3.2:latest"
                />
              )}
              <button
                type="button"
                onClick={loadOllamaModels}
                disabled={ollamaLoading}
                className="settings-refresh-btn"
                title="Refresh model list"
              >
                <RefreshCw size={14} className={ollamaLoading ? 'settings-spin' : ''} />
              </button>
            </div>
          )}

          {/* ── Persona ── */}
          <SectionHeader>Persona</SectionHeader>
          <div ref={personaMenuRef} className={`settings-model-menu${personaMenuOpen ? ' settings-model-menu--open' : ''}`}>
            <button
              type="button"
              onClick={() => setPersonaMenuOpen((v) => !v)}
              className={`settings-model-trigger${personaMenuOpen ? ' settings-model-trigger-open' : ''}`}
            >
              <span className="settings-model-trigger-label">{formatPersonaLabel(persona)}</span>
              <ChevronDown size={16} className={`settings-model-trigger-chevron${personaMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
            </button>
            {personaMenuOpen && (
              <div className="settings-model-dropdown">
                {personas.map((p) => (
                  <button
                    key={p}
                    type="button"
                    className={`settings-model-option${persona === p ? ' settings-model-option-active' : ''}`}
                    onClick={() => { setPersona(p); setPersonaMenuOpen(false); }}
                  >
                    <span>{formatPersonaLabel(p)}</span>
                    {persona === p && <Check size={14} />}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* ── Live2D Character ── */}
          {live2dModels.length > 0 && (
            <>
              <SectionHeader>Live2D Character</SectionHeader>
              <Live2DPicker models={live2dModels} selectedId={live2dModelId} onSelect={setLive2dModelId} />
            </>
          )}

          {/* ── Background ── */}
          <SectionHeader>Background</SectionHeader>
          <textarea
            className="settings-popup-input friend-sheet-textarea"
            value={background}
            onChange={(e) => setBackground(e.target.value)}
            placeholder="works as a barista, uses casual speech, loves coffee"
            rows={3}
          />

          {error && <p className="settings-save-msg--err" style={{ marginTop: 8 }}>{error}</p>}
        </div>

        <div className="friend-sheet-footer">
          <button className="friend-sheet-save-btn" onClick={handleSave}>
            Create Friend
          </button>
        </div>
      </div>
    </>
  );

  return createPortal(content, document.body);
}
