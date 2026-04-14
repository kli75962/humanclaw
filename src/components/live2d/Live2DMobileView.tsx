import { createPortal } from 'react-dom';
import { ChevronLeft } from 'lucide-react';
import { Live2DCanvas } from './Live2DCanvas';
import { useLive2DModels } from '../../hooks/useLive2DModels';
import '../../style/Live2DMobile.css';

interface Props {
  onClose: () => void;
}

export function Live2DMobileView({ onClose }: Props) {
  const { activeModel } = useLive2DModels();
  return createPortal(
    <div className="l2d-mobile-overlay">
      <button
        className="l2d-mobile-back"
        onClick={onClose}
        aria-label="Back"
      >
        <ChevronLeft size={22} />
      </button>
      <div className="l2d-mobile-canvas-wrap">
        <Live2DCanvas modelUrl={activeModel?.modelUrl} />
      </div>
    </div>,
    document.body,
  );
}
