import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

const isAndroid = () => /android/i.test(navigator.userAgent);

/**
 * Speech-to-text with platform-aware backend:
 * - Android: native STT via Web Speech API (webkitSpeechRecognition)
 * - Desktop: Google Cloud STT API via Tauri command (stt_start / stt_stop)
 */
export function useStt(onTranscript: (text: string) => void) {
  const [isListening, setIsListening] = useState(false);
  const [sttError, setSttError] = useState<string | null>(null);

  const onTranscriptRef = useRef(onTranscript);
  onTranscriptRef.current = onTranscript;

  const isListeningRef = useRef(false);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const recognitionRef = useRef<any>(null);

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
      // ── Android: native STT via Web Speech API ──────────────────────────────
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const SR = (window as any).SpeechRecognition ?? (window as any).webkitSpeechRecognition;
      if (!SR) {
        setSttError('Speech recognition not available on this device');
        return;
      }
      const recognition = new SR();
      recognition.continuous = true;
      recognition.interimResults = true;
      recognition.lang = 'en-US';

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      recognition.onresult = (e: any) => {
        let interim = '';
        for (let i = e.resultIndex; i < e.results.length; i++) {
          interim += e.results[i][0].transcript;
        }
        if (interim) onTranscriptRef.current(interim);
      };

      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      recognition.onerror = (e: any) => {
        setSttError(`STT error: ${e.error}`);
        isListeningRef.current = false;
        setIsListening(false);
      };

      recognition.onend = () => {
        // Restart automatically while still in listening state.
        if (isListeningRef.current) recognition.start();
      };

      recognitionRef.current = recognition;
      recognition.start();
      isListeningRef.current = true;
      setIsListening(true);
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
      recognitionRef.current?.stop();
      recognitionRef.current = null;
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

