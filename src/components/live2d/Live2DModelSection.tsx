import { useEffect, useRef, useState } from 'react';
import { Plus, Trash2, User } from 'lucide-react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { useLive2DModels, type Live2DModelEntry } from '../../hooks/useLive2DModels';
import '../../style/Live2DModelSection.css';

const BUNDLE_URL = 'live2d://localhost/model/model.model3.json';

export function Live2DModelSection() {
  const { models, addModel, removeModel, updateModel } = useLive2DModels();
  const [confirmId, setConfirmId] = useState<string | null>(null);
  const iconFileRefs = useRef<Map<string, HTMLInputElement>>(new Map());

  // Auto-detect encrypted bundle model on first render
  useEffect(() => {
    const alreadyListed = models.some((m) => m.modelUrl === BUNDLE_URL);
    if (alreadyListed) return;
    fetch(BUNDLE_URL)
      .then((r) => {
        if (!r.ok) return;
        addModel({
          id: 'default-bundle',
          name: 'Default Character',
          icon: null,
          isDefault: true,
          modelUrl: BUNDLE_URL,
          sizeKb: 0,
        });
      })
      .catch(() => {});
  // only run once on mount
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Import a new model via native file dialog — returns full OS path
  async function handleImportModel() {
    const path = await openDialog({
      title: 'Select Live2D model',
      filters: [{ name: 'Live2D Model', extensions: ['json'] }],
      multiple: false,
      directory: false,
    });
    if (!path || typeof path !== 'string') return;
    if (!path.endsWith('.model3.json')) {
      alert('Please select a .model3.json file.');
      return;
    }
    const sep = path.includes('\\') ? '\\' : '/';
    const name = path.split(sep).pop()?.replace('.model3.json', '') ?? 'Model';
    addModel({
      id: `model-${Date.now()}`,
      name,
      icon: null,
      isDefault: false,
      modelUrl: path,
      sizeKb: 0,
    });
  }

  // Pick a PNG/JPG as the character icon
  function handleIconPick(id: string, e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => updateModel(id, { icon: reader.result as string });
    reader.readAsDataURL(file);
    e.target.value = '';
  }

  return (
    <div className="live2d-model-section">
      {/* First row: import button */}
      <div className="live2d-model-row live2d-model-row--import">
        <button
          type="button"
          className="live2d-import-btn"
          onClick={handleImportModel}
        >
          <Plus size={15} />
          Import model
        </button>
      </div>

      {models.length === 0 && (
        <p className="live2d-empty-hint">No characters yet. Import a .model3.json file above.</p>
      )}

      {models.map((model: Live2DModelEntry) => (
        <div key={model.id} className="live2d-model-entry">
          <div className="live2d-model-row">
            {/* Icon square — click to pick PNG/JPG */}
            <button
              type="button"
              className="live2d-model-icon-btn"
              onClick={() => iconFileRefs.current.get(model.id)?.click()}
              title="Click to change icon"
            >
              {model.icon
                ? <img src={model.icon} alt={model.name} className="live2d-model-icon-img" />
                : <User size={20} />
              }
            </button>
            <input
              ref={(el) => {
                if (el) iconFileRefs.current.set(model.id, el);
                else     iconFileRefs.current.delete(model.id);
              }}
              type="file"
              accept="image/png,image/jpeg"
              style={{ display: 'none' }}
              onChange={(e) => handleIconPick(model.id, e)}
            />

            {/* Editable name */}
            <input
              key={model.id + '-name'}
              className="live2d-model-name-input"
              defaultValue={model.name}
              onBlur={(e) => {
                const trimmed = e.target.value.trim();
                if (trimmed && trimmed !== model.name) updateModel(model.id, { name: trimmed });
              }}
            />

            {/* Remove button */}
            <button
              type="button"
              className="live2d-remove-btn"
              onClick={() => setConfirmId(confirmId === model.id ? null : model.id)}
              title="Remove"
            >
              <Trash2 size={14} />
            </button>
          </div>

          {/* Inline confirmation */}
          {confirmId === model.id && (
            <div className="live2d-confirm-row">
              <p className="live2d-confirm-msg">
                {model.isDefault
                  ? `This character is bundled with the app and this action is not undoable${model.sizeKb > 0 ? `, it will free ~${model.sizeKb} KB` : ''}. Are you sure?`
                  : 'Are you sure you want to remove this character?'
                }
              </p>
              <div className="live2d-confirm-btns">
                <button
                  type="button"
                  className="live2d-confirm-yes"
                  onClick={() => { removeModel(model.id); setConfirmId(null); }}
                >
                  Yes
                </button>
                <button
                  type="button"
                  className="live2d-confirm-no"
                  onClick={() => setConfirmId(null)}
                >
                  No
                </button>
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
