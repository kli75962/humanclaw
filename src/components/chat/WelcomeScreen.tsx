import { memo } from 'react';
import type { WelcomeScreenProps } from '../../types';
import '../../style/WelcomeScreen.css';

function getGreeting(): string {
  const h = new Date().getHours();
  if (h < 12) return 'Good morning';
  if (h < 18) return 'Good afternoon';
  return 'Good night';
}

export const WelcomeScreen = memo(function WelcomeScreen({ onSend: _ }: WelcomeScreenProps) {
  return (
    <div className="welcome">
      <div className="welcome-header">
        <h1 className="welcome-title">{getGreeting()}</h1>
      </div>
    </div>
  );
});
