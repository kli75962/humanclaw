import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft, Camera, ChevronDown, ChevronRight, ImagePlus, Monitor, QrCode, Cpu, Smartphone, Trash2, Mic, Network, Check, Pencil } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { scan, Format } from '@tauri-apps/plugin-barcode-scanner';
import jsQR from 'jsqr';
import { Modal } from './Modal';
import './SettingsScreen.css';
import { useSession } from '../hooks/useSession';
import type { SettingsScreenProps } from '../types';

type SettingsTab = 'general' | 'connect';

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

// ── Sub-components ──────────────────────────────────────────────────────────────────

function SegmentControl<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <div className="settings-segment flex bg-[#181C24] border border-[#2C3444] rounded-full p-1">
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className={`settings-segment-btn flex-1 py-2 text-sm font-medium rounded-full transition-all ${
            value === opt.value
              ? 'settings-segment-btn-active bg-indigo-600 text-white shadow-sm'
              : 'text-slate-400 hover:text-slate-200'
          }`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <p className="settings-section-header text-[11px] font-bold text-slate-400 uppercase tracking-[0.14em]">
      {children}
    </p>
  );
}

function SectionFooter({ children }: { children: React.ReactNode }) {
  return (
    <p className="settings-section-footer text-[12px] text-slate-400 leading-relaxed">
      {children}
    </p>
  );
}

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="settings-card">
      {children}
    </div>
  );
}

function CardRow({
  onClick,
  children,
}: {
  onClick?: () => void;
  children: React.ReactNode;
}) {
  const Tag = onClick ? 'button' : 'div';
  return (
    <Tag
      onClick={onClick}
      className="settings-card-row w-full flex items-center justify-between text-left transition-colors"
    >
      {children}
    </Tag>
  );
}

function CardDivider() {
  return <div className="settings-card-divider h-px bg-[#2B3344] mx-4" />;
}

// ── QR Pairing views ────────────────────────────────────────────────────────────────

function ShowQrView() {
  const [svg, setSvg] = useState('');
  const [allAddresses, setAllAddresses] = useState<string[]>([]);
  const [customAddress, setCustomAddress] = useState('');
  const [error, setError] = useState('');
  const [fetchingPublicIp, setFetchingPublicIp] = useState(false);

  // Load all detected addresses on mount
  useEffect(() => {
    invoke<string[]>('get_all_local_addresses')
      .then(setAllAddresses)
      .catch(() => setAllAddresses([]));
  }, []);

  // Generate QR with all addresses (or custom override) on change
  useEffect(() => {
    if (allAddresses.length === 0 && !customAddress) return;
    setSvg('');
    setError('');
    const timer = setTimeout(() => {
      // If custom address is set, use only that; otherwise auto-detect encodes all
      const opts = customAddress.trim()
        ? { customAddress: customAddress.trim() }
        : {};
      invoke<string>('get_qr_pair_svg', opts)
        .then(setSvg)
        .catch((e) => setError(String(e)));
    }, 400);
    return () => clearTimeout(timer);
  }, [allAddresses, customAddress]);

  async function fetchPublicIp() {
    setFetchingPublicIp(true);
    try {
      const resp = await fetch('https://api.ipify.org?format=text');
      const ip = (await resp.text()).trim();
      const port = allAddresses[0]?.split(':')[1] ?? '9876';
      setCustomAddress(`${ip}:${port}`);
    } catch {
      setError('Could not fetch public IP.');
    } finally {
      setFetchingPublicIp(false);
    }
  }

  return (
    <div className="flex flex-col items-center gap-4">
      {svg ? (
        <div
          className="bg-white rounded-xl p-3"
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      ) : error ? (
        <p className="text-xs text-red-400 text-center py-4">{error}</p>
      ) : (
        <div className="flex items-center justify-center py-8">
          <QrCode size={48} className="text-gray-600 animate-pulse" />
        </div>
      )}

      <div className="w-full flex flex-col gap-2">
        <p className="text-xs text-gray-400">
          Detected addresses ({allAddresses.length})
        </p>
        {allAddresses.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {allAddresses.map((addr) => (
              <span
                key={addr}
                className="px-2 py-1 text-xs rounded-lg bg-[#222836] text-slate-300"
              >
                {addr}
              </span>
            ))}
          </div>
        )}
        <p className="text-xs text-gray-500 mt-1">
          All addresses above are encoded in the QR. Phone will try each automatically.
        </p>
        <div className="flex gap-2 mt-1">
          <input
            value={customAddress}
            onChange={(e) => setCustomAddress(e.target.value)}
            placeholder="Override with custom ip:port"
            className="flex-1 bg-[#111521] border border-[#2F3A52] text-slate-200 text-sm px-3 py-2 rounded-xl outline-none focus:ring-1 focus:ring-indigo-500"
          />
          <button
            onClick={fetchPublicIp}
            disabled={fetchingPublicIp}
            className="px-3 py-2 rounded-xl bg-indigo-600/20 text-indigo-300 text-xs hover:bg-indigo-600/30 transition-colors disabled:opacity-50"
          >
            {fetchingPublicIp ? '…' : 'Public IP'}
          </button>
        </div>
        <p className="text-xs text-gray-500">
          On mobile data? Click <span className="text-gray-300">Public IP</span> and ensure port forwarding is set up on your router.
        </p>
      </div>
    </div>
  );
}

function ScanView({ onPaired }: { onPaired: () => void; isAndroid: boolean }) {
  const [status, setStatus] = useState<'idle' | 'scanning' | 'pairing' | 'done' | 'error'>('idle');
  const [error, setError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  async function pairWithPayload(raw: string) {
    let parsed: { addresses?: string[]; address?: string; pairing_token?: string; hash_key?: string };
    try {
      parsed = JSON.parse(raw);
    } catch {
      throw new Error('Invalid QR code — not a valid pairing code.');
    }
    // Support new token-based QR (pairing_token) and legacy hash_key QR.
    const token = parsed.pairing_token ?? parsed.hash_key ?? null;
    const addresses = parsed.addresses ?? (parsed.address ? [parsed.address] : []);
    if (addresses.length === 0 || !token) {
      throw new Error('Invalid QR code — missing address or pairing token.');
    }
    setStatus('pairing');
    await invoke('pair_from_qr', { addresses, hashKey: token });
    setStatus('done');
    onPaired();
  }

  async function handleImageFile(file: File) {
    setStatus('scanning');
    setError('');
    try {
      const bitmap = await createImageBitmap(file);
      const canvas = document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const ctx = canvas.getContext('2d')!;
      ctx.drawImage(bitmap, 0, 0);
      const imageData = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
      const code = jsQR(imageData.data, imageData.width, imageData.height);
      if (!code) throw new Error('No QR code detected in image.');
      await pairWithPayload(code.data);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setStatus('error');
    }
  }

  async function handleScan() {
    setStatus('scanning');
    setError('');
    try {
      const result = await scan({ formats: [Format.QRCode], windowed: true });
      await pairWithPayload(result.content);
    } catch (e) {
      const msg = e instanceof Error ? e.message : typeof e === 'object' ? JSON.stringify(e) : String(e);
      const normalized = msg.toLowerCase();
      if (
        normalized.includes('cancel') ||
        normalized.includes('closed') ||
        normalized.includes('dismiss')
      ) {
        setStatus('idle');
        setError('');
        return;
      }
      setError(msg);
      setStatus('error');
    }
  }

  return (
    <div className="flex flex-col items-center gap-4">
      <input
        ref={fileInputRef}
        type="file"
        accept="image/*"
        style={{ display: 'none' }}
        onChange={(e) => { const f = e.target.files?.[0]; if (f) handleImageFile(f); e.target.value = ''; }}
      />

      {status === 'idle' && (
        <div className="flex flex-col items-center gap-3 w-full">
          <button
            onClick={handleScan}
            className="flex items-center gap-2 px-6 py-3 rounded-xl bg-indigo-600 text-sm font-medium text-white hover:bg-indigo-500 transition-colors"
          >
            <Camera size={16} />
            Scan QR Code
          </button>
          <button
            onClick={() => fileInputRef.current?.click()}
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-[#222836] text-sm text-slate-300 hover:bg-[#2B3243] transition-colors"
          >
            <ImagePlus size={15} />
            Import from image
          </button>
        </div>
      )}

      {status === 'scanning' && (
        <p className="text-sm text-gray-400 animate-pulse">Opening camera…</p>
      )}

      {status === 'pairing' && (
        <p className="text-sm text-gray-400 animate-pulse">Linking device…</p>
      )}

      {status === 'error' && (
        <div className="flex flex-col items-center gap-3">
          <p className="text-xs text-red-400 text-center">❌ {error}</p>
          <div className="flex gap-2">
            <button
              onClick={handleScan}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[#222836] text-sm text-slate-200 hover:bg-[#2B3243] transition-colors"
            >
              <Camera size={14} />
              Try again
            </button>
            <button
              onClick={() => fileInputRef.current?.click()}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[#222836] text-sm text-slate-200 hover:bg-[#2B3243] transition-colors"
            >
              <ImagePlus size={14} />
              Image
            </button>
          </div>
        </div>
      )}

      <p className="text-xs text-gray-500 text-center">
        Scan live or import a screenshot of the QR code.
      </p>
    </div>
  );
}

// ── Main component ──────────────────────────────────────────────────────────────────

export function SettingsScreen({ model, availableModels, onModelChange, onOllamaEndpointChanged, onBack }: SettingsScreenProps) {
  const [tab, setTab] = useState<SettingsTab>('general');
  const [showQrPair, setShowQrPair] = useState(false);
  const [showEndpointEdit, setShowEndpointEdit] = useState(false);
  const [showSttEdit, setShowSttEdit] = useState(false);
  const [isModelMenuOpen, setIsModelMenuOpen] = useState(false);
  const [isPersonaMenuOpen, setIsPersonaMenuOpen] = useState(false);
  const [personas, setPersonas] = useState<string[]>(FALLBACK_PERSONAS);
  const [personaSaveMsg, setPersonaSaveMsg] = useState('');
  const [googleApiKey, setGoogleApiKey] = useState('');
  const [googleSttLanguages, setGoogleSttLanguages] = useState('en-US,yue-Hant-HK,cmn-Hans-CN');
  const [ollamaHost, setOllamaHost] = useState('127.0.0.1');
  const [ollamaPort, setOllamaPort] = useState('11434');
  const [ollamaSaving, setOllamaSaving] = useState(false);
  const [ollamaSaveMsg, setOllamaSaveMsg] = useState('');

  useEffect(() => {
    invoke<string | null>('load_secret', { key: 'google_api_key' })
      .then((val) => { if (val) setGoogleApiKey(val); })
      .catch(() => {});

    invoke<string | null>('load_secret', { key: 'google_stt_languages' })
      .then((val) => { if (val) setGoogleSttLanguages(val); })
      .catch(() => {});
  }, []);
  const [peerStatus, setPeerStatus] = useState<Record<string, boolean>>({});
  const { session, refresh, removeLinkedDevice, setOllamaEndpoint, listPersonas, setPersona } = useSession();
  const isAndroid = session?.device.device_type === 'android';
  const modelMenuRef = useRef<HTMLDivElement>(null);
  const personaMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listPersonas()
      .then((names) => {
        if (names.length > 0) {
          setPersonas(names);
        }
      })
      .catch(() => {
        setPersonas(FALLBACK_PERSONAS);
      });
  }, [listPersonas]);

  useEffect(() => {
    const fallbackHost = isAndroid
      ? (session?.paired_devices?.[0]?.address.split(':')[0] ?? '127.0.0.1')
      : '127.0.0.1';
    const host = (session?.ollama_host_override ?? '').trim();
    setOllamaHost(host || fallbackHost);
    setOllamaPort(String(session?.ollama_port ?? 11434));
  }, [isAndroid, session?.ollama_host_override, session?.ollama_port, session?.paired_devices]);

  useEffect(() => {
    if ((session?.paired_devices ?? []).length === 0) return;
    // Initial poll to get current state.
    invoke<Array<{ device_id: string; online: boolean }>>('get_all_peer_status')
      .then((list) => setPeerStatus(Object.fromEntries(list.map((p) => [p.device_id, p.online]))))
      .catch(() => {});
    // Listen for real-time updates emitted by the Rust peer monitor.
    const unlisten = listen<Array<{ device_id: string; online: boolean }>>('peer-status-changed', (event) => {
      setPeerStatus(Object.fromEntries(event.payload.map((p) => [p.device_id, p.online])));
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [session?.paired_devices]);

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

  async function handleSaveOllamaEndpoint() {
    const host = ollamaHost.trim();
    const port = Number.parseInt(ollamaPort.trim(), 10);
    if (!host) {
      setOllamaSaveMsg('Host is required');
      return;
    }
    if (!Number.isFinite(port) || port < 1 || port > 65535) {
      setOllamaSaveMsg('Port must be 1-65535');
      return;
    }

    setOllamaSaving(true);
    setOllamaSaveMsg('');
    try {
      await setOllamaEndpoint(host, port);
      onOllamaEndpointChanged();
      setOllamaSaveMsg('Saved');
      setTimeout(() => setOllamaSaveMsg(''), 2000);
    } catch (e) {
      setOllamaSaveMsg(e instanceof Error ? e.message : String(e));
    } finally {
      setOllamaSaving(false);
    }
  }

  return (
    <div className="settings-root flex flex-col h-screen bg-gradient-to-b from-[#0D1017] to-[#090B11] text-[#E7EAF3]">
      {showQrPair && (
        <Modal title={isAndroid ? 'Scan QR to Link' : 'Show QR to Link'} onClose={() => setShowQrPair(false)}>
          {isAndroid ? (
            <ScanView onPaired={() => { refresh(); setShowQrPair(false); }} isAndroid={isAndroid} />
          ) : (
            <ShowQrView />
          )}
        </Modal>
      )}
      {showEndpointEdit && (
        <Modal title="Edit Ollama Endpoint" onClose={() => setShowEndpointEdit(false)}>
          <div className="settings-edit-modal-body">
            <p className="text-[12px] text-slate-400">Host and port for model list and chat requests.</p>
            <p className="text-[12px] text-slate-400 mt-1">Enter only host/IP and port. Example: 192.168.1.10 and 11434.</p>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 mt-3">
              <input
                value={ollamaHost}
                onChange={(e) => { setOllamaHost(e.target.value); setOllamaSaveMsg(''); }}
                placeholder="127.0.0.1"
                className="settings-popup-input sm:col-span-2 bg-[#101521] border border-[#2F3A52] rounded-xl px-4 py-2.5 text-sm font-mono text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
              <input
                value={ollamaPort}
                onChange={(e) => { setOllamaPort(e.target.value); setOllamaSaveMsg(''); }}
                placeholder="11434"
                inputMode="numeric"
                className="settings-popup-input bg-[#101521] border border-[#2F3A52] rounded-xl px-4 py-2.5 text-sm font-mono text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
              />
            </div>

            <div className="settings-edit-modal-actions">
              <p className={`text-xs ${ollamaSaveMsg === 'Saved' ? 'text-green-400' : 'text-red-400'}`}>
                {ollamaSaveMsg || ' '}
              </p>
              <button
                onClick={handleSaveOllamaEndpoint}
                disabled={ollamaSaving}
                className="px-4 py-2 rounded-full text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 transition-colors"
              >
                {ollamaSaving ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        </Modal>
      )}
      {showSttEdit && !isAndroid && (
        <Modal title="Edit Speech to Text" onClose={() => setShowSttEdit(false)}>
          <div className="settings-edit-modal-body">
            <p className="text-[12px] text-slate-400">Google Cloud Speech-to-Text API settings.</p>
            <input
              type="password"
              value={googleApiKey}
              onChange={(e) => {
                setGoogleApiKey(e.target.value);
                invoke('store_secret', { key: 'google_api_key', value: e.target.value }).catch(() => {});
              }}
              placeholder="AIzaSy..."
              autoComplete="off"
              className="settings-popup-input mt-3 bg-[#101521] border border-[#2F3A52] rounded-xl px-4 py-2.5 text-sm font-mono text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            />
            <input
              value={googleSttLanguages}
              onChange={(e) => {
                const value = e.target.value;
                setGoogleSttLanguages(value);
                invoke('store_secret', { key: 'google_stt_languages', value }).catch(() => {});
              }}
              placeholder="en-US,yue-Hant-HK,cmn-Hans-CN"
              autoComplete="off"
              className="settings-popup-input mt-2 bg-[#101521] border border-[#2F3A52] rounded-xl px-4 py-2.5 text-sm font-mono text-slate-200 focus:outline-none focus:ring-1 focus:ring-indigo-500"
            />
            <p className="text-[12px] text-slate-400 mt-2">
              Or set <span className="font-mono text-slate-300">GOOGLE_API_KEY</span> in <span className="font-mono text-slate-300">src-tauri/.secrets</span>.
              Language codes are comma-separated, first is primary.
            </p>
          </div>
        </Modal>
      )}

      {/* Header */}
      <div className="settings-header flex items-center gap-3 px-2 py-3 border-b border-[#2A3140]">
        <button
          onClick={onBack}
          className="settings-back-btn p-2 hover:bg-[#1A2130] rounded-full transition-colors"
        >
          <ArrowLeft size={22} className="text-slate-300" />
        </button>
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>

      {/* Segment tabs */}
      <div className="settings-tabs px-4 pb-3">
        <SegmentControl
          options={[
            { value: 'general' as const, label: 'General' },
            { value: 'connect' as const, label: 'Connect' },
          ]}
          value={tab}
          onChange={setTab}
        />
      </div>

      {/* Scrollable content */}
      <div className="settings-body flex-1 min-h-0 overflow-y-auto px-4 custom-scrollbar">
        <div className="settings-content max-w-2xl mx-auto pb-12">
          {tab === 'general' && (
            <>
              {/* Model */}
              <SectionHeader>Model</SectionHeader>
              <Card>
                <div className="px-4 py-3.5 flex flex-col gap-2">
                  <div className="flex items-center gap-3 mb-1">
                    <div className="w-9 h-9 rounded-xl bg-indigo-500/15 flex items-center justify-center shrink-0">
                      <Cpu size={18} className="text-indigo-300" />
                    </div>
                    <div>
                      <p className="text-[15px]">Active model</p>
                      <p className="text-[12px] text-slate-400 mt-0.5">Select model for chat requests</p>
                    </div>
                  </div>

                  <div ref={modelMenuRef} className="settings-model-menu mt-1">
                    <button
                      type="button"
                      onClick={() => setIsModelMenuOpen((v) => !v)}
                      className={`settings-model-trigger ${isModelMenuOpen ? 'settings-model-trigger-open' : ''}`}
                    >
                      <span className="settings-model-trigger-label">
                        {model || 'No model selected'}
                      </span>
                      <ChevronDown size={16} className={`settings-model-trigger-chevron ${isModelMenuOpen ? 'settings-model-trigger-chevron-open' : ''}`} />
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
                              className={`settings-model-option ${m === model ? 'settings-model-option-active' : ''}`}
                              onClick={() => {
                                onModelChange(m);
                                setIsModelMenuOpen(false);
                              }}
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
              <SectionFooter>
                Models are loaded from your local Ollama instance.
              </SectionFooter>

              {/* Persona */}
              <SectionHeader>Persona</SectionHeader>
              <Card>
                <div className="px-4 py-3.5 flex flex-col gap-2">
                  <div className="flex items-center gap-3 mb-1">
                    <div className="w-9 h-9 rounded-xl bg-emerald-500/15 flex items-center justify-center shrink-0">
                      <Cpu size={18} className="text-emerald-300" />
                    </div>
                    <div>
                      <p className="text-[15px]">Assistant persona</p>
                      <p className="text-[12px] text-slate-400 mt-0.5">Choose response style and character</p>
                    </div>
                  </div>

                  <div ref={personaMenuRef} className="settings-model-menu mt-1">
                    <button
                      type="button"
                      onClick={() => setIsPersonaMenuOpen((v) => !v)}
                      className={`settings-model-trigger ${isPersonaMenuOpen ? 'settings-model-trigger-open' : ''}`}
                    >
                      <span className="settings-model-trigger-label">
                        {formatPersonaLabel(session?.persona || 'persona_default')}
                      </span>
                      <ChevronDown size={16} className={`settings-model-trigger-chevron ${isPersonaMenuOpen ? 'settings-model-trigger-chevron-open' : ''}`} />
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
                              className={`settings-model-option ${persona === session?.persona ? 'settings-model-option-active' : ''}`}
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
          )}

          {tab === 'connect' && (
            <>
              <SectionHeader>Ollama Endpoint</SectionHeader>
              <Card>
                <div className="px-4 py-3.5 flex flex-col gap-3">
                  <div className="flex items-center gap-3 mb-1">
                    <div className="w-9 h-9 rounded-xl bg-blue-500/15 flex items-center justify-center shrink-0">
                      <Network size={18} className="text-blue-300" />
                    </div>
                    <div className="flex-1">
                      <p className="text-[15px]">Host and Port</p>
                      <p className="text-[12px] text-slate-400 mt-0.5">Used for model list and chat requests</p>
                    </div>
                    <button
                      type="button"
                      onClick={() => setShowEndpointEdit(true)}
                      className="settings-edit-icon"
                      aria-label="Edit Ollama endpoint"
                    >
                      <Pencil size={14} />
                    </button>
                  </div>
                </div>
              </Card>

              {/* STT — desktop only; Android uses native speech recognition */}
              {!isAndroid && (
                <>
              <SectionHeader>Speech to Text</SectionHeader>
              <Card>
                <div className="px-4 py-3.5 flex flex-col gap-2">
                  <div className="flex items-center gap-3 mb-1">
                    <div className="w-9 h-9 rounded-xl bg-blue-500/15 flex items-center justify-center shrink-0">
                      <Mic size={18} className="text-blue-300" />
                    </div>
                    <div className="flex-1">
                      <p className="text-[15px]">Google API Key</p>
                      <p className="text-[12px] text-slate-400 mt-0.5">Google Cloud Speech-to-Text API key</p>
                    </div>
                    <button
                      type="button"
                      onClick={() => setShowSttEdit(true)}
                      className="settings-edit-icon"
                      aria-label="Edit speech to text settings"
                    >
                      <Pencil size={14} />
                    </button>
                  </div>
                </div>
              </Card>
                </>
              )}

              {/* Session */}
              <SectionHeader>Session</SectionHeader>
              <Card>
                <CardRow onClick={() => setShowQrPair(true)}>
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-xl bg-indigo-500/15 flex items-center justify-center">
                      <QrCode size={18} className="text-indigo-300" />
                    </div>
                    <div>
                      <p className="text-[15px]">Pair with QR code</p>
                      <p className="text-[12px] text-slate-400 mt-0.5">Scan or display a QR code to link devices</p>
                    </div>
                  </div>
                  <ChevronRight size={18} className="text-slate-500 shrink-0" />
                </CardRow>
              </Card>

              {/* Linked devices */}
              {(session?.paired_devices ?? []).length > 0 && (
                <>
                  <SectionHeader>Linked devices</SectionHeader>
                  <Card>
                    {(session?.paired_devices ?? []).map((dev, i) => (
                      <div key={dev.device_id}>
                        {i > 0 && <CardDivider />}
                        <div className="flex items-center justify-between px-4 py-3.5">
                          <div className="flex items-center gap-3">
                            <div className="w-9 h-9 rounded-xl bg-[#222836] flex items-center justify-center shrink-0">
                              {dev.label.toLowerCase().includes('phone') || dev.label.toLowerCase().includes('android')
                                ? <Smartphone size={17} className="text-slate-400" />
                                : <Monitor size={17} className="text-slate-400" />}
                            </div>
                            <div>
                              <p className="text-[15px]">{dev.label}</p>
                              <p className="text-[12px] text-slate-400 mt-0.5 font-mono">{dev.address}</p>
                            </div>
                          </div>
                          <div className="flex items-center gap-2">
                            <span className={`text-[12px] font-medium ${peerStatus[dev.device_id] ? 'text-green-400' : 'text-gray-500'}`}>
                              {peerStatus[dev.device_id] !== undefined
                                ? (peerStatus[dev.device_id] ? 'Online' : 'Offline')
                                : '—'}
                            </span>
                            <button
                              onClick={async () => { await removeLinkedDevice(dev.device_id); }}
                              className="p-2 rounded-full hover:bg-red-500/10 transition-colors"
                            >
                              <Trash2 size={15} className="text-red-400" />
                            </button>
                          </div>
                        </div>
                      </div>
                    ))}
                  </Card>
                </>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
