import { memo } from 'react';
import { Settings, Menu } from 'lucide-react';
import type { TopBarProps } from '../types';
import '../style/TopBar.css';

export const TopBar = memo(function TopBar({ model, onMenuOpen, onSettingsOpen }: TopBarProps) {
  return (
    <div className="topbar">
      <button onClick={onMenuOpen} className="topbar-btn">
        <Menu size={22} />
      </button>

      <span className="topbar-model">{model}</span>

      <button onClick={onSettingsOpen} className="topbar-btn">
        <Settings size={22} />
      </button>
    </div>
  );
});
