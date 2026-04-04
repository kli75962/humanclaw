/**
 * Unified tool framework — mirrors the Rust ToolResult and defines
 * JSON Schema for all tools exposed to the LLM.
 */

// ── Result types ────────────────────────────────────────────────────────────

export type ToolErrorCode =
  | 'PERMISSION_DENIED'
  | 'NOT_AVAILABLE'
  | 'NOT_FOUND'
  | 'INVALID_ARGS'
  | 'EXECUTION_FAILED'
  | 'DEVICE_NOT_FOUND'
  | 'DEVICE_UNREACHABLE'
  | 'DEVICE_ERROR';

/** Mirrors Rust ToolResult — every tool returns this shape. */
export interface ToolResult {
  tool_name: string;
  success: boolean;
  output: string;
  error_code?: ToolErrorCode;
}

// ── Schema types (JSON Schema subset) ──────────────────────────────────────

type JsonSchemaType = 'string' | 'number' | 'boolean' | 'array' | 'object';

interface JsonSchemaProperty {
  type: JsonSchemaType;
  description?: string;
  default?: unknown;
  minimum?: number;
  maximum?: number;
  items?: { type: JsonSchemaType };
  enum?: string[];
}

interface JsonSchemaObject {
  type: 'object';
  properties: Record<string, JsonSchemaProperty>;
  required?: string[];
}

/** Single tool definition — mirrors OpenClaw's AnyAgentTool shape. */
export interface ToolDef {
  /** Unique name used by the LLM in tool_call requests. */
  name: string;
  /** Human-readable description sent to the LLM. */
  description: string;
  /** JSON Schema for the tool's parameters. */
  parameters: JsonSchemaObject;
}

/** Full OpenAI-compatible function schema used in the Ollama API request. */
export interface ToolSchema {
  type: 'function';
  function: {
    name: string;
    description: string;
    parameters: JsonSchemaObject;
  };
}

export function toToolSchema(def: ToolDef): ToolSchema {
  return {
    type: 'function',
    function: {
      name: def.name,
      description: def.description,
      parameters: def.parameters,
    },
  };
}
