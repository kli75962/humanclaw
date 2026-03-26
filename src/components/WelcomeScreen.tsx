import { memo } from 'react';
import type { WelcomeScreenProps } from '../types';
import '../style/WelcomeScreen.css';

export const WelcomeScreen = memo(function WelcomeScreen({ onSend: _ }: WelcomeScreenProps) {
  return (
    <div className="welcome">
      <div className="welcome-header">
        <h1 className="welcome-title">Hello, User</h1>
        <h2 className="welcome-subtitle">How can I help today?</h2>
      </div>
    </div>
  );
});
