import { useState } from "react";

const MODELS_KEY = "phoneclaw_live2d_models";
const ACTIVE_KEY = "phoneclaw_live2d_active_model";

export interface Live2DModelEntry {
  id: string;
  name: string;
  icon: string | null; // base64 data URL (PNG/JPG), null = no custom icon
  isDefault: boolean; // true = encrypted live2d:// bundle, false = user filesystem path
  modelUrl: string; // 'live2d://localhost/...' or absolute filesystem path
  sizeKb: number; // approximate file size (for default models)
}

function loadModels(): Live2DModelEntry[] {
  try {
    return JSON.parse(localStorage.getItem(MODELS_KEY) ?? "[]");
  } catch {
    return [];
  }
}

export function useLive2DModels() {
  const [models, setModels] = useState<Live2DModelEntry[]>(loadModels);
  const [activeId, setActiveIdState] = useState<string | null>(() =>
    localStorage.getItem(ACTIVE_KEY),
  );

  function save(next: Live2DModelEntry[]) {
    setModels(next);
    localStorage.setItem(MODELS_KEY, JSON.stringify(next));
  }

  function addModel(entry: Live2DModelEntry) {
    const updated = [...models, entry];
    save(updated);
    // Auto-select if this is the first model
    if (!activeId) setActive(entry.id);
  }

  function removeModel(id: string) {
    const updated = models.filter((m) => m.id !== id);
    save(updated);
    if (activeId === id) {
      const next = updated[0] ?? null;
      setActive(next?.id ?? null);
    }
  }

  function updateModel(id: string, patch: Partial<Live2DModelEntry>) {
    save(models.map((m) => (m.id === id ? { ...m, ...patch } : m)));
  }

  function setActive(id: string | null) {
    setActiveIdState(id);
    if (id) localStorage.setItem(ACTIVE_KEY, id);
    else localStorage.removeItem(ACTIVE_KEY);
  }

  const activeModel =
    models.find((m) => m.id === activeId) ?? models[0] ?? null;

  return {
    models,
    activeId,
    activeModel,
    addModel,
    removeModel,
    updateModel,
    setActive,
  } as const;
}
