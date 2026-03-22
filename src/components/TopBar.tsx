import { memo } from 'react';
import type { TopBarProps } from '../types';
import '../style/TopBar.css';

export const TopBar = memo(function TopBar({ model }: TopBarProps) {
  return (
    <div className="topbar">
      <span className="topbar-model">{model}</span>
    </div>
  );
});
