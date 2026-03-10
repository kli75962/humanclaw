import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft, Camera, ChevronRight, ImagePlus, Monitor, Save, QrCode, Cpu, Smartphone, Trash2, Mic } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { scan, Format } from '@tauri-apps/plugin-barcode-scanner';
import jsQR from 'jsqr';
import { Modal } from './Modal';
import { useSession } from '../hooks/useSession';
import type { SettingsScreenProps } from '../types';

// ── Types ───────────────────────────────────────────────────────────────────────────────

type MemoryFile = 'core.md' | 'conversations.jsonl';
type SettingsTab = 'general' | 'memory';

// ── Memory file tab config ─────────────────────────────────────────────────────────

const MEMORY_TABS: { file: MemoryFile; label: string; desc: string }[] = [
  {
    file: 'core.md',
    label: 'Core',
    desc: 'Injected into every prompt — keep short. User preferences, name, language.',
  },
  {
    file: 'conversations.jsonl',
    label: 'Recall',
    desc: 'Last 50 conversation summaries (JSONL). Used by the LLM to search past sessions.',
  },
];

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
    <div className="flex bg-[#1E1F20] rounded-full p-1">
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className={`flex-1 py-2 text-sm font-medium rounded-full transition-all ${
            value === opt.value
              ? 'bg-purple-600 text-white shadow-sm'
              : 'text-gray-400 hover:text-gray-200'
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
    <p className="px-1 pb-2 pt-6 text-[11px] font-bold text-gray-500 uppercase tracking-wider">
      {children}
    </p>
  );
}

function SectionFooter({ children }: { children: React.ReactNode }) {
  return (
    <p className="px-1 pt-2 pb-1 text-[12px] text-gray-500 leading-relaxed">
      {children}
    </p>
  );
}

function Card({ children }: { children: React.ReactNode }) {
  return (
    <div className="bg-[#1E1F20] rounded-2xl overflow-hidden">
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
      className="w-full flex items-center justify-between px-4 py-3.5 text-left hover:bg-white/[0.03] transition-colors"
    >
      {children}
    </Tag>
  );
}

function CardDivider() {
  return <div className="h-px bg-white/[0.06] mx-4" />;
}

function Radio({ active }: { active: boolean }) {
  return (
    <div
      className={`w-[22px] h-[22px] rounded-full border-2 flex items-center justify-center shrink-0 transition-colors ${
        active ? 'border-purple-500 bg-purple-500' : 'border-gray-600'
      }`}
    >
      {active && <div className="w-2 h-2 rounded-full bg-white" />}
    </div>
  );
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
                className="px-2 py-1 text-xs rounded-lg bg-[#2C2C2C] text-gray-300"
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
            className="flex-1 bg-[#2C2C2C] text-gray-200 text-sm px-3 py-2 rounded-xl outline-none focus:ring-1 focus:ring-purple-500"
          />
          <button
            onClick={fetchPublicIp}
            disabled={fetchingPublicIp}
            className="px-3 py-2 rounded-xl bg-purple-600/20 text-purple-400 text-xs hover:bg-purple-600/30 transition-colors disabled:opacity-50"
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
      const result = await scan({ formats: [Format.QRCode], windowed: false });
      await pairWithPayload(result.content);
    } catch (e) {
      const msg = e instanceof Error ? e.message : typeof e === 'object' ? JSON.stringify(e) : String(e);
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
            className="flex items-center gap-2 px-6 py-3 rounded-xl bg-purple-600 text-sm font-medium text-white hover:bg-purple-500 transition-colors"
          >
            <Camera size={16} />
            Scan QR Code
          </button>
          <button
            onClick={() => fileInputRef.current?.click()}
            className="flex items-center gap-2 px-5 py-2.5 rounded-xl bg-[#2C2C2C] text-sm text-gray-300 hover:bg-[#383838] transition-colors"
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
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[#2C2C2C] text-sm text-gray-200 hover:bg-[#383838] transition-colors"
            >
              <Camera size={14} />
              Try again
            </button>
            <button
              onClick={() => fileInputRef.current?.click()}
              className="flex items-center gap-2 px-4 py-2 rounded-xl bg-[#2C2C2C] text-sm text-gray-200 hover:bg-[#383838] transition-colors"
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

export function SettingsScreen({ model, availableModels, onModelChange, onBack }: SettingsScreenProps) {
  const [tab, setTab] = useState<SettingsTab>('general');
  const [showQrPair, setShowQrPair] = useState(false);
  const [showModelSelect, setShowModelSelect] = useState(false);
  const [googleApiKey, setGoogleApiKey] = useState('');

  useEffect(() => {
    invoke<string | null>('load_secret', { key: 'google_api_key' })
      .then((val) => { if (val) setGoogleApiKey(val); })
      .catch(() => {});
  }, []);

  const [activeMemTab, setActiveMemTab] = useState<MemoryFile>('core.md');
  const [fileContent, setFileContent] = useState('');
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [saveMsg, setSaveMsg] = useState('');
  const [peerStatus, setPeerStatus] = useState<Record<string, boolean>>({});
  const { session, refresh, removeLinkedDevice } = useSession();
  const isAndroid = session?.device.device_type === 'android';
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    setDirty(false);
    setSaveMsg('');
    invoke<string>('get_memory_file', { filename: activeMemTab })
      .then((content) => setFileContent(content))
      .catch(() => setFileContent(''));
  }, [activeMemTab]);

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

  async function handleSave() {
    setSaving(true);
    try {
      await invoke('set_memory_file', { filename: activeMemTab, content: fileContent });
      setDirty(false);
      setSaveMsg('Saved');
      setTimeout(() => setSaveMsg(''), 2000);
    } catch {
      setSaveMsg('Error saving');
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="flex flex-col h-screen bg-[#131314] text-[#E3E3E3]">
      {showQrPair && (
        <Modal title={isAndroid ? 'Scan QR to Link' : 'Show QR to Link'} onClose={() => setShowQrPair(false)}>
          {isAndroid ? (
            <ScanView onPaired={() => { refresh(); setShowQrPair(false); }} isAndroid={isAndroid} />
          ) : (
            <ShowQrView />
          )}
        </Modal>
      )}
      {showModelSelect && (
        <Modal title="Select Model" onClose={() => setShowModelSelect(false)}>
          {availableModels.length === 0 ? (
            <p className="text-sm text-gray-500 text-center py-4">No models found — is Ollama running?</p>
          ) : (
            <div className="flex flex-col -mx-5">
              {availableModels.map((m, i) => (
                <div key={m}>
                  {i > 0 && <CardDivider />}
                  <button
                    onClick={() => { onModelChange(m); setShowModelSelect(false); }}
                    className="w-full flex items-center justify-between px-5 py-3.5 text-left hover:bg-white/[0.03] transition-colors"
                  >
                    <p className="text-[15px]">{m}</p>
                    <Radio active={m === model} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </Modal>
      )}

      {/* Header */}
      <div className="flex items-center gap-3 px-2 py-3 border-b border-[#2C2C2C]">
        <button
          onClick={onBack}
          className="p-2 hover:bg-[#2C2C2C] rounded-full transition-colors"
        >
          <ArrowLeft size={22} className="text-gray-400" />
        </button>
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>

      {/* Segment tabs */}
      <div className="px-4 pb-3">
        <SegmentControl
          options={[
            { value: 'general' as const, label: 'General' },
            { value: 'memory' as const, label: 'Memory' },
          ]}
          value={tab}
          onChange={setTab}
        />
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto px-4 custom-scrollbar">
        <div className="max-w-2xl mx-auto pb-12">
          {tab === 'general' ? (
            <>
              {/* Model */}
              <SectionHeader>Model</SectionHeader>
              <Card>
                <CardRow onClick={() => setShowModelSelect(true)}>
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-xl bg-purple-500/15 flex items-center justify-center">
                      <Cpu size={18} className="text-purple-400" />
                    </div>
                    <div>
                      <p className="text-[15px]">Active model</p>
                      <p className="text-[12px] text-gray-500 mt-0.5 font-mono">{model || 'None selected'}</p>
                    </div>
                  </div>
                  <ChevronRight size={18} className="text-gray-500 shrink-0" />
                </CardRow>
              </Card>
              <SectionFooter>
                Models are loaded from your local Ollama instance.
              </SectionFooter>

              {/* STT — desktop only; Android uses native speech recognition */}
              {!isAndroid && (
                <>
              <SectionHeader>Speech to Text</SectionHeader>
              <Card>
                <div className="px-4 py-3.5 flex flex-col gap-2">
                  <div className="flex items-center gap-3 mb-1">
                    <div className="w-9 h-9 rounded-xl bg-blue-500/15 flex items-center justify-center shrink-0">
                      <Mic size={18} className="text-blue-400" />
                    </div>
                    <div>
                      <p className="text-[15px]">Google API Key</p>
                      <p className="text-[12px] text-gray-500 mt-0.5">Google Cloud Speech-to-Text API key</p>
                    </div>
                  </div>
                  <input
                    type="password"
                    value={googleApiKey}
                    onChange={(e) => {
                      setGoogleApiKey(e.target.value);
                      invoke('store_secret', { key: 'google_api_key', value: e.target.value }).catch(() => {});
                    }}
                    placeholder="AIzaSy..."
                    autoComplete="off"
                    className="bg-[#131314] border border-[#2C2C2C] rounded-xl px-4 py-2.5 text-sm font-mono text-gray-300 focus:outline-none focus:ring-1 focus:ring-blue-500/50"
                  />
                </div>
              </Card>
              <SectionFooter>
                Or set{' '}
                <span className="font-mono text-gray-300">GOOGLE_API_KEY</span>{' '}
                in <span className="font-mono text-gray-300">src-tauri/.secrets</span>.
              </SectionFooter>
                </>
              )}

              {/* Session */}
              <SectionHeader>Session</SectionHeader>
              <Card>
                <CardRow onClick={() => setShowQrPair(true)}>
                  <div className="flex items-center gap-3">
                    <div className="w-9 h-9 rounded-xl bg-purple-500/15 flex items-center justify-center">
                      <QrCode size={18} className="text-purple-400" />
                    </div>
                    <div>
                      <p className="text-[15px]">Pair with QR code</p>
                      <p className="text-[12px] text-gray-500 mt-0.5">Scan or display a QR code to link devices</p>
                    </div>
                  </div>
                  <ChevronRight size={18} className="text-gray-500 shrink-0" />
                </CardRow>
              </Card>
              <SectionFooter>
                Pair devices to sync sessions automatically.
              </SectionFooter>

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
                            <div className="w-9 h-9 rounded-xl bg-[#2C2C2C] flex items-center justify-center shrink-0">
                              {dev.label.toLowerCase().includes('phone') || dev.label.toLowerCase().includes('android')
                                ? <Smartphone size={17} className="text-gray-400" />
                                : <Monitor size={17} className="text-gray-400" />}
                            </div>
                            <div>
                              <p className="text-[15px]">{dev.label}</p>
                              <p className="text-[12px] text-gray-500 mt-0.5 font-mono">{dev.address}</p>
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
          ) : (
            <>
              {/* Memory sub-tabs */}
              <div className="pt-2 pb-4">
                <SegmentControl
                  options={MEMORY_TABS.map((t) => ({ value: t.file, label: t.label }))}
                  value={activeMemTab}
                  onChange={setActiveMemTab}
                />
              </div>

              <Card>
                <div className="px-4 py-3">
                  <p className="text-[13px] text-gray-400 leading-relaxed">
                    {MEMORY_TABS.find((t) => t.file === activeMemTab)?.desc}
                  </p>
                </div>
              </Card>

              <div className="mt-3">
                <Card>
                  <div className="p-3">
                    <textarea
                      ref={textareaRef}
                      value={fileContent}
                      onChange={(e) => { setFileContent(e.target.value); setDirty(true); setSaveMsg(''); }}
                      className="w-full h-64 bg-[#131314] border border-[#2C2C2C] rounded-xl px-4 py-3 text-sm font-mono text-gray-300 focus:outline-none focus:ring-1 focus:ring-purple-500/50 resize-none leading-relaxed transition-all"
                      spellCheck={false}
                      placeholder={`No content in ${activeMemTab}`}
                    />
                  </div>
                </Card>
              </div>

              {(dirty || saveMsg) && (
                <div className="mt-3 flex justify-end">
                  {dirty && (
                    <button
                      onClick={handleSave}
                      disabled={saving}
                      className="flex items-center gap-1.5 text-sm font-medium text-white bg-purple-600 hover:bg-purple-500 px-4 py-2 rounded-full transition-colors disabled:opacity-50"
                    >
                      <Save size={14} />
                      {saving ? 'Saving…' : 'Save changes'}
                    </button>
                  )}
                  {saveMsg && !dirty && (
                    <span className="text-sm font-medium text-green-400 bg-green-500/10 px-4 py-2 rounded-full">
                      {saveMsg}
                    </span>
                  )}
                </div>
              )}

              <SectionFooter>
                The LLM can read and write these files using the{' '}
                <span className="font-mono bg-white/5 px-1 py-0.5 rounded text-gray-400">memory</span> tool.
              </SectionFooter>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
