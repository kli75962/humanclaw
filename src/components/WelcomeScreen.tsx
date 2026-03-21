import { memo } from 'react';
import { Compass, Code, Lightbulb } from 'lucide-react';
import type { WelcomeScreenProps } from '../types';
import '../style/WelcomeScreen.css';

const SUGGESTIONS = [
  { icon: Compass, text: 'Plan a trip to Tokyo', colorClass: 'welcome-icon--blue' },
  { icon: Lightbulb, text: 'Brainstorm app ideas', colorClass: 'welcome-icon--yellow' },
  { icon: Code, text: 'Write a React hook', colorClass: 'welcome-icon--purple' },
];

export const WelcomeScreen = memo(function WelcomeScreen({ onSend }: WelcomeScreenProps) {
  return (
    <div className="welcome">
      <div className="welcome-header">
        <h1 className="welcome-title">Hello, User</h1>
        <h2 className="welcome-subtitle">How can I help today?</h2>
      </div>

      <div className="welcome-suggestions">
        {SUGGESTIONS.map((item, i) => (
          <button key={i} onClick={() => onSend(item.text)} className="welcome-card">
            <p className="welcome-card-text">{item.text}</p>
            <div className={`welcome-card-icon ${item.colorClass}`}>
              <item.icon size={20} />
            </div>
          </button>
        ))}
      </div>
    </div>
  );
});
