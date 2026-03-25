import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Character } from '../types';

export function useCharacters() {
  const [characters, setCharacters] = useState<Character[]>([]);

  const refresh = useCallback(() => {
    invoke<Character[]>('list_characters').then(setCharacters).catch(() => {});
  }, []);

  // Load on mount + re-sync when the backend emits a sync event
  useEffect(() => {
    refresh();
    const unlisten = listen('character-sync-updated', refresh);
    return () => { unlisten.then((fn) => fn()); };
  }, [refresh]);

  const addCharacter = useCallback(async (data: Omit<Character, 'id' | 'createdAt'>): Promise<Character> => {
    const newChar: Character = {
      ...data,
      id: crypto.randomUUID(),
      createdAt: new Date().toISOString(),
    };
    await invoke('save_character', { character: newChar });
    setCharacters((prev) => [newChar, ...prev]);
    return newChar;
  }, []);

  const deleteCharacter = useCallback(async (id: string) => {
    await invoke('delete_character', { id }).catch(() => {});
    setCharacters((prev) => prev.filter((c) => c.id !== id));
  }, []);

  return { characters, addCharacter, deleteCharacter };
}
