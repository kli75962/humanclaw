import { X } from 'lucide-react';
import type { ModalProps } from '../types';

/** Reusable centered popup overlay. */
export function Modal({ title, onClose, children }: ModalProps) {
  return (
    <div
      style={{ zIndex: 60 }}
      className="fixed inset-0 flex items-end sm:items-center justify-center bg-black/60 px-0 sm:px-4"
      onClick={onClose}
    >
      <div
        className="w-full sm:max-w-sm bg-[#1E1F20] rounded-t-2xl sm:rounded-2xl shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4">
          <span className="text-sm font-semibold text-[#E3E3E3]">{title}</span>
          <button
            onClick={onClose}
            className="w-8 h-8 flex items-center justify-center rounded-full bg-[#2C2C2C] hover:bg-[#3C3C3C] transition-colors"
          >
            <X size={14} className="text-gray-400" />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 pb-6 pt-1">{children}</div>
      </div>
    </div>
  );
}
