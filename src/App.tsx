import { useState, useRef, useEffect } from 'react';
import { Send, Image, Mic, Sparkles, Menu, Compass, Code, Lightbulb } from 'lucide-react';

type Message = { role: 'user' | 'model'; content: string };

function App() {
  const [input, setInput] = useState('');
  const [isThinking, setIsThinking] = useState(false);
  const [messages, setMessages] = useState<Message[]>([]); // Start empty for the "Welcome" screen
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking]);

  const handleSend = (text: string) => {
    if (!text.trim()) return;
    
    setMessages(prev => [...prev, { role: 'user', content: text }]);
    setInput('');
    setIsThinking(true);

    // Simulate Gemini Response
    setTimeout(() => {
      setIsThinking(false);
      setMessages(prev => [...prev, { 
        role: 'model', 
        content: "I can certainly help with that. I'll start the agent process to handle this request on your device." 
      }]);
    }, 1500);
  };

  return (
    <div className="flex flex-col h-screen bg-[#131314] text-[#E3E3E3] font-sans">
      
      {/* Top Bar */}
      <div className="flex justify-between items-center p-4 sticky top-0 bg-[#131314] z-10">
        <button className="p-2 hover:bg-[#2C2C2C] rounded-full transition-colors">
          <Menu size={24} className="text-gray-400" />
        </button>
        
        <div className="flex items-center gap-2 cursor-pointer hover:bg-[#1E1F20] px-3 py-1 rounded-lg transition-colors">
          <span className="text-lg font-medium bg-gradient-to-r from-blue-400 via-purple-400 to-red-400 text-transparent bg-clip-text">
            Gemini
          </span>
          <span className="text-xs text-gray-500">▼</span>
        </div>

        <div className="w-8 h-8 rounded-full bg-purple-600 flex items-center justify-center text-xs font-bold">
          U
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 overflow-y-auto px-4 pb-32 custom-scrollbar">
        
        {/* Empty State (Welcome Screen) */}
        {messages.length === 0 && (
          <div className="flex flex-col h-full justify-center max-w-2xl mx-auto opacity-0 animate-[fadeIn_0.5s_ease-out_forwards]">
            <div className="mb-12">
              <h1 className="text-5xl font-semibold mb-2 bg-gradient-to-r from-[#4285F4] to-[#D96570] text-transparent bg-clip-text tracking-tight">
                Hello, User
              </h1>
              <h2 className="text-5xl font-semibold text-[#444746]">
                How can I help today?
              </h2>
            </div>

            {/* Suggestion Cards */}
            <div className="flex gap-4 overflow-x-auto pb-4 scrollbar-hide">
              {[
                { icon: Compass, text: "Plan a trip to Tokyo", color: "text-blue-400" },
                { icon: Lightbulb, text: "Brainstorm app ideas", color: "text-yellow-400" },
                { icon: Code, text: "Write a React hook", color: "text-purple-400" },
              ].map((item, i) => (
                <button 
                  key={i}
                  onClick={() => handleSend(item.text)}
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
        )}

        {/* Chat History */}
        <div className="max-w-3xl mx-auto space-y-8 mt-4">
          {messages.map((msg, idx) => (
            <div key={idx} className={`flex gap-4 ${msg.role === 'user' ? 'flex-row-reverse' : ''}`}>
              
              {/* Avatar */}
              {msg.role === 'model' && (
                <div className="w-8 h-8 shrink-0 mt-1">
                  <Sparkles className="text-blue-400 animate-pulse" size={24} />
                </div>
              )}

              {/* Message Bubble */}
              <div className={`max-w-[85%] text-[16px] leading-7 ${
                msg.role === 'user' 
                  ? 'bg-[#2C2C2C] px-5 py-3 rounded-3xl rounded-tr-sm' 
                  : 'text-gray-100 px-0' // Gemini usually has no background bubble
              }`}>
                {msg.content}
              </div>
            </div>
          ))}

          {/* Loading Indicator */}
          {isThinking && (
            <div className="flex gap-4">
              <div className="w-8 h-8 shrink-0">
                <Sparkles className="text-blue-400 animate-spin" size={24} />
              </div>
              <div className="h-4 w-24 bg-gradient-to-r from-[#1E1F20] via-[#2C2C2C] to-[#1E1F20] bg-[length:200%_100%] animate-shimmer rounded-full mt-2"></div>
            </div>
          )}
          <div ref={scrollRef} />
        </div>
      </div>

      {/* Input Area */}
      <div className="fixed bottom-0 left-0 w-full bg-[#131314] p-4 pb-6">
        <div className="max-w-3xl mx-auto bg-[#1E1F20] rounded-full flex items-center px-2 py-2 border border-transparent focus-within:border-gray-600 transition-colors">
          
          <button className="p-3 hover:bg-[#2C2C2C] rounded-full text-gray-400 transition-colors">
            <Image size={22} />
          </button>
          
          <input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend(input)}
            placeholder="Enter a prompt here"
            className="flex-1 bg-transparent text-white placeholder-gray-400 px-2 outline-none h-full"
          />

          {input.trim() ? (
            <button 
              onClick={() => handleSend(input)}
              className="p-3 hover:bg-[#2C2C2C] rounded-full text-blue-400 transition-colors"
            >
              <Send size={22} />
            </button>
          ) : (
            <button className="p-3 hover:bg-[#2C2C2C] rounded-full text-gray-400 transition-colors">
              <Mic size={22} />
            </button>
          )}
        </div>
        <p className="text-center text-[10px] text-gray-500 mt-3">
          Gemini may display inaccurate info, including about people, so double-check its responses.
        </p>
      </div>

    </div>
  );
}

export default App;