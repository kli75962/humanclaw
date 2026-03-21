import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChevronRight, Mic, Monitor, Network, Pencil, QrCode, Smartphone, Trash2 } from 'lucide-react';
import { Modal } from './Modal';
import { ShowQrView, ScanView } from './SettingsQrPairing';
import { Card, CardDivider, CardRow, SectionHeader } from './SettingsUI';
import type { SessionConfig } from '../types';

interface ConnectTabProps {
  session: SessionConfig | null;
  isAndroid: boolean;
  peerStatus: Record<string, boolean>;
  removeLinkedDevice: (deviceId: string) => Promise<void>;
  setOllamaEndpoint: (host: string, port: number) => Promise<SessionConfig>;
  onOllamaEndpointChanged: () => void;
  onPaired: () => void;
}

export function ConnectTab({
  session,
  isAndroid,
  peerStatus,
  removeLinkedDevice,
  setOllamaEndpoint,
  onOllamaEndpointChanged,
  onPaired,
}: ConnectTabProps) {
  const [showQrPair, setShowQrPair] = useState(false);
  const [showEndpointEdit, setShowEndpointEdit] = useState(false);
  const [showSttEdit, setShowSttEdit] = useState(false);

  const [ollamaHost, setOllamaHost] = useState('127.0.0.1');
  const [ollamaPort, setOllamaPort] = useState('11434');
  const [ollamaSaving, setOllamaSaving] = useState(false);
  const [ollamaSaveMsg, setOllamaSaveMsg] = useState('');

  const [googleApiKey, setGoogleApiKey] = useState('');
  const [googleSttLanguages, setGoogleSttLanguages] = useState('en-US,yue-Hant-HK,cmn-Hans-CN');

  useEffect(() => {
    invoke<string | null>('load_secret', { key: 'google_api_key' })
      .then((val) => { if (val) setGoogleApiKey(val); })
      .catch(() => {});
    invoke<string | null>('load_secret', { key: 'google_stt_languages' })
      .then((val) => { if (val) setGoogleSttLanguages(val); })
      .catch(() => {});
  }, []);

  useEffect(() => {
    const fallbackHost = isAndroid
      ? (session?.paired_devices?.[0]?.address.split(':')[0] ?? '127.0.0.1')
      : '127.0.0.1';
    const host = (session?.ollama_host_override ?? '').trim();
    setOllamaHost(host || fallbackHost);
    setOllamaPort(String(session?.ollama_port ?? 11434));
  }, [isAndroid, session?.ollama_host_override, session?.ollama_port, session?.paired_devices]);

  async function handleSaveOllamaEndpoint() {
    const host = ollamaHost.trim();
    const port = Number.parseInt(ollamaPort.trim(), 10);
    if (!host) { setOllamaSaveMsg('Host is required'); return; }
    if (!Number.isFinite(port) || port < 1 || port > 65535) { setOllamaSaveMsg('Port must be 1-65535'); return; }

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
    <>
      {showQrPair && (
        <Modal title={isAndroid ? 'Scan QR to Link' : 'Show QR to Link'} onClose={() => setShowQrPair(false)}>
          {isAndroid ? (
            <ScanView onPaired={() => { onPaired(); setShowQrPair(false); }} isAndroid={isAndroid} />
          ) : (
            <ShowQrView />
          )}
        </Modal>
      )}

      {showEndpointEdit && (
        <Modal title="Edit Ollama Endpoint" onClose={() => setShowEndpointEdit(false)}>
          <div className="settings-edit-modal-body">
            <p>Host and port for model list and chat requests.</p>
            <p>Enter only host/IP and port. Example: 192.168.1.10 and 11434.</p>
            <div className="settings-endpoint-grid">
              <input
                value={ollamaHost}
                onChange={(e) => { setOllamaHost(e.target.value); setOllamaSaveMsg(''); }}
                placeholder="127.0.0.1"
                className="settings-popup-input settings-endpoint-host"
                style={{ marginTop: 0 }}
              />
              <input
                value={ollamaPort}
                onChange={(e) => { setOllamaPort(e.target.value); setOllamaSaveMsg(''); }}
                placeholder="11434"
                inputMode="numeric"
                className="settings-popup-input"
                style={{ marginTop: 0 }}
              />
            </div>
            <div className="settings-edit-modal-actions">
              <p className={ollamaSaveMsg === 'Saved' ? 'settings-save-msg--ok' : 'settings-save-msg--err'}>
                {ollamaSaveMsg || ' '}
              </p>
              <button onClick={handleSaveOllamaEndpoint} disabled={ollamaSaving} className="settings-save-btn">
                {ollamaSaving ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        </Modal>
      )}

      {showSttEdit && !isAndroid && (
        <Modal title="Edit Speech to Text" onClose={() => setShowSttEdit(false)}>
          <div className="settings-edit-modal-body">
            <p>Google Cloud Speech-to-Text API settings.</p>
            <input
              type="password"
              value={googleApiKey}
              onChange={(e) => {
                setGoogleApiKey(e.target.value);
                invoke('store_secret', { key: 'google_api_key', value: e.target.value }).catch(() => {});
              }}
              placeholder="AIzaSy..."
              autoComplete="off"
              className="settings-popup-input"
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
              className="settings-popup-input"
            />
            <p style={{ marginTop: 8 }}>
              Or set <span style={{ fontFamily: 'monospace', color: '#CBD5E1' }}>GOOGLE_API_KEY</span> in{' '}
              <span style={{ fontFamily: 'monospace', color: '#CBD5E1' }}>src-tauri/.secrets</span>.
              Language codes are comma-separated, first is primary.
            </p>
          </div>
        </Modal>
      )}

      <SectionHeader>Ollama Endpoint</SectionHeader>
      <Card>
        <div className="settings-card-body">
          <div className="settings-item-header">
            <div className="settings-icon-badge settings-icon-badge--blue">
              <Network size={18} />
            </div>
            <div className="settings-item-info">
              <p className="settings-item-title">Host and Port</p>
              <p className="settings-item-subtitle">Used for model list and chat requests</p>
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

      {!isAndroid && (
        <>
          <SectionHeader>Speech to Text</SectionHeader>
          <Card>
            <div className="settings-card-body">
              <div className="settings-item-header">
                <div className="settings-icon-badge settings-icon-badge--blue">
                  <Mic size={18} />
                </div>
                <div className="settings-item-info">
                  <p className="settings-item-title">Google API Key</p>
                  <p className="settings-item-subtitle">Google Cloud Speech-to-Text API key</p>
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

      <SectionHeader>Session</SectionHeader>
      <Card>
        <CardRow onClick={() => setShowQrPair(true)}>
          <div className="settings-qr-row-left">
            <div className="settings-icon-badge settings-icon-badge--indigo">
              <QrCode size={18} />
            </div>
            <div>
              <p className="settings-item-title">Pair with QR code</p>
              <p className="settings-item-subtitle">Scan or display a QR code to link devices</p>
            </div>
          </div>
          <ChevronRight size={18} className="settings-chevron" />
        </CardRow>
      </Card>

      {(session?.paired_devices ?? []).length > 0 && (
        <>
          <SectionHeader>Linked devices</SectionHeader>
          <Card>
            {(session?.paired_devices ?? []).map((dev, i) => (
              <div key={dev.device_id}>
                {i > 0 && <CardDivider />}
                <div className="settings-device-row">
                  <div className="settings-device-left">
                    <div className="settings-icon-badge settings-icon-badge--neutral">
                      {dev.label.toLowerCase().includes('phone') || dev.label.toLowerCase().includes('android')
                        ? <Smartphone size={17} />
                        : <Monitor size={17} />}
                    </div>
                    <div>
                      <p className="settings-item-title">{dev.label}</p>
                      <p className="settings-device-info-address">{dev.address}</p>
                    </div>
                  </div>
                  <div className="settings-device-right">
                    <span className={peerStatus[dev.device_id] ? 'settings-device-status--online' : 'settings-device-status--offline'}>
                      {peerStatus[dev.device_id] !== undefined
                        ? (peerStatus[dev.device_id] ? 'Online' : 'Offline')
                        : '—'}
                    </span>
                    <button
                      onClick={async () => { await removeLinkedDevice(dev.device_id); }}
                      className="settings-device-remove-btn"
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </Card>
        </>
      )}
    </>
  );
}
