import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChevronDown, ChevronRight, Mic, Monitor, Pencil, QrCode, Smartphone, Trash2 } from 'lucide-react';
import { Modal } from './Modal';
import { ShowQrView, ScanView } from './SettingsQrPairing';
import { Card, CardDivider, CardRow, SectionHeader } from './SettingsUI';
import type { SessionConfig } from '../types';

interface ConnectTabProps {
  session: SessionConfig | null;
  isAndroid: boolean;
  peerStatus: Record<string, boolean>;
  removeLinkedDevice: (deviceId: string) => Promise<void>;
  onPaired: () => void;
}

export function ConnectTab({
  session,
  isAndroid,
  peerStatus,
  removeLinkedDevice,
  onPaired,
}: ConnectTabProps) {
  const [showQrPair, setShowQrPair] = useState(false);
  const [showSttEdit, setShowSttEdit] = useState(false);

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

  return (
    <>
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
              Or set <span style={{ fontFamily: 'monospace', color: 'var(--color-text-2)' }}>GOOGLE_API_KEY</span> in{' '}
              <span style={{ fontFamily: 'monospace', color: 'var(--color-text-2)' }}>src-tauri/.secrets</span>.
              Language codes are comma-separated, first is primary.
            </p>
          </div>
        </Modal>
      )}

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
        <CardRow onClick={() => setShowQrPair((v) => !v)}>
          <div className="settings-qr-row-left">
            <div className="settings-icon-badge settings-icon-badge--indigo">
              <QrCode size={18} />
            </div>
            <div>
              <p className="settings-item-title">Pair with QR code</p>
              <p className="settings-item-subtitle">Scan or display a QR code to link devices</p>
            </div>
          </div>
          {showQrPair
            ? <ChevronDown size={18} className="settings-chevron" />
            : <ChevronRight size={18} className="settings-chevron" />
          }
        </CardRow>

        {showQrPair && (
          <div className="settings-inline-expand">
            {isAndroid ? (
              <ScanView onPaired={() => { onPaired(); setShowQrPair(false); }} isAndroid={isAndroid} />
            ) : (
              <ShowQrView />
            )}
          </div>
        )}
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
