import type { ReactNode } from 'react';

/** Metadata for a saved chat session. */
export interface ChatMeta {
  id: string;
  title: string;
  createdAt: string;
}

export interface ChatMessageProps {
  message: Message;
  isLastMessage: boolean;
  isThinking: boolean;
  onRetry?: () => void;
}

export interface InputBarProps {
  value: string;
  isThinking: boolean;
  isListening: boolean;
  sttError: string | null;
  onChange: (value: string) => void;
  onSend: (text: string) => void;
  onSttToggle: () => void;
  onStop: () => void;
}

export interface ModalProps {
  title: string;
  onClose: () => void;
  children: ReactNode;
}

export interface SettingsScreenProps {
  model: string;
  availableModels: string[];
  onModelChange: (model: string) => void;
  onOllamaEndpointChanged: () => void;
  onBack: () => void;
}

export interface SideMenuProps {
  open: boolean;
  onClose: () => void;
  onNewChat: () => void;
  chats: ChatMeta[];
  activeChatId: string | null;
  onSelectChat: (id: string) => void;
  onDeleteChat: (id: string) => void;
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
  onMenuOpen: () => void;
  onSettingsOpen: () => void;
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
}
