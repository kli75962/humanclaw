import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronDown, ChevronRight, Cpu, Image, LayoutGrid, Monitor, Palette, Plus, RefreshCw, User } from 'lucide-react';
import type { PcPermissions, PermissionState, SessionConfig } from '../../types';
import { Card, CardRow, SectionFooter, SectionHeader, SegmentControl } from '../settings/SettingsUI';
import { PersonaWizard } from '../persona/PersonaWizard';
import { BirthdayCalendar } from '../character/BirthdayCalendar';
import { ImageCropperModal } from '../ui/ImageCropperModal';
import { useTheme } from '../../hooks/useTheme';
import type { Theme } from '../../hooks/useTheme';
import { useWallpaper } from '../../hooks/useWallpaper';

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
}: {
  session: SessionConfig | null;
  currentModel: string;
  onModelChange: (m: string) => void;
  setOllamaEndpoint: (host: string, port: number) => Promise<SessionConfig>;
  onOllamaEndpointChanged: () => void;
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
  const [saveMsg, setSaveMsg] = useState('');
  const flashTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  function flashSaved(msg = 'Saved') {
    clearTimeout(flashTimerRef.current);
    setSaveMsg(msg);
    flashTimerRef.current = setTimeout(() => setSaveMsg(''), 1800);
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
    setOllamaModels([]);
    try {
      const models = await invoke<string[]>('list_models_at', { host, port });
      setOllamaModels(models);
      if (models.length > 0 && !models.includes(ollamaModel)) setOllamaModel(models[0]);
    } catch {
      // silently ignore — UI shows "No model detected"
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
    onModelChange(p === 'claude' ? claudeModel : ollamaModel);
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
            <div className="settings-model-trigger settings-model-trigger--disabled" style={{ marginTop: 6 }}>
              <span className="settings-model-trigger-label settings-model-trigger-label--muted">
                {ollamaModelsLoading ? 'Loading…' : 'No model detected'}
              </span>
            </div>
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

const PERM_ROWS: { field: keyof PcPermissions; label: string; subtitle: string }[] = [
  { field: 'shell_execution', label: 'Run Commands',   subtitle: 'Execute shell commands via system_run' },
  { field: 'launch_app',      label: 'Open URL / App', subtitle: 'Open URLs and files with system launcher' },
  { field: 'take_screenshot', label: 'Screenshot',     subtitle: 'Capture the screen for verification' },
];

const DEFAULT_PC_PERMS: PcPermissions = {
  take_screenshot: 'allow_all',
  launch_app:      'ask_before_use',
  shell_execution: 'ask_before_use',
};

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
  setPcPermissions: (p: PcPermissions) => Promise<SessionConfig>;
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
  onChatModeChange: _onChatModeChange,
  igMode,
  onIgModeChange,
  setPcPermissions,
}: GeneralTabProps) {
  const [pcPerms, setPcPermsLocal] = useState<PcPermissions>(
    () => session?.pc_permissions ?? DEFAULT_PC_PERMS,
  );

  useEffect(() => {
    if (session?.pc_permissions) setPcPermsLocal(session.pc_permissions);
  }, [session]);

  async function handlePermChange(field: keyof PcPermissions, value: PermissionState) {
    const next = { ...pcPerms, [field]: value };
    setPcPermsLocal(next);
    try { await setPcPermissions(next); } catch { /* ignore */ }
  }
  const [showModelConfig, setShowModelConfig] = useState(false);
  const [showPermConfig, setShowPermConfig] = useState(false);
  const [showWallpaperConfig, setShowWallpaperConfig] = useState(false);

  const [isPersonaMenuOpen, setIsPersonaMenuOpen] = useState(false);
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [personaSaveMsg, setPersonaSaveMsg] = useState('');
  const personaTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const personaMenuRef = useRef<HTMLDivElement>(null);
  const [showAddPersona, setShowAddPersona] = useState(false);
  const [personaBuilding, setPersonaBuilding] = useState(false);

  const [isThemeMenuOpen, setIsThemeMenuOpen] = useState(false);
  const themeMenuRef = useRef<HTMLDivElement>(null);

  const [theme, setTheme] = useTheme();
  const { url: wallpaperUrl, blur, dim, loadWallpaperFile, clearWallpaper, setBlur, setDim, cropperSrc, onCropperConfirm, onCropperCancel } = useWallpaper();
  const wallpaperFileRef = useRef<HTMLInputElement>(null);

  // ── User Birthday ─────────────────────────────────────────────────────────
  const [showBirthdayConfig, setShowBirthdayConfig] = useState(false);
  const [userBirthday, setUserBirthday] = useState<string | null>(null);
  const [birthdaySaveMsg, setBirthdaySaveMsg] = useState('');
  const birthdayTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  // Format birthday display
  const formatBirthdayDisplay = () => {
    if (!userBirthday) return 'Not set';
    const [year, month, day] = userBirthday.split('-');
    return new Date(`${year}-${month}-${day}`).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
  };

  useEffect(() => {
    // Load user birthday from memory
    invoke<string>('get_memory_file', { filename: 'core.md' })
      .then((content) => {
        const match = content.match(/\[USER_BIRTHDAY\]:\s*(.+)/);
        if (match) {
          const birthday = match[1].trim();
          setUserBirthday(birthday);
        }
      })
      .catch(() => {});
  }, []);

  async function saveBirthday(dateStr: string) {
    if (!dateStr) return;

    try {
      let content = await invoke<string>('get_memory_file', { filename: 'core.md' });

      const birthdayLine = `[USER_BIRTHDAY]: ${dateStr}`;
      if (content.includes('[USER_BIRTHDAY]')) {
        content = content.replace(/\[USER_BIRTHDAY\]:.+/, birthdayLine);
      } else {
        content = content.trimEnd() + '\n\n' + birthdayLine;
      }

      await invoke('set_memory_file', { filename: 'core.md', content });
      setUserBirthday(dateStr);
      clearTimeout(birthdayTimerRef.current);
      setBirthdaySaveMsg('Saved');
      birthdayTimerRef.current = setTimeout(() => setBirthdaySaveMsg(''), 1800);
    } catch (e) {
      setBirthdaySaveMsg(e instanceof Error ? e.message : String(e));
    }
  }

  async function clearBirthday() {
    try {
      let content = await invoke<string>('get_memory_file', { filename: 'core.md' });
      content = content.replace(/\[USER_BIRTHDAY\]:.+\n?/g, '');
      await invoke('set_memory_file', { filename: 'core.md', content });
      setUserBirthday(null);
      clearTimeout(birthdayTimerRef.current);
      setBirthdaySaveMsg('Cleared');
      birthdayTimerRef.current = setTimeout(() => setBirthdaySaveMsg(''), 1800);
    } catch (e) {
      setBirthdaySaveMsg(e instanceof Error ? e.message : String(e));
    }
  }

  useEffect(() => {
    if (!isPersonaMenuOpen) return;
    listPersonas()
      .then((names) => { if (names.length > 0) setPersonas(names); })
      .catch(() => setPersonas(FALLBACK_PERSONAS));
  }, [isPersonaMenuOpen, listPersonas]);

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
      {/* User Birthday Section — chat mode only */}
      {chatMode && (
        <>
          <SectionHeader>Your Profile</SectionHeader>
          <Card>
            <CardRow onClick={() => setShowBirthdayConfig((v) => !v)}>
              <div className="settings-qr-row-left">
                <div className="settings-icon-badge settings-icon-badge--emerald">
                  🎂
                </div>
                <div>
                  <p className="settings-item-title">Birthday</p>
                  <p className="settings-item-subtitle">{formatBirthdayDisplay()}</p>
                </div>
              </div>
              {showBirthdayConfig
                ? <ChevronDown size={18} className="settings-chevron" />
                : <ChevronRight size={18} className="settings-chevron" />
              }
            </CardRow>

            {showBirthdayConfig && (
              <div className="settings-inline-expand">
                <BirthdayCalendar value={userBirthday} onChange={saveBirthday} />

                {userBirthday && (
                  <button
                    type="button"
                    onClick={() => clearBirthday()}
                    style={{
                      marginTop: 12,
                      padding: '8px 16px',
                      borderRadius: 4,
                      border: '1px solid var(--color-border)',
                      backgroundColor: 'transparent',
                      color: 'var(--color-text-2)',
                      cursor: 'pointer',
                      fontSize: 12,
                    }}
                  >
                    Clear
                  </button>
                )}

                {birthdaySaveMsg && (
                  <p className={birthdaySaveMsg === 'Saved' || birthdaySaveMsg === 'Cleared' ? 'settings-save-msg--ok' : 'settings-save-msg--err'} style={{ marginTop: 12, fontSize: 12 }}>
                    {birthdaySaveMsg}
                  </p>
                )}
              </div>
            )}
          </Card>
        </>
      )}

      {chatMode && (
        <>
          <SectionHeader>Post Mode</SectionHeader>
          <Card>
            <div className="settings-perm-row" style={{ padding: '10px 16px' }}>
              <div className="settings-qr-row-left">
                <div className="settings-icon-badge settings-icon-badge--amber">
                  <LayoutGrid size={18} />
                </div>
                <div className="settings-item-info">
                  <p className="settings-item-title">Social posts</p>
                  <p className="settings-item-subtitle">Allow characters create posts like social media.</p>
                </div>
              </div>
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
          <SectionFooter>This will cause more token spend base on how many character you create.</SectionFooter>
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
          />
        )}
      </Card>

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
                          clearTimeout(personaTimerRef.current);
                          setPersonaSaveMsg('Saved');
                          personaTimerRef.current = setTimeout(() => setPersonaSaveMsg(''), 1800);
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
        {personaSaveMsg ? ` ${personaSaveMsg}` : ''}
      </SectionFooter>

      </>}

      <SectionHeader>Add Persona</SectionHeader>
      <Card>
        <CardRow onClick={() => { if (!personaBuilding) setShowAddPersona((v) => !v); }}>
          <div className="settings-qr-row-left">
            <div className="settings-icon-badge settings-icon-badge--emerald">
              <Plus size={18} />
            </div>
            <div>
              <p className="settings-item-title">Create new persona</p>
              <p className="settings-item-subtitle">Let AI build a custom persona for you</p>
            </div>
          </div>
          {!personaBuilding && (showAddPersona
            ? <ChevronDown size={18} className="settings-chevron" />
            : <ChevronRight size={18} className="settings-chevron" />
          )}
        </CardRow>
        {showAddPersona && !personaBuilding && (
          <div className="settings-inline-expand">
            <PersonaWizard
              onComplete={async (answers) => {
                setShowAddPersona(false);
                setPersonaBuilding(true);
                document.dispatchEvent(new CustomEvent('persona-build-start', {
                  detail: {
                    displayName: answers.personaName === 'random' ? 'new persona' : answers.personaName,
                    model,
                    sex: answers.sex,
                    ageRange: answers.ageRange,
                    vibe: answers.vibe,
                    world: answers.world,
                    connectsBy: answers.connectsBy,
                    personaName: answers.personaName,
                  },
                }));
                try {
                  await invoke('create_persona_background', {
                    model,
                    sex: answers.sex,
                    ageRange: answers.ageRange,
                    vibe: answers.vibe,
                    world: answers.world,
                    connectsBy: answers.connectsBy,
                    personaName: answers.personaName,
                  });
                  // Rust already wrote the "done" status; App.tsx will pick it up via event
                } catch {
                  // Rust already wrote the "interrupted" status
                } finally {
                  setPersonaBuilding(false);
                  document.dispatchEvent(new Event('persona-build-settled'));
                }
              }}
            />
          </div>
        )}
      </Card>

      <SectionHeader>PC Control</SectionHeader>
      <Card>
        <CardRow onClick={() => setShowPermConfig((v) => !v)}>
          <div className="settings-qr-row-left">
            <div className="settings-icon-badge settings-icon-badge--indigo">
              <Monitor size={18} />
            </div>
            <div>
              <p className="settings-item-title">Tool permissions</p>
              <p className="settings-item-subtitle">Control what the AI can do on this PC</p>
            </div>
          </div>
          {showPermConfig
            ? <ChevronDown size={18} className="settings-chevron" />
            : <ChevronRight size={18} className="settings-chevron" />
          }
        </CardRow>
        {showPermConfig && (
          <div className="settings-inline-expand">
            <div className="settings-perm-list">
              {PERM_ROWS.map(({ field, label, subtitle }) => (
                <div key={field} className="settings-perm-row">
                  <div className="settings-perm-info">
                    <span className="settings-perm-label">{label}</span>
                    <span className="settings-perm-sub">{subtitle}</span>
                  </div>
                  <SegmentControl
                    options={[
                      { value: 'allow_all'      as PermissionState, label: 'Allow' },
                      { value: 'ask_before_use' as PermissionState, label: 'Ask' },
                      { value: 'not_allow'      as PermissionState, label: 'Block' },
                    ]}
                    value={pcPerms[field]}
                    onChange={(v) => handlePermChange(field, v)}
                  />
                </div>
              ))}
            </div>
          </div>
        )}
      </Card>

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

      <Card>
        <CardRow onClick={() => setShowWallpaperConfig((v) => !v)}>
          <div className="settings-qr-row-left">
            <div className="settings-icon-badge settings-icon-badge--neutral">
              <Image size={18} />
            </div>
            <div>
              <p className="settings-item-title">Wallpaper</p>
              <p className="settings-item-subtitle">{wallpaperUrl ? 'Custom image set' : 'Not set'}</p>
            </div>
          </div>
          {showWallpaperConfig
            ? <ChevronDown size={18} className="settings-chevron" />
            : <ChevronRight size={18} className="settings-chevron" />
          }
        </CardRow>

        {showWallpaperConfig && (
          <div className="settings-inline-expand">
            {/* Wallpaper preview/picker */}
            <div style={{ marginBottom: 12 }}>
              <div
                className="settings-wallpaper-picker"
                onClick={() => {
                  if (!wallpaperUrl) wallpaperFileRef.current?.click();
                }}
              >
                {wallpaperUrl ? (
                  <div
                    className="settings-wallpaper-preview"
                    style={{ backgroundImage: `url(${wallpaperUrl})` }}
                    onClick={() => clearWallpaper()}
                  >
                    <button
                      type="button"
                      className="settings-wallpaper-clear-overlay"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        clearWallpaper();
                      }}
                    >
                      Clear
                    </button>
                  </div>
                ) : (
                  <div className="settings-wallpaper-empty">
                    <Image size={32} color="var(--color-text-3)" />
                    <button
                      type="button"
                      className="settings-wallpaper-set-overlay"
                      onClick={(e) => {
                        e.preventDefault();
                        e.stopPropagation();
                        wallpaperFileRef.current?.click();
                      }}
                    >
                      Set Wallpaper
                    </button>
                  </div>
                )}
              </div>
              <input
                ref={wallpaperFileRef}
                type="file"
                accept="image/png,image/jpeg"
                style={{ display: 'none' }}
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) loadWallpaperFile(file);
                  e.target.value = '';
                }}
              />
            </div>

            {/* Blur slider */}
            <div className="settings-tweak-row">
              <span className="settings-tweak-label">Blur</span>
              <input
                type="range"
                min="0"
                max="20"
                step="1"
                value={blur}
                onChange={(e) => setBlur(Number(e.target.value))}
                className="settings-tweak-slider"
              />
              <span className="settings-tweak-value">{blur}px</span>
            </div>

            {/* Dim slider */}
            <div className="settings-tweak-row" style={{ marginTop: 8 }}>
              <span className="settings-tweak-label">Dim</span>
              <input
                type="range"
                min="0"
                max="90"
                step="5"
                value={Math.round(dim * 100)}
                onChange={(e) => setDim(Number(e.target.value) / 100)}
                className="settings-tweak-slider"
              />
              <span className="settings-tweak-value">{Math.round(dim * 100)}%</span>
            </div>
          </div>
        )}
      </Card>

      {cropperSrc && (
        <ImageCropperModal
          src={cropperSrc}
          aspectRatio={16 / 9}
          onConfirm={onCropperConfirm}
          onCancel={onCropperCancel}
          title="Crop Wallpaper"
        />
      )}
    </>
  );
}
