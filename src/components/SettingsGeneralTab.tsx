import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronDown, ChevronRight, Cpu, Palette, RefreshCw, User } from 'lucide-react';
import type { SessionConfig } from '../types';
import { Card, CardRow, SectionFooter, SectionHeader, SegmentControl } from './SettingsUI';
import { useTheme } from '../hooks/useTheme';
import type { Theme } from '../hooks/useTheme';

const PROVIDER_KEY = 'phoneclaw_provider';
const CLAUDE_MODEL_KEY = 'phoneclaw_claude_model';

type Provider = 'claude' | 'ollama';

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

const THEMES: { value: Theme; label: string; colors: string[] }[] = [
  { value: 'dark',    label: 'Dark',    colors: ['#131314', '#1e1f20', '#e3e3e3'] },
  { value: 'light',   label: 'Light',   colors: ['#f5f5f7', '#ffffff', '#1c1c1e'] },
  { value: 'gruvbox', label: 'Gruvbox', colors: ['#282828', '#3c3836', '#ebdbb2'] },
];

function formatPersonaLabel(persona: string): string {
  return persona
    .replace(/^persona_/, '')
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function formatModelLabel(id: string): string {
  return CLAUDE_MODELS.find((m) => m.id === id)?.label ?? id;
}

// ── Model config inline panel ──────────────────────────────────────────────────

function ModelConfigPanel({
  session,
  currentModel,
  onModelChange,
  setOllamaEndpoint,
  onOllamaEndpointChanged,
  onSaved,
}: {
  session: SessionConfig | null;
  currentModel: string;
  onModelChange: (m: string) => void;
  setOllamaEndpoint: (host: string, port: number) => Promise<SessionConfig>;
  onOllamaEndpointChanged: () => void;
  onSaved: (provider: Provider) => void;
}) {
  const [provider, setProvider] = useState<Provider>(
    () => (localStorage.getItem(PROVIDER_KEY) as Provider) ?? 'ollama',
  );
  const [claudeApiKey, setClaudeApiKey] = useState('');
  const [claudeModel, setClaudeModel] = useState(
    () => localStorage.getItem(CLAUDE_MODEL_KEY) ?? 'claude-sonnet-4-6',
  );
  const [isModelMenuOpen, setIsModelMenuOpen] = useState(false);
  const modelMenuRef = useRef<HTMLDivElement>(null);
  const [ollamaHostPort, setOllamaHostPort] = useState(() => {
    const host = (session?.ollama_host_override ?? '').trim() || '127.0.0.1';
    const port = session?.ollama_port ?? 11434;
    return `${host}:${port}`;
  });
  const [ollamaModel, setOllamaModel] = useState(currentModel);
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);
  const [ollamaModelsLoading, setOllamaModelsLoading] = useState(false);
  const [ollamaModelsError, setOllamaModelsError] = useState('');
  const [saveMsg, setSaveMsg] = useState('');

  function flashSaved(msg = 'Saved') {
    setSaveMsg(msg);
    setTimeout(() => setSaveMsg(''), 1800);
  }

  useEffect(() => {
    invoke<string | null>('load_secret', { key: 'claude_api_key' })
      .then((val) => { if (val) setClaudeApiKey(val); })
      .catch(() => {});
  }, []);

  useEffect(() => {
    function onPointerDown(event: PointerEvent) {
      if (modelMenuRef.current && !modelMenuRef.current.contains(event.target as Node)) {
        setIsModelMenuOpen(false);
      }
    }
    window.addEventListener('pointerdown', onPointerDown);
    return () => window.removeEventListener('pointerdown', onPointerDown);
  }, []);

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
      if (models.length > 0 && !models.includes(ollamaModel)) setOllamaModel(models[0]);
    } catch (e) {
      setOllamaModelsError(e instanceof Error ? e.message : String(e));
    } finally {
      setOllamaModelsLoading(false);
    }
  }

  useEffect(() => {
    if (provider === 'ollama') fetchOllamaModels(ollamaHostPort);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider]);

  function handleProviderChange(p: Provider) {
    setProvider(p);
    localStorage.setItem(PROVIDER_KEY, p);
    onSaved(p);
  }

  async function handleApiKeyBlur() {
    if (!claudeApiKey.trim()) return;
    try {
      await invoke('store_secret', { key: 'claude_api_key', value: claudeApiKey.trim() });
      flashSaved();
    } catch (e) {
      flashSaved(e instanceof Error ? e.message : String(e));
    }
  }

  function handleClaudeModelSelect(id: string) {
    setClaudeModel(id);
    setIsModelMenuOpen(false);
    localStorage.setItem(PROVIDER_KEY, 'claude');
    localStorage.setItem(CLAUDE_MODEL_KEY, id);
    onModelChange(id);
    onSaved('claude');
    flashSaved();
  }

  async function handleOllamaHostBlur() {
    const colonIdx = ollamaHostPort.lastIndexOf(':');
    const host = colonIdx > 0 ? ollamaHostPort.slice(0, colonIdx).trim() : ollamaHostPort.trim();
    const port = colonIdx > 0 ? parseInt(ollamaHostPort.slice(colonIdx + 1), 10) : 11434;
    if (!host || !Number.isFinite(port) || port < 1 || port > 65535) {
      flashSaved('Invalid IP:Port'); return;
    }
    try {
      await setOllamaEndpoint(host, port);
      onOllamaEndpointChanged();
      flashSaved();
    } catch (e) {
      flashSaved(e instanceof Error ? e.message : String(e));
    }
  }

  function handleOllamaModelSelect(m: string) {
    setOllamaModel(m);
    setIsModelMenuOpen(false);
    localStorage.setItem(PROVIDER_KEY, 'ollama');
    onModelChange(m);
    onSaved('ollama');
    flashSaved();
  }

  return (
    <div className="settings-inline-expand">
      <p className="settings-modal-field-label" style={{ marginTop: 0 }}>Provider</p>
      <div className="settings-provider-row">
        {(['claude', 'ollama'] as Provider[]).map((p) => (
          <button
            key={p}
            onClick={() => handleProviderChange(p)}
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
            onBlur={handleApiKeyBlur}
            placeholder="sk-ant-..."
            autoComplete="off"
            className="settings-popup-input"
            style={{ marginTop: 6 }}
          />
          <p className="settings-modal-field-label">Model</p>
          <div ref={modelMenuRef} className={`settings-model-menu${isModelMenuOpen ? ' settings-model-menu--open' : ''}`} style={{ marginTop: 6 }}>
            <button
              type="button"
              onClick={() => setIsModelMenuOpen((v) => !v)}
              className={`settings-model-trigger${isModelMenuOpen ? ' settings-model-trigger-open' : ''}`}
            >
              <span className="settings-model-trigger-label">{formatModelLabel(claudeModel)}</span>
              <ChevronDown size={16} className={`settings-model-trigger-chevron${isModelMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
            </button>
            {isModelMenuOpen && (
              <div className="settings-model-dropdown">
                {CLAUDE_MODELS.map((m) => (
                  <button
                    key={m.id}
                    type="button"
                    className={`settings-model-option${claudeModel === m.id ? ' settings-model-option-active' : ''}`}
                    onClick={() => handleClaudeModelSelect(m.id)}
                  >
                    <span>{m.label}</span>
                    {claudeModel === m.id && <Check size={14} />}
                  </button>
                ))}
              </div>
            )}
          </div>
        </>
      )}

      {provider === 'ollama' && (
        <>
          <p className="settings-modal-field-label">IP:Port</p>
          <div className="settings-hostport-row">
            <input
              value={ollamaHostPort}
              onChange={(e) => setOllamaHostPort(e.target.value)}
              onBlur={handleOllamaHostBlur}
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
          {ollamaModelsError && (
            <p className="settings-save-msg--err" style={{ marginTop: 6, fontSize: 11 }}>
              {ollamaModelsError}
            </p>
          )}
          {ollamaModels.length > 0 ? (
            <div ref={modelMenuRef} className={`settings-model-menu${isModelMenuOpen ? ' settings-model-menu--open' : ''}`} style={{ marginTop: 6 }}>
              <button
                type="button"
                onClick={() => setIsModelMenuOpen((v) => !v)}
                className={`settings-model-trigger${isModelMenuOpen ? ' settings-model-trigger-open' : ''}`}
              >
                <span className="settings-model-trigger-label">{ollamaModel}</span>
                <ChevronDown size={16} className={`settings-model-trigger-chevron${isModelMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
              </button>
              {isModelMenuOpen && (
                <div className="settings-model-dropdown">
                  {ollamaModels.map((m) => (
                    <button
                      key={m}
                      type="button"
                      className={`settings-model-option${ollamaModel === m ? ' settings-model-option-active' : ''}`}
                      onClick={() => handleOllamaModelSelect(m)}
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
              value={ollamaModel}
              onChange={(e) => setOllamaModel(e.target.value)}
              onBlur={() => handleOllamaModelSelect(ollamaModel)}
              placeholder="llama3.2:latest"
              className="settings-popup-input"
              style={{ marginTop: 6 }}
            />
          )}
        </>
      )}

      {saveMsg && (
        <p className={saveMsg === 'Saved' ? 'settings-save-msg--ok' : 'settings-save-msg--err'} style={{ marginTop: 8, fontSize: 12 }}>
          {saveMsg}
        </p>
      )}
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
  chatMode: boolean;
  onChatModeChange: (v: boolean) => void;
  igMode: boolean;
  onIgModeChange: (v: boolean) => void;
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
  chatMode,
  onChatModeChange,
  igMode,
  onIgModeChange,
}: GeneralTabProps) {
  const [showModelConfig, setShowModelConfig] = useState(false);
  const [activeProvider, setActiveProvider] = useState<Provider>(
    () => (localStorage.getItem(PROVIDER_KEY) as Provider) ?? 'ollama',
  );

  const [isPersonaMenuOpen, setIsPersonaMenuOpen] = useState(false);
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [personaSaveMsg, setPersonaSaveMsg] = useState('');
  const personaMenuRef = useRef<HTMLDivElement>(null);

  const [isThemeMenuOpen, setIsThemeMenuOpen] = useState(false);
  const themeMenuRef = useRef<HTMLDivElement>(null);

  const [theme, setTheme] = useTheme();

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
      if (themeMenuRef.current && !themeMenuRef.current.contains(event.target as Node)) {
        setIsThemeMenuOpen(false);
      }
    }
    window.addEventListener('pointerdown', onPointerDown);
    return () => window.removeEventListener('pointerdown', onPointerDown);
  }, []);

  return (
    <>
      <SectionHeader>Mode</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <SegmentControl
            options={[
              { value: 'normal' as const, label: 'Normal' },
              { value: 'chat' as const, label: 'Chat' },
            ]}
            value={chatMode ? 'chat' : 'normal'}
            onChange={(v) => onChatModeChange(v === 'chat')}
          />
        </div>
      </Card>
      <SectionFooter>Chat mode lets you create AI friends with custom personas.</SectionFooter>

      {chatMode && (
        <>
          <SectionHeader>IG Mode</SectionHeader>
          <Card>
            <div className="settings-card-body">
              <SegmentControl
                options={[
                  { value: 'off' as const, label: 'Off' },
                  { value: 'on' as const, label: 'On' },
                ]}
                value={igMode ? 'on' : 'off'}
                onChange={(v) => onIgModeChange(v === 'on')}
              />
            </div>
          </Card>
          <SectionFooter>Characters can share posts visible in the feed.</SectionFooter>
        </>
      )}

      {!chatMode && <><SectionHeader>Model</SectionHeader>
      <Card>
        <CardRow onClick={() => setShowModelConfig((v) => !v)}>
          <div className="settings-qr-row-left">
            <div className="settings-icon-badge settings-icon-badge--indigo">
              <Cpu size={18} />
            </div>
            <div>
              <p className="settings-item-title">Active model</p>
              <p className="settings-item-subtitle">{formatModelLabel(model) || 'Not configured'}</p>
            </div>
          </div>
          {showModelConfig
            ? <ChevronDown size={18} className="settings-chevron" />
            : <ChevronRight size={18} className="settings-chevron" />
          }
        </CardRow>

        {showModelConfig && (
          <ModelConfigPanel
            session={session}
            currentModel={model}
            onModelChange={onModelChange}
            setOllamaEndpoint={setOllamaEndpoint}
            onOllamaEndpointChanged={onOllamaEndpointChanged}

            onSaved={(p) => setActiveProvider(p)}
          />
        )}
      </Card>
      <SectionFooter>
        {activeProvider === 'claude' ? 'Using Claude API. Model and API key are stored securely.' : 'Using local Ollama instance.'}
      </SectionFooter>

      <SectionHeader>Persona</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <div className="settings-item-header">
            <div className="settings-icon-badge settings-icon-badge--emerald">
              <User size={18} />
            </div>
            <div className="settings-item-info">
              <p className="settings-item-title">Assistant persona</p>
              <p className="settings-item-subtitle">Choose response style and character</p>
            </div>
          </div>

          <div ref={personaMenuRef} className={`settings-model-menu${isPersonaMenuOpen ? ' settings-model-menu--open' : ''}`}>
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
      </SectionFooter></>}

      <SectionHeader>Appearance</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <div className="settings-item-header">
            <div className="settings-icon-badge settings-icon-badge--amber">
              <Palette size={18} />
            </div>
            <div className="settings-item-info">
              <p className="settings-item-title">Color theme</p>
              <p className="settings-item-subtitle">Choose UI appearance</p>
            </div>
          </div>

          <div ref={themeMenuRef} className={`settings-model-menu${isThemeMenuOpen ? ' settings-model-menu--open' : ''}`}>
            <button
              type="button"
              onClick={() => setIsThemeMenuOpen((v) => !v)}
              className={`settings-model-trigger${isThemeMenuOpen ? ' settings-model-trigger-open' : ''}`}
            >
              <span className="settings-model-trigger-label">
                {THEMES.find((t) => t.value === theme)?.label ?? theme}
              </span>
              <ChevronDown size={16} className={`settings-model-trigger-chevron${isThemeMenuOpen ? ' settings-model-trigger-chevron-open' : ''}`} />
            </button>

            {isThemeMenuOpen && (
              <div className="settings-model-dropdown">
                {THEMES.map((t) => (
                  <button
                    key={t.value}
                    type="button"
                    className={`settings-model-option${theme === t.value ? ' settings-model-option-active' : ''}`}
                    onClick={() => { setTheme(t.value); setIsThemeMenuOpen(false); }}
                  >
                    <span>{t.label}</span>
                    {theme === t.value && <Check size={14} />}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </Card>
      <SectionFooter>Changes apply immediately.</SectionFooter>
    </>
  );
}
