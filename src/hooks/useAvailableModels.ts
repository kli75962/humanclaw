import { useState, useCallback, useEffect } from 'react';
import { fetchOllamaModels } from '../lib/models';

export function useAvailableModels() {
  const [ollamaModels, setOllamaModels] = useState<string[]>([]);

  const refresh = useCallback(() => {
    fetchOllamaModels().then(setOllamaModels);
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  // Only warns for Ollama models. If the list is empty (unreachable / Claude-only),
  // no warning is shown since availability cannot be determined.
  function isModelMissing(model: string): boolean {
    return ollamaModels.length > 0 && !ollamaModels.includes(model);
  }

  return { ollamaModels, refresh, isModelMissing };
}
