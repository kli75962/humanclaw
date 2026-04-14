import { useEffect, useRef, useState } from 'react';
import { Check, ChevronDown } from 'lucide-react';
import type { Live2DModelEntry } from '../../hooks/useLive2DModels';

interface Props {
  models: Live2DModelEntry[];
  selectedId: string | null;
  onSelect: (id: string | null) => void;
}

export function Live2DPicker({ models, selectedId, onSelect }: Props) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function onDown(e: PointerEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setOpen(false);
    }
    window.addEventListener('pointerdown', onDown);
    return () => window.removeEventListener('pointerdown', onDown);
  }, []);

  const label = selectedId ? (models.find((m) => m.id === selectedId)?.name ?? 'Unknown') : 'None';

  return (
    <div ref={menuRef} className={`settings-model-menu${open ? ' settings-model-menu--open' : ''}`}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className={`settings-model-trigger${open ? ' settings-model-trigger-open' : ''}`}
      >
        <span className="settings-model-trigger-label">{label}</span>
        <ChevronDown size={16} className={`settings-model-trigger-chevron${open ? ' settings-model-trigger-chevron-open' : ''}`} />
      </button>
      {open && (
        <div className="settings-model-dropdown">
          <button
            type="button"
            className={`settings-model-option${selectedId === null ? ' settings-model-option-active' : ''}`}
            onClick={() => { onSelect(null); setOpen(false); }}
          >
            <span>None</span>
            {selectedId === null && <Check size={14} />}
          </button>
          {models.map((m) => (
            <button
              key={m.id}
              type="button"
              className={`settings-model-option${selectedId === m.id ? ' settings-model-option-active' : ''}`}
              onClick={() => { onSelect(m.id); setOpen(false); }}
            >
              <span>{m.name}</span>
              {selectedId === m.id && <Check size={14} />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
