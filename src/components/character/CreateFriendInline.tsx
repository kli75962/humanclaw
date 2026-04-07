import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronDown, ImageIcon, RefreshCw } from 'lucide-react';
import type { Character } from '../../types';
import { BirthdayCalendar } from '../character/BirthdayCalendar';
import { ImageCropperModal } from '../ui/ImageCropperModal';
import '../../style/SettingsScreen.css';

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

function formatPersonaLabel(p: string) {
  return p.replace(/^persona_/, '').split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ');
}

function resizeImage(dataUrl: string, maxPx = 256): Promise<string> {
  return new Promise((resolve) => {
    const img = new window.Image();
    img.onload = () => {
      const ratio = Math.min(maxPx / img.width, maxPx / img.height, 1);
      const canvas = document.createElement('canvas');
      canvas.width = Math.round(img.width * ratio);
      canvas.height = Math.round(img.height * ratio);
      canvas.getContext('2d')!.drawImage(img, 0, 0, canvas.width, canvas.height);
      resolve(canvas.toDataURL('image/jpeg', 0.95));
    };
    img.src = dataUrl;
  });
}

interface Props {
  defaultModel?: string;
  onSave: (data: Omit<Character, 'id' | 'createdAt'>) => void;
  onCancel: () => void;
}

export function CreateFriendInline({ defaultModel = '', onSave, onCancel }: Props) {
  // ── Icon ──────────────────────────────────────────────────────────────────
  const [icon, setIcon] = useState<string | undefined>();
  const [cropperSrc, setCropperSrc] = useState('');
  const fileRef = useRef<HTMLInputElement>(null);

  function handleFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = async (ev) => {
      setCropperSrc(ev.target?.result as string);
    };
    reader.readAsDataURL(file);
  }

  async function handleCropperConfirm(croppedDataUrl: string) {
    const resized = await resizeImage(croppedDataUrl);
    setIcon(resized);
    setCropperSrc('');
  }

  // ── Name + background ─────────────────────────────────────────────────────
  const [name, setName] = useState('');
  const [background, setBackground] = useState('');
  const [error, setError] = useState('');

  // ── Model ─────────────────────────────────────────────────────────────────
  const [provider, setProvider] = useState<Provider>(
    () => (localStorage.getItem(PROVIDER_KEY) as Provider) ?? 'ollama',
  );
  const [claudeModel, setClaudeModel] = useState(
    () => localStorage.getItem(CLAUDE_MODEL_KEY) ?? 'claude-sonnet-4-6',
  );
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaModel, setOllamaModel] = useState(defaultModel);
  const [ollamaLoading, setOllamaLoading] = useState(false);
  const [modelOpen, setModelOpen] = useState(false);
  const modelRef = useRef<HTMLDivElement>(null);

  async function loadOllamaModels() {
    setOllamaLoading(true);
    try {
      const models = await invoke<string[]>('list_models');
      setOllamaModels(models);
      if (models.length > 0 && !models.includes(ollamaModel)) setOllamaModel(models[0]);
    } catch { /* ignore */ } finally { setOllamaLoading(false); }
  }

  useEffect(() => {
    if (provider === 'ollama') loadOllamaModels();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider]);

  // ── Persona ───────────────────────────────────────────────────────────────
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [persona, setPersona] = useState(FALLBACK_PERSONAS[0]);
  const [personaOpen, setPersonaOpen] = useState(false);
  const personaRef = useRef<HTMLDivElement>(null);

  // ── Active Time ────────────────────────────────────────────────────────────
  const [activeTime, setActiveTime] = useState<'early' | 'night' | 'random'>('random');

  // ── Birthday ───────────────────────────────────────────────────────────────
  const [selectedBirthday, setSelectedBirthday] = useState<string | null>(null);
  const [birthdayRandom, setBirthdayRandom] = useState(true);

  useEffect(() => {
    invoke<string[]>('list_personas')
      .then((list) => { if (list.length > 0) setPersonas(list); })
      .catch(() => {});
  }, []);

  // Close dropdowns on outside click
  useEffect(() => {
    function onDown(e: PointerEvent) {
      if (modelRef.current && !modelRef.current.contains(e.target as Node)) setModelOpen(false);
      if (personaRef.current && !personaRef.current.contains(e.target as Node)) setPersonaOpen(false);
    }
    window.addEventListener('pointerdown', onDown);
    return () => window.removeEventListener('pointerdown', onDown);
  }, []);

  function currentModel() {
    return provider === 'claude' ? claudeModel : ollamaModel;
  }

  function currentModelLabel() {
    if (provider === 'claude') return CLAUDE_MODELS.find((m) => m.id === claudeModel)?.label ?? claudeModel;
    return ollamaModel || 'Select…';
  }

  function handleSave() {
    if (!name.trim()) { setError('Name is required.'); return; }
    if (!currentModel().trim()) { setError('Model is required.'); return; }

    onSave({
      name: name.trim(),
      icon,
      model: currentModel().trim(),
      persona,
      background: background.trim(),
      activeTime: activeTime === 'random' ? undefined : activeTime,
      birthday: birthdayRandom ? undefined : selectedBirthday || undefined,
    });
  }

  return (
    <>
      {cropperSrc && (
        <ImageCropperModal
          src={cropperSrc}
          aspectRatio={1}
          onConfirm={handleCropperConfirm}
          onCancel={() => setCropperSrc('')}
          title="Crop Character Icon"
        />
      )}

      <div className="friend-inline-form">
        {/* Icon */}
        <p className="settings-modal-field-label" style={{ marginTop: 0 }}>Icon (optional)</p>
        <div style={{ marginBottom: 12 }}>
          <div
            className="friend-inline-icon-picker"
            onClick={() => {
              if (!icon) fileRef.current?.click();
            }}
          >
            {icon ? (
              <div
                className="friend-inline-icon-preview-container"
                style={{ backgroundImage: `url(${icon})` }}
                onClick={() => setIcon(undefined)}
              >
                <button
                  type="button"
                  className="friend-inline-icon-remove-overlay"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setIcon(undefined);
                  }}
                >
                  Remove
                </button>
              </div>
            ) : (
              <div className="friend-inline-icon-empty">
                <ImageIcon size={32} color="var(--color-text-3)" />
                <button
                  type="button"
                  className="friend-inline-icon-set-overlay"
                  onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    fileRef.current?.click();
                  }}
                >
                  Set Icon
                </button>
              </div>
            )}
          </div>
          <input ref={fileRef} type="file" accept="image/*" style={{ display: 'none' }} onChange={handleFile} />
        </div>

      {/* Name */}
      <p className="settings-modal-field-label">Name</p>
      <input
        className="settings-popup-input"
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Hana"
        style={{ marginTop: 6 }}
      />

      {/* Model */}
      <p className="settings-modal-field-label">Model</p>
      <div className="settings-provider-row" style={{ marginBottom: 6 }}>
        {(['claude', 'ollama'] as Provider[]).map((p) => (
          <button key={p} type="button"
            onClick={() => setProvider(p)}
            className={`settings-provider-btn${provider === p ? ' settings-provider-btn--active' : ''}`}
          >
            {p === 'claude' ? 'Claude' : 'Ollama'}
          </button>
        ))}
      </div>

      {provider === 'claude' && (
        <div ref={modelRef} className={`settings-model-menu${modelOpen ? ' settings-model-menu--open' : ''}`}>
          <button type="button" onClick={() => setModelOpen((v) => !v)}
            className={`settings-model-trigger${modelOpen ? ' settings-model-trigger-open' : ''}`}>
            <span className="settings-model-trigger-label">{currentModelLabel()}</span>
            <ChevronDown size={16} className={`settings-model-trigger-chevron${modelOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
          </button>
          {modelOpen && (
            <div className="settings-model-dropdown">
              {CLAUDE_MODELS.map((m) => (
                <button key={m.id} type="button"
                  className={`settings-model-option${claudeModel === m.id ? ' settings-model-option-active' : ''}`}
                  onClick={() => { setClaudeModel(m.id); setModelOpen(false); }}>
                  <span>{m.label}</span>
                  {claudeModel === m.id && <Check size={14} />}
                </button>
              ))}
            </div>
          )}
        </div>
      )}

      {provider === 'ollama' && (
        <div style={{ display: 'flex', gap: 6 }}>
          {ollamaModels.length > 0 ? (
            <div ref={modelRef} className={`settings-model-menu${modelOpen ? ' settings-model-menu--open' : ''}`} style={{ flex: 1 }}>
              <button type="button" onClick={() => setModelOpen((v) => !v)}
                className={`settings-model-trigger${modelOpen ? ' settings-model-trigger-open' : ''}`}>
                <span className="settings-model-trigger-label">{ollamaModel || 'Select…'}</span>
                <ChevronDown size={16} className={`settings-model-trigger-chevron${modelOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
              </button>
              {modelOpen && (
                <div className="settings-model-dropdown">
                  {ollamaModels.map((m) => (
                    <button key={m} type="button"
                      className={`settings-model-option${ollamaModel === m ? ' settings-model-option-active' : ''}`}
                      onClick={() => { setOllamaModel(m); setModelOpen(false); }}>
                      <span>{m}</span>
                      {ollamaModel === m && <Check size={14} />}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <input className="settings-popup-input" style={{ flex: 1, marginTop: 0 }}
              value={ollamaModel} onChange={(e) => setOllamaModel(e.target.value)}
              placeholder="llama3.2:latest" />
          )}
          <button type="button" onClick={loadOllamaModels} disabled={ollamaLoading}
            className="settings-refresh-btn" title="Refresh models">
            <RefreshCw size={14} className={ollamaLoading ? 'settings-spin' : ''} />
          </button>
        </div>
      )}

      {/* Persona */}
      <p className="settings-modal-field-label">Persona</p>
      <div ref={personaRef} className={`settings-model-menu${personaOpen ? ' settings-model-menu--open' : ''}`} style={{ marginTop: 6 }}>
        <button type="button" onClick={() => setPersonaOpen((v) => !v)}
          className={`settings-model-trigger${personaOpen ? ' settings-model-trigger-open' : ''}`}>
          <span className="settings-model-trigger-label">{formatPersonaLabel(persona)}</span>
          <ChevronDown size={16} className={`settings-model-trigger-chevron${personaOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
        </button>
        {personaOpen && (
          <div className="settings-model-dropdown">
            {personas.map((p) => (
              <button key={p} type="button"
                className={`settings-model-option${persona === p ? ' settings-model-option-active' : ''}`}
                onClick={() => { setPersona(p); setPersonaOpen(false); }}>
                <span>{formatPersonaLabel(p)}</span>
                {persona === p && <Check size={14} />}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Background */}
      <p className="settings-modal-field-label">Background</p>
      <textarea
        className="settings-popup-input"
        value={background}
        onChange={(e) => setBackground(e.target.value)}
        placeholder="works as a barista, uses casual speech, loves coffee"
        rows={3}
        style={{ marginTop: 6, resize: 'vertical', minHeight: 64, fontFamily: 'ui-sans-serif, system-ui, sans-serif' }}
      />

      {/* Active Time */}
      <p className="settings-modal-field-label">Active Time</p>
      <div className="settings-provider-row" style={{ marginTop: 6, marginBottom: 12 }}>
        {(['early', 'night', 'random'] as const).map((t) => (
          <button key={t} type="button"
            onClick={() => setActiveTime(t)}
            className={`settings-provider-btn${activeTime === t ? ' settings-provider-btn--active' : ''}`}
            style={{ flex: 1 }}
          >
            {t === 'early' ? '🌅 Early' : t === 'night' ? '🌙 Night' : '🎲 Random'}
          </button>
        ))}
      </div>

      {/* Birthday */}
      <p className="settings-modal-field-label">Birthday</p>
      <div style={{ marginTop: 6, marginBottom: 12 }}>
        <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
          <button
            type="button"
            onClick={() => setBirthdayRandom(true)}
            style={{
              flex: 1,
              padding: '8px 12px',
              borderRadius: 4,
              border: birthdayRandom ? '2px solid var(--color-primary)' : '1px solid var(--color-border)',
              backgroundColor: birthdayRandom ? 'var(--color-bg-secondary)' : 'transparent',
              color: 'var(--color-text-1)',
              cursor: 'pointer',
              fontSize: 12,
              fontWeight: 500,
            }}
          >
            🎲 Random
          </button>
          <button
            type="button"
            onClick={() => setBirthdayRandom(false)}
            style={{
              flex: 1,
              padding: '8px 12px',
              borderRadius: 4,
              border: !birthdayRandom ? '2px solid var(--color-primary)' : '1px solid var(--color-border)',
              backgroundColor: !birthdayRandom ? 'var(--color-bg-secondary)' : 'transparent',
              color: 'var(--color-text-1)',
              cursor: 'pointer',
              fontSize: 12,
              fontWeight: 500,
            }}
          >
            📅 Specific
          </button>
        </div>

        {!birthdayRandom && (
          <BirthdayCalendar value={selectedBirthday} onChange={setSelectedBirthday} />
        )}
      </div>

      {error && <p className="settings-save-msg--err" style={{ marginTop: 12, fontSize: 12 }}>{error}</p>}

      {/* Actions */}
      <div className="friend-inline-actions">
        <button type="button" className="friend-inline-cancel-btn" onClick={onCancel}>Cancel</button>
        <button type="button" className="friend-inline-save-btn" onClick={handleSave}>Create</button>
      </div>
      </div>
    </>
  );
}
