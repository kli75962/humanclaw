import { memo } from 'react';
import { Compass, Code, Lightbulb } from 'lucide-react';
import type { WelcomeScreenProps } from '../types';

const SUGGESTIONS = [
  { icon: Compass, text: 'Plan a trip to Tokyo', color: 'text-blue-400' },
  { icon: Lightbulb, text: 'Brainstorm app ideas', color: 'text-yellow-400' },
  { icon: Code, text: 'Write a React hook', color: 'text-purple-400' },
];

/**
 * Shown when the conversation is empty.
 * Renders a greeting headline and quick-prompt suggestion cards.
 */
export const WelcomeScreen = memo(function WelcomeScreen({ onSend }: WelcomeScreenProps) {
  return (
    <div className="flex flex-col h-full justify-center max-w-2xl mx-auto opacity-0 animate-[fadeIn_0.5s_ease-out_forwards]">
      <div className="mb-12">
        <h1 className="text-5xl font-semibold mb-2 bg-gradient-to-r from-[#4285F4] to-[#D96570] text-transparent bg-clip-text tracking-tight">
          Hello, User
        </h1>
        <h2 className="text-5xl font-semibold text-[#444746]">
          How can I help today?
        </h2>
      </div>

      <div className="flex gap-4 overflow-x-auto pb-4 scrollbar-hide">
        {SUGGESTIONS.map((item, i) => (
          <button
            key={i}
            onClick={() => onSend(item.text)}
            className="min-w-[180px] h-48 bg-[#1E1F20] hover:bg-[#2C2C2C] p-4 rounded-3xl flex flex-col justify-between transition-all text-left"
          >
            <p className="text-sm font-medium text-gray-200">{item.text}</p>
            <div className={`p-2 bg-black/20 rounded-full w-fit ${item.color}`}>
              <item.icon size={20} />
            </div>
          </button>
        ))}
      </div>
    </div>
  );
});
