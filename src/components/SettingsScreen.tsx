import { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { ArrowLeft } from 'lucide-react';
import { useSession } from '../hooks/useSession';
import type { SettingsScreenProps } from '../types';
import { SegmentControl } from './SettingsUI';
import { GeneralTab } from './SettingsGeneralTab';
import { ConnectTab } from './SettingsConnectTab';
import '../style/SettingsScreen.css';

type SettingsTab = 'general' | 'connect';

export function SettingsScreen({ model, availableModels, onModelChange, onOllamaEndpointChanged, onBack }: SettingsScreenProps) {
  const [tab, setTab] = useState<SettingsTab>('general');
  const [peerStatus, setPeerStatus] = useState<Record<string, boolean>>({});
  const { session, refresh, removeLinkedDevice, setOllamaEndpoint, listPersonas, setPersona } = useSession();
  const isAndroid = session?.device.device_type === 'android';

  useEffect(() => {
    if ((session?.paired_devices ?? []).length === 0) return;
    invoke<Array<{ device_id: string; online: boolean }>>('get_all_peer_status')
      .then((list) => setPeerStatus(Object.fromEntries(list.map((p) => [p.device_id, p.online]))))
      .catch(() => {});
    const unlisten = listen<Array<{ device_id: string; online: boolean }>>('peer-status-changed', (event) => {
      setPeerStatus(Object.fromEntries(event.payload.map((p) => [p.device_id, p.online])));
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [session?.paired_devices]);

  return (
    <div className="settings-root">
      <div className="settings-header">
        <button onClick={onBack} className="settings-back-btn">
          <ArrowLeft size={22} />
        </button>
        <h1 className="settings-header-title">Settings</h1>
      </div>

      <div className="settings-tabs">
        <SegmentControl
          options={[
            { value: 'general' as const, label: 'General' },
            { value: 'connect' as const, label: 'Connect' },
          ]}
          value={tab}
          onChange={setTab}
        />
      </div>

      <div className="settings-body custom-scrollbar">
        <div className="settings-content">
          {tab === 'general' && (
            <GeneralTab
              model={model}
              availableModels={availableModels}
              onModelChange={onModelChange}
              session={session}
              listPersonas={listPersonas}
              setPersona={setPersona}
            />
          )}
          {tab === 'connect' && (
            <ConnectTab
              session={session}
              isAndroid={isAndroid}
              peerStatus={peerStatus}
              removeLinkedDevice={removeLinkedDevice}
              setOllamaEndpoint={setOllamaEndpoint}
              onOllamaEndpointChanged={onOllamaEndpointChanged}
              onPaired={refresh}
            />
          )}
        </div>
      </div>
    </div>
  );
}
