import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const isAndroid = () => /android/i.test(navigator.userAgent);

/**
 * Speech-to-text with platform-aware backend:
 * - Android: native STT loop via Tauri mobile plugin (stt_android_once)
 * - Desktop: Google Cloud STT API via Tauri command (stt_start / stt_stop)
 */
export function useStt(onTranscript: (text: string) => void) {
  const [isListening, setIsListening] = useState(false);
  const [sttError, setSttError] = useState<string | null>(null);

  const onTranscriptRef = useRef(onTranscript);
  onTranscriptRef.current = onTranscript;

  const isListeningRef = useRef(false);

  const isTransientAndroidSttError = (msg: string) => {
    const lower = msg.toLowerCase();
    return (
      lower.includes('no speech input') ||
      lower.includes('no speech matched') ||
      lower.includes('timed out') ||
      lower.includes('client error')
    );
  };

  // Desktop only: listen for partial transcripts emitted by the Rust streaming task.
  useEffect(() => {
    if (isAndroid()) return;
    const unlisten = listen<{ text: string }>('stt-partial', (event) => {
      if (isListeningRef.current) {
        onTranscriptRef.current(event.payload.text);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const startListening = useCallback(async () => {
    setSttError(null);

    if (isAndroid()) {
      // ── Android: trigger-style native STT loop ───────────────────────────────
      try {
        if (isListeningRef.current) return;
        isListeningRef.current = true;
        setIsListening(true);
        let runningTranscript = '';

        while (isListeningRef.current) {
          try {
            const text = (await invoke<string>('stt_android_once')).trim();
            if (!isListeningRef.current) break;
            if (!text) continue;

            runningTranscript = runningTranscript
              ? `${runningTranscript} ${text}`
              : text;
            onTranscriptRef.current(runningTranscript);
          } catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            if (!isListeningRef.current) break;
            if (isTransientAndroidSttError(msg)) {
              continue;
            }
            setSttError(`STT error: ${msg}`);
            break;
          }
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setSttError(`STT error: ${msg}`);
      } finally {
        isListeningRef.current = false;
        setIsListening(false);
      }
    } else {
      // ── Desktop: Google Cloud STT via Tauri command ──────────────────────────
      try {
        const apiKey = await invoke<string | null>('load_secret', { key: 'google_api_key' }) ?? undefined;
        await invoke('stt_start', { apiKey });
        isListeningRef.current = true;
        setIsListening(true);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setSttError(`STT start failed: ${msg}`);
      }
    }
  }, []);

  const stopListening = useCallback(async () => {
    isListeningRef.current = false;
    setIsListening(false);

    if (isAndroid()) {
      // Cancel active native recognizer so stop is immediate.
      invoke('stt_android_cancel').catch(() => {});
    } else {
      try {
        const text = await invoke<string>('stt_stop');
        if (text) onTranscriptRef.current(text);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setSttError(msg);
      }
    }
  }, []);

  return { isListening, sttError, startListening, stopListening } as const;
}

