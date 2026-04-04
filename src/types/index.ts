import type { ReactNode } from 'react';

/** Metadata for a saved chat session. */
export interface ChatMeta {
  id: string;
  title: string;
  createdAt: string;
}

/** A post created by a Chat Mode character. */
export interface Post {
  id: string;
  characterId: string;
  text: string;
  image?: string;      // base64 data URL, optional
  createdAt: string;   // ISO datetime
  likeCount: number;
}

/** A comment on a post. authorId is a characterId or "user". */
export interface PostComment {
  id: string;
  postId: string;
  authorId: string;
  text: string;
  createdAt: string;
}

/** Returned by react_to_user_post. action="dm" means text should be injected into chat; action="comment" means it was already saved. */
export interface ReactResult {
  characterId: string;
  action: 'dm' | 'comment';
  text: string;
  commentId?: string;
}

/** A Chat Mode character/friend. */
export interface Character {
  id: string;
  name: string;
  icon?: string;            // emoji, optional
  model: string;
  persona: string;
  background: string;
  createdAt: string;
  activeTime?: 'early' | 'night' | 'random';  // When character is most active
  birthday?: string;        // ISO date YYYY-MM-DD or 'random'
}

export interface ChatMessageProps {
  message: Message;
  isLastMessage: boolean;
  isThinking: boolean;
  onRetry?: () => void;
}

export interface InputBarProps {
  isThinking: boolean;
  isListening: boolean;
  sttError: string | null;
  onSend: (text: string) => void;
  onSttToggle: () => void;
  onStop: () => void;
  quotedPost?: Post | null;
  onClearQuote?: () => void;
}

/** Imperative handle exposed by InputBar for STT integration. */
export interface InputBarHandle {
  setInput: (text: string) => void;
  getInput: () => string;
  /** Attach a browser File object (from the file picker). */
  attachFile: (file: File) => void;
  /** Attach a file by its OS path string (from Tauri drag-drop). */
  attachFilePath: (path: string) => void;
}

export interface ModalProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
}


/** Memo list entry (no messages for perf). */
export interface MemoMeta {
  id: string;
  title: string;
  created_at: string;
}

export interface SideMenuProps {
  view: 'history' | 'settings' | 'connect' | 'memos';
  onSwitchView: (v: 'history' | 'settings' | 'connect' | 'memos') => void;
  activeMemoId: string | null;
  onSelectMemo: (id: string) => void;
  onNewChat: () => void;
  chats: ChatMeta[];
  activeChatId: string | null;
  onSelectChat: (id: string) => void;
  onDeleteChat: (id: string) => void;
  model: string;
  onModelChange: (m: string) => void;
  onOllamaEndpointChanged: () => void;
  isMobileOpen?: boolean;
  onCloseSide?: () => void;
  chatMode: boolean;
  onChatModeChange: (v: boolean) => void;
  characters: Character[];
  activeCharacterId: string | null;
  onSelectCharacter: (id: string) => void;
  onCreateCharacter: (data: Omit<Character, 'id' | 'createdAt'>) => void;
  onDeleteCharacter: (id: string) => void;
  igMode: boolean;
  onIgModeChange: (v: boolean) => void;
  onAddPersona?: (answers: WizardAnswers) => void;
}

export interface WizardAnswers {
  sex: string;
  personality: string;
  profession: string;
  personaName: string;
}

export interface WelcomeScreenProps {
  onSend: (text: string) => void;
}

export interface UseOllamaChatReturn {
  messages: Message[];
  isThinking: boolean;
  agentStatus: string | null;
  error: string | null;
  handleSend: (text: string) => Promise<void>;
  handleStop: () => void;
  handleRetry: () => Promise<void>;
}

export interface TopBarProps {
  model: string;
}

/** A single message in the Ollama chat conversation. */
export interface Message {
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
}

/** Payload emitted by the `ollama-stream` Tauri event for every token. */
export interface StreamPayload {
  content: string;
  done: boolean;
}

/** Emitted by `agent-status` when the LLM is executing a tool. */
export interface AgentStatusPayload {
  message: string;
}

export type PermissionState = 'allow_all' | 'ask_before_use' | 'not_allow';

export interface PcPermissions {
  take_screenshot: PermissionState;
  launch_app:      PermissionState;
  shell_execution: PermissionState;
}

/** Session config returned by the Rust `get_session` command. */
export interface SessionConfig {
  device: {
    device_id: string;
    device_type: 'android' | 'desktop';
    label: string;
  };
  hash_key: string;
  paired_devices: { device_id: string; address: string; label: string }[];
  bridge_port: number;
  ollama_host_override: string | null;
  ollama_port: number;
  persona: string;
  pc_permissions: PcPermissions;
}
