import { invoke } from '@tauri-apps/api/core';

export const CLAUDE_MODELS: { id: string; label: string }[] = [
  { id: 'claude-haiku-4-5-20251001', label: 'Haiku 4.5' },
  { id: 'claude-sonnet-4-6',         label: 'Sonnet 4.6' },
  { id: 'claude-opus-4-6',           label: 'Opus 4.6' },
];

export async function fetchOllamaModels(): Promise<string[]> {
  return invoke<string[]>('list_models').catch(() => []);
}
