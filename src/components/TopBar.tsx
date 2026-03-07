import { Settings, Menu } from 'lucide-react';
import type { TopBarProps } from '../types';

/** Fixed top navigation bar. */
export function TopBar({ model, onMenuOpen, onSettingsOpen }: TopBarProps) {
  return (
    <div className="flex justify-between items-center p-4 sticky top-0 bg-[#131314] z-10">
      <button
        onClick={onMenuOpen}
        className="p-2 hover:bg-[#2C2C2C] rounded-full transition-colors"
      >
        <Menu size={22} className="text-gray-400" />
      </button>

      <span className="text-sm text-gray-500 font-mono truncate max-w-[160px]">{model}</span>

      <button
        onClick={onSettingsOpen}
        className="p-2 hover:bg-[#2C2C2C] rounded-full transition-colors"
      >
        <Settings size={22} className="text-gray-400" />
      </button>
    </div>
  );
}
