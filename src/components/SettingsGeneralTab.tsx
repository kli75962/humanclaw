import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronDown, ChevronRight, Cpu, RefreshCw } from 'lucide-react';
import type { SessionConfig } from '../types';
import { Card, SectionFooter, SectionHeader } from './SettingsUI';
import { Modal } from './Modal';

const PROVIDER_KEY = 'phoneclaw_provider';
const CLAUDE_MODEL_KEY = 'phoneclaw_claude_model';

const FALLBACK_PERSONAS = [
  'persona_default',
  'persona_jk',
  'persona_jobs_professional',
  'persona_mentor',
  'persona_concise',
];

const CLAUDE_MODELS = [
  { id: 'claude-haiku-4-5-20251001', label: 'Haiku 4.5' },
  { id: 'claude-sonnet-4-6',         label: 'Sonnet 4.6' },
  { id: 'claude-opus-4-6',           label: 'Opus 4.6' },
];

type Provider = 'claude' | 'ollama';

function formatPersonaLabel(persona: string): string {
  return persona
    .replace(/^persona_/, '')
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

// ── Model config modal ─────────────────────────────────────────────────────────

interface ModelConfigModalProps {
  session: SessionConfig | null;
  currentModel: string;
  onModelChange: (m: string) => void;
  setOllamaEndpoint: (host: string, port: number) => Promise<SessionConfig>;
  onOllamaEndpointChanged: () => void;
  onClose: () => void;
  onSaved: (provider: Provider, model: string) => void;
}

function ModelConfigModal({
  session,
  currentModel,
  onModelChange,
  setOllamaEndpoint,
  onOllamaEndpointChanged,
  onClose,
  onSaved,
}: ModelConfigModalProps) {
  const [provider, setProvider] = useState<Provider>(
    () => (localStorage.getItem(PROVIDER_KEY) as Provider) ?? 'ollama',
  );
  const [claudeApiKey, setClaudeApiKey] = useState('');
  const [claudeModel, setClaudeModel] = useState(
    () => localStorage.getItem(CLAUDE_MODEL_KEY) ?? 'claude-sonnet-4-6',
  );
  const [ollamaHostPort, setOllamaHostPort] = useState(() => {
    const host = (session?.ollama_host_override ?? '').trim() || '127.0.0.1';
    const port = session?.ollama_port ?? 11434;
    return `${host}:${port}`;
  });
  const [ollamaModel, setOllamaModel] = useState(currentModel);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaModelsLoading, setOllamaModelsLoading] = useState(false);
  const [ollamaModelsError, setOllamaModelsError] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState('');

  useEffect(() => {
    invoke<string | null>('load_secret', { key: 'claude_api_key' })
      .then((val) => { if (val) setClaudeApiKey(val); })
      .catch(() => {});
  }, []);

  // Fetch Ollama models when provider is ollama or when IP:Port changes
  async function fetchOllamaModels(hostPort: string) {
    const colonIdx = hostPort.lastIndexOf(':');
    const host = colonIdx > 0 ? hostPort.slice(0, colonIdx).trim() : hostPort.trim();
    const port = colonIdx > 0 ? parseInt(hostPort.slice(colonIdx + 1), 10) : 11434;
    if (!host || !Number.isFinite(port)) return;

    setOllamaModelsLoading(true);
    setOllamaModelsError('');
    try {
      const models = await invoke<string[]>('list_models_at', { host, port });
      setOllamaModels(models);
      if (models.length > 0 && !models.includes(ollamaModel)) {
        setOllamaModel(models[0]);
      }
    } catch (e) {
      setOllamaModelsError(e instanceof Error ? e.message : String(e));
    } finally {
      setOllamaModelsLoading(false);
    }
  }

  useEffect(() => {
    if (provider === 'ollama') {
      fetchOllamaModels(ollamaHostPort);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider]);

  async function handleSave() {
    setSaving(true);
    setSaveMsg('');
    try {
      localStorage.setItem(PROVIDER_KEY, provider);

      if (provider === 'claude') {
        if (claudeApiKey.trim()) {
          await invoke('store_secret', { key: 'claude_api_key', value: claudeApiKey.trim() });
        }
        localStorage.setItem(CLAUDE_MODEL_KEY, claudeModel);
        onModelChange(claudeModel);
        onSaved('claude', claudeModel);
      } else {
        const colonIdx = ollamaHostPort.lastIndexOf(':');
        const host = colonIdx > 0 ? ollamaHostPort.slice(0, colonIdx).trim() : ollamaHostPort.trim();
        const port = colonIdx > 0 ? parseInt(ollamaHostPort.slice(colonIdx + 1), 10) : 11434;
        if (!host || !Number.isFinite(port) || port < 1 || port > 65535) {
          setSaveMsg('Invalid IP:Port');
          return;
        }
        await setOllamaEndpoint(host, port);
        onOllamaEndpointChanged();
        onModelChange(ollamaModel);
        onSaved('ollama', ollamaModel);
      }

      setSaveMsg('Saved');
      setTimeout(onClose, 700);
    } catch (e) {
      setSaveMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="settings-edit-modal-body">
      <p className="settings-modal-field-label" style={{ marginTop: 0 }}>Provider</p>
      <div className="settings-provider-row">
        {(['claude', 'ollama'] as Provider[]).map((p) => (
          <button
            key={p}
            onClick={() => setProvider(p)}
            className={`settings-provider-btn${provider === p ? ' settings-provider-btn--active' : ''}`}
          >
            {p === 'claude' ? 'Claude' : 'Ollama'}
          </button>
        ))}
      </div>

      {provider === 'claude' && (
        <>
          <p className="settings-modal-field-label">API Key</p>
          <input
            type="password"
            value={claudeApiKey}
            onChange={(e) => setClaudeApiKey(e.target.value)}
            placeholder="sk-ant-..."
            autoComplete="off"
            className="settings-popup-input"
            style={{ marginTop: 6 }}
          />
          <p className="settings-modal-field-label">Model</p>
          <select
            value={claudeModel}
            onChange={(e) => setClaudeModel(e.target.value)}
            className="settings-popup-input settings-popup-select"
            style={{ marginTop: 6 }}
          >
            {CLAUDE_MODELS.map((m) => (
              <option key={m.id} value={m.id}>{m.label}</option>
            ))}
          </select>
        </>
      )}

      {provider === 'ollama' && (
        <>
          <p className="settings-modal-field-label">IP:Port</p>
          <div className="settings-hostport-row">
            <input
              value={ollamaHostPort}
              onChange={(e) => setOllamaHostPort(e.target.value)}
              placeholder="127.0.0.1:11434"
              className="settings-popup-input"
              style={{ marginTop: 6, flex: 1 }}
            />
            <button
              type="button"
              onClick={() => fetchOllamaModels(ollamaHostPort)}
              disabled={ollamaModelsLoading}
              className="settings-refresh-btn"
              title="Refresh model list"
            >
              <RefreshCw size={14} className={ollamaModelsLoading ? 'settings-spin' : ''} />
            </button>
          </div>
          <p className="settings-modal-field-label">Model</p>
          {ollamaModelsError ? (
            <p className="settings-save-msg--err" style={{ marginTop: 6, fontSize: 11 }}>
              {ollamaModelsError}
            </p>
          ) : null}
          {ollamaModels.length > 0 ? (
            <select
              value={ollamaModel}
              onChange={(e) => setOllamaModel(e.target.value)}
              className="settings-popup-input settings-popup-select"
              style={{ marginTop: 6 }}
            >
              {ollamaModels.map((m) => (
                <option key={m} value={m}>{m}</option>
              ))}
            </select>
          ) : (
            <input
              value={ollamaModel}
              onChange={(e) => setOllamaModel(e.target.value)}
              placeholder="llama3.2:latest"
              className="settings-popup-input"
              style={{ marginTop: 6 }}
            />
          )}
        </>
      )}

      <div className="settings-edit-modal-actions">
        <p className={saveMsg === 'Saved' ? 'settings-save-msg--ok' : 'settings-save-msg--err'}>
          {saveMsg || ' '}
        </p>
        <button onClick={handleSave} disabled={saving} className="settings-save-btn">
          {saving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  );
}

// ── GeneralTab ─────────────────────────────────────────────────────────────────

interface GeneralTabProps {
  model: string;
  availableModels: string[];
  onModelChange: (m: string) => void;
  session: SessionConfig | null;
  listPersonas: () => Promise<string[]>;
  setPersona: (persona: string) => Promise<SessionConfig>;
  setOllamaEndpoint: (host: string, port: number) => Promise<SessionConfig>;
  onOllamaEndpointChanged: () => void;
}

export function GeneralTab({
  model,
  availableModels: _availableModels,
  onModelChange,
  session,
  listPersonas,
  setPersona,
  setOllamaEndpoint,
  onOllamaEndpointChanged,
}: GeneralTabProps) {
  const [showModelConfig, setShowModelConfig] = useState(false);
  const [activeProvider, setActiveProvider] = useState<Provider>(
    () => (localStorage.getItem(PROVIDER_KEY) as Provider) ?? 'ollama',
  );

  const [isPersonaMenuOpen, setIsPersonaMenuOpen] = useState(false);
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [personaSaveMsg, setPersonaSaveMsg] = useState('');
  const personaMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listPersonas()
      .then((names) => { if (names.length > 0) setPersonas(names); })
      .catch(() => setPersonas(FALLBACK_PERSONAS));
  }, [listPersonas]);

  useEffect(() => {
    function onPointerDown(event: PointerEvent) {
      if (personaMenuRef.current && !personaMenuRef.current.contains(event.target as Node)) {
        setIsPersonaMenuOpen(false);
      }
    }
    window.addEventListener('pointerdown', onPointerDown);
    return () => window.removeEventListener('pointerdown', onPointerDown);
  }, []);

  return (
    <>
      {showModelConfig && (
        <Modal title="Model Configuration" onClose={() => setShowModelConfig(false)}>
          <ModelConfigModal
            session={session}
            currentModel={model}
            onModelChange={onModelChange}
            setOllamaEndpoint={setOllamaEndpoint}
            onOllamaEndpointChanged={onOllamaEndpointChanged}
            onClose={() => setShowModelConfig(false)}
            onSaved={(provider) => setActiveProvider(provider)}
          />
        </Modal>
      )}

      <SectionHeader>Model</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <div className="settings-item-header">
            <div className="settings-icon-badge settings-icon-badge--indigo">
              <Cpu size={18} />
            </div>
            <div className="settings-item-info">
              <p className="settings-item-title">Active model</p>
              <p className="settings-item-subtitle">Provider and model for chat requests</p>
            </div>
          </div>

          <button className="settings-model-config-btn" onClick={() => setShowModelConfig(true)}>
            <div className="settings-model-config-btn-left">
              <span className="settings-model-config-provider">
                {activeProvider === 'claude' ? 'Claude' : 'Ollama'}
              </span>
              <span className="settings-model-config-model">{model || 'Not configured'}</span>
            </div>
            <ChevronRight size={16} style={{ color: '#64748b', flexShrink: 0 }} />
          </button>
        </div>
      </Card>
      <SectionFooter>
        {activeProvider === 'claude'
          ? 'Using Claude API. Model and API key are stored securely.'
          : 'Using local Ollama instance.'}
      </SectionFooter>

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
