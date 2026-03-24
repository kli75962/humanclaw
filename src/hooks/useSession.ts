import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useState } from 'react';
import type { SessionConfig } from '../types';

export function useSession() {
  const [session, setSession] = useState<SessionConfig | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await invoke<SessionConfig>('get_session');
      setSession(s);
    } catch {
      // session not available on this build
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  // Re-sync when the backend emits session-changed (e.g. a peer unpaired us).
  useEffect(() => {
    const unlisten = listen('session-changed', () => { refresh(); });
    return () => { unlisten.then((fn) => fn()); };
  }, [refresh]);

  /** Set the session hash key (must be the 64-char hex key from the app). */
  const setHashKey = useCallback(async (hashKey: string): Promise<SessionConfig> => {
    const s = await invoke<SessionConfig>('set_session_hash_key', { hashKey });
    setSession(s);
    return s;
  }, []);

  /** Remove a paired device by device_id. */
  const removeLinkedDevice = useCallback(async (deviceId: string): Promise<void> => {
    const s = await invoke<SessionConfig>('remove_paired_device', { deviceId });
    setSession(s);
  }, []);

  /** Set Ollama endpoint host/IP and port. */
  const setOllamaEndpoint = useCallback(async (host: string, port: number): Promise<SessionConfig> => {
    const s = await invoke<SessionConfig>('set_ollama_endpoint', { host, port });
    setSession(s);
    return s;
  }, []);

  /** Return available persona names from backend skills registry. */
  const listPersonas = useCallback(async (): Promise<string[]> => {
    return invoke<string[]>('list_personas');
  }, []);

  /** Set active persona. */
  const setPersona = useCallback(async (persona: string): Promise<SessionConfig> => {
    const s = await invoke<SessionConfig>('set_persona', { persona });
    setSession(s);
    return s;
  }, []);

  return {
    session,
    refresh,
    setHashKey,
    removeLinkedDevice,
    setOllamaEndpoint,
    listPersonas,
    setPersona,
  };
}
