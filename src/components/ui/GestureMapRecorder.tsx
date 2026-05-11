import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../../style/GestureMapRecorder.css';

interface GestureMapSummary {
  name: string;
  description?: string;
  created_at: string;
  verified_count: number;
}

interface GestureMapRecorderProps {
  appPackage: string;
  onClose: () => void;
  initialPhase?: 'list' | 'recording';
}

type Phase = 'list' | 'recording' | 'naming' | 'saved';

export function GestureMapRecorder({ appPackage, onClose, initialPhase = 'list' }: GestureMapRecorderProps) {
  const [maps, setMaps] = useState<GestureMapSummary[]>([]);
  const [phase, setPhase] = useState<Phase>(initialPhase);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [shareEnabled, setShareEnabled] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    invoke<GestureMapSummary[]>('list_gesture_maps_cmd', { appPackage })
      .then(setMaps)
      .catch(() => {});
    invoke<boolean>('get_gesture_share_setting')
      .then(setShareEnabled)
      .catch(() => {});
  }, [appPackage]);

  async function handleStartRecording() {
    setBusy(true);
    setError('');
    try {
      await invoke('start_gesture_recording_cmd');
      setPhase('recording');
    } catch (e: any) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleStopRecording() {
    setBusy(true);
    try {
      await invoke('stop_gesture_recording_cmd');
      setPhase('naming');
    } catch (e: any) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveGesture() {
    if (!name.trim()) { setError('Name is required.'); return; }
    setBusy(true);
    try {
      await invoke('save_gesture_share_setting', { enabled: shareEnabled });
      setMaps(await invoke<GestureMapSummary[]>('list_gesture_maps_cmd', { appPackage }));
      setPhase('saved');
    } catch (e: any) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(mapName: string) {
    await invoke('delete_gesture_map_cmd', { appPackage, name: mapName }).catch(() => {});
    setMaps(prev => prev.filter(m => m.name !== mapName));
  }

  async function handleShareToggle(enabled: boolean) {
    setShareEnabled(enabled);
    await invoke('save_gesture_share_setting', { enabled }).catch(() => {});
  }

  return (
    <div className="gmr-backdrop" onClick={onClose}>
      <div className="gmr-panel" onClick={e => e.stopPropagation()}>
        <div className="gmr-header">
          <span className="gmr-title">Gesture Maps</span>
          <span className="gmr-pkg">{appPackage}</span>
          <button className="gmr-close" onClick={onClose}>✕</button>
        </div>

        {phase === 'list' && (
          <>
            <div className="gmr-share-row">
              <label>
                <input type="checkbox" checked={shareEnabled} onChange={e => handleShareToggle(e.target.checked)} />
                {' '}Share recordings with community
              </label>
            </div>
            {maps.length === 0 ? (
              <p className="gmr-empty">No gesture maps recorded for this app.</p>
            ) : (
              <ul className="gmr-list">
                {maps.map(m => (
                  <li key={m.name} className="gmr-item">
                    <span className="gmr-item-name">{m.name}</span>
                    <span className="gmr-item-count">✓ {m.verified_count}</span>
                    <button className="gmr-delete" onClick={() => handleDelete(m.name)}>Delete</button>
                  </li>
                ))}
              </ul>
            )}
            <button className="gmr-btn gmr-btn-primary" disabled={busy} onClick={handleStartRecording}>
              {busy ? 'Starting…' : '● Start Recording'}
            </button>
          </>
        )}

        {phase === 'recording' && (
          <div className="gmr-recording">
            <div className="gmr-rec-badge">● REC</div>
            <p>Perform the actions on your phone now.</p>
            <p className="gmr-hint">The phone shows a REC indicator. When done, tap Stop.</p>
            <button className="gmr-btn gmr-btn-danger" disabled={busy} onClick={handleStopRecording}>
              {busy ? 'Stopping…' : '■ Stop Recording'}
            </button>
          </div>
        )}

        {phase === 'naming' && (
          <div className="gmr-naming">
            <p>Name your recording:</p>
            <input
              className="gmr-input"
              placeholder="e.g. foodpanda_login"
              value={name}
              onChange={e => setName(e.target.value.toLowerCase().replace(/\s+/g, '_'))}
            />
            <input
              className="gmr-input"
              placeholder="Description (optional)"
              value={description}
              onChange={e => setDescription(e.target.value)}
            />
            <button className="gmr-btn gmr-btn-primary" disabled={busy || !name.trim()} onClick={handleSaveGesture}>
              {busy ? 'Saving…' : 'Save'}
            </button>
          </div>
        )}

        {phase === 'saved' && (
          <div className="gmr-saved">
            <p>✓ Saved successfully{shareEnabled ? ' and queued for community upload.' : '.'}</p>
            <button className="gmr-btn gmr-btn-primary" onClick={() => { setPhase('list'); setName(''); setDescription(''); }}>
              Back to list
            </button>
          </div>
        )}

        {error && <p className="gmr-error">{error}</p>}
      </div>
    </div>
  );
}
