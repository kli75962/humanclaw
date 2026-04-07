import { X } from 'lucide-react';
import type { ModalProps } from '../../types';
import '../../style/Modal.css';

/** Reusable centered popup overlay. */
export function Modal({ title, onClose, children }: ModalProps) {
  return (
    <div
      className="pc-modal-overlay"
      onClick={onClose}
    >
      <div
        className="pc-modal"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="pc-modal-header">
          <span className="pc-modal-title">{title}</span>
          <button
            onClick={onClose}
            className="pc-modal-close"
          >
            <X size={14} className="pc-modal-close-icon" />
          </button>
        </div>

        {/* Body */}
        <div className="pc-modal-body">{children}</div>
      </div>
    </div>
  );
}
