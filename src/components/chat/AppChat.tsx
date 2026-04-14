import { useRef, useMemo, useEffect, useCallback } from 'react';
import { ExplainPopup } from '../ui/ExplainPopup';
import { WelcomeScreen } from './WelcomeScreen';
import { ChatMessage } from './ChatMessage';
import { InputBar } from './InputBar';
import { PermissionRequest } from '../ui/PermissionDialog';
import { AskUserBubble } from '../ui/AskUserBubble';
import { MemoChatView } from './MemoChatView';
import { Live2DMobileView } from '../live2d/Live2DMobileView';
import { File, Sparkles } from 'lucide-react';
import { useExplainPopup } from '../../hooks/useExplainPopup';
import { useFileDragDrop } from '../../hooks/useFileDragDrop';
import { useLive2D } from '../../hooks/useLive2D';
import { useLive2DModels } from '../../hooks/useLive2DModels';
import type { Message, Post, Character, InputBarHandle } from '../../types';
import type { PermissionRequest as PermissionRequestData } from '../ui/PermissionDialog';
import type { AskQuestion } from '../ui/AskUserBubble';

interface AppChatProps {
  activeMemoId: string | null;
  memoMessages: Message[];
  memoStreaming: boolean;
  memoStreamContent: string;
  handleMemoSend: (text: string) => void;
  messages: Message[];
  activeCharacter: Character | null;
  isThinking: boolean;
  agentStatus: string | null;
  permRequest: PermissionRequestData | null;
  setPermRequest: (req: PermissionRequestData | null) => void;
  askUserRequest: { id: string; questions: AskQuestion[] } | null;
  setAskUserRequest: (req: { id: string; questions: AskQuestion[] } | null) => void;
  error: string | null;
  handleRetry: () => void;
  onSend: (text: string) => void;
  isListening: boolean;
  sttError: string | null;
  handleSttToggle: () => void;
  handleStop: () => void;
  quotedPost: Post | null;
  setQuotedPost: (post: Post | null) => void;
  model: string;
  handleSaveMemo: (title: string, msgs: Message[]) => Promise<string | null>;
  handleOpenMemo: (id: string) => void;
}

export function AppChat(props: AppChatProps) {
  const {
    activeMemoId, memoMessages, memoStreaming, memoStreamContent, handleMemoSend,
    messages, activeCharacter, isThinking, agentStatus, permRequest, setPermRequest,
    askUserRequest, setAskUserRequest, error, handleRetry, onSend, isListening,
    sttError, handleSttToggle, handleStop, quotedPost, setQuotedPost, model,
    handleSaveMemo, handleOpenMemo
  } = props;

  const { explainText, showExplain, setShowExplain, floatBtn, handleExplainClick } = useExplainPopup();
  const { isOpen: live2DOpen, toggle: toggleLive2D, isMobileDevice } = useLive2D();
  const { models: live2dModels, activeModel: activeLive2DModel, setActive: setLive2DActive } = useLive2DModels();

  const handleMeetingToggle = useCallback(() => {
    const modelId = activeCharacter?.live2dModelId;
    // Prefer character's assigned model, fall back to the globally active model
    const model = modelId
      ? live2dModels.find((m) => m.id === modelId)
      : activeLive2DModel;
    if (model) setLive2DActive(model.id);
    toggleLive2D(model?.modelUrl);
  }, [activeCharacter?.live2dModelId, live2dModels, activeLive2DModel, setLive2DActive, toggleLive2D]);
  const inputBarRef = useRef<InputBarHandle>(null);
  const isDraggingFile = useFileDragDrop(inputBarRef);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    scrollRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isThinking, agentStatus]);

  const messageList = useMemo(() => {
    const lastUserMsgIdx = messages.reduce(
      (acc, m, i) => (m.role === 'user' ? i : acc),
      -1,
    );
    return messages.map((msg, idx) => (
      <ChatMessage
        key={idx}
        message={msg}
        isLastMessage={idx === messages.length - 1}
        isThinking={isThinking}
        onRetry={idx === lastUserMsgIdx && !isThinking ? handleRetry : undefined}
      />
    ));
  }, [messages, isThinking, handleRetry]);

  return (
    <>
      {isDraggingFile && (
        <div className="drag-file-overlay">
          <div className="drag-file-overlay-content">
            <File size={48} strokeWidth={1.5} />
            <span className="drag-file-overlay-text">Insert file</span>
          </div>
        </div>
      )}

      {floatBtn && (
        <button
          className="explain-float-btn"
          style={{ left: floatBtn.x, top: floatBtn.y }}
          onMouseDown={(e) => { e.preventDefault(); handleExplainClick(); }}
        >
          Explain more
        </button>
      )}

      {showExplain && (
        <ExplainPopup
          selectedText={explainText}
          model={model}
          contextMessages={messages}
          onClose={() => setShowExplain(false)}
          onSaveMemo={handleSaveMemo}
          onOpenMemo={handleOpenMemo}
        />
      )}

      {activeMemoId ? (
        <MemoChatView
          key={activeMemoId}
          messages={memoMessages}
          streaming={memoStreaming}
          streamContent={memoStreamContent}
          onSend={handleMemoSend}
        />
      ) : (
        <>
          <div className="app-content custom-scrollbar">
            {messages.length === 0 ? (
              activeCharacter ? (
                <div className="app-friend-empty">Start to chat with your new friend</div>
              ) : (
                <WelcomeScreen onSend={onSend} />
              )
            ) : (
              <div className="app-messages">
                {messageList}

                {agentStatus && (
                  <div className="app-agent-status">
                    <span className="app-agent-dot" />
                    {agentStatus}
                  </div>
                )}

                {permRequest && (
                  <PermissionRequest
                    request={permRequest}
                    onDone={() => setPermRequest(null)}
                  />
                )}

                {askUserRequest && (
                  <div className="chat-message">
                    <div className="chat-avatar">
                      <Sparkles size={24} />
                    </div>
                    <AskUserBubble
                      id={askUserRequest.id}
                      questions={askUserRequest.questions}
                      onDone={() => setAskUserRequest(null)}
                    />
                  </div>
                )}

                {error && (
                  <div className="app-error">{error}</div>
                )}

                <div ref={scrollRef} />
              </div>
            )}
          </div>

          <InputBar
            ref={inputBarRef}
            isThinking={isThinking}
            isListening={isListening}
            sttError={sttError}
            onSend={onSend}
            onSttToggle={handleSttToggle}
            onStop={handleStop}
            quotedPost={quotedPost}
            onClearQuote={() => setQuotedPost(null)}
            onLive2DToggle={activeCharacter?.live2dModelId ? handleMeetingToggle : undefined}
            live2DOpen={live2DOpen}
          />
        </>
      )}

      {/* Mobile: full-screen overlay (desktop window is managed by useLive2D hook) */}
      {isMobileDevice && live2DOpen && <Live2DMobileView onClose={toggleLive2D} />}
    </>
  );
}
