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
