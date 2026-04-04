/**
 * PC control tool definitions.
 *
 * These mirror the JSON schemas in src-tauri/src/tools/regtools/
 * and define the three-layer strategy from OpenClaw:
 *
 *   Layer 1 — system_run     (direct command, primary)
 *   Layer 2 — pc_ui_elements + pc_activate / pc_set_text  (structured UI)
 *   Layer 3 — pc_screenshot  (visual verification only)
 */

import type { ToolDef } from './types';

export const systemRun: ToolDef = {
  name: 'system_run',
  description:
    'Execute a system command directly on the PC without UI interaction. ' +
    'Use this as the PRIMARY method — faster, reliable, and accessibility-friendly. ' +
    'Prefer over screenshot loops for any task expressible as a shell command.',
  parameters: {
    type: 'object',
    properties: {
      command: {
        type: 'string',
        description: "Executable to run (e.g. 'bash', 'powershell', 'xdg-open').",
      },
      args: {
        type: 'array',
        items: { type: 'string' },
        description: 'Arguments as separate strings.',
      },
      timeout_secs: {
        type: 'number',
        description: 'Max seconds before killing the process (default: 30).',
        default: 30,
      },
    },
    required: ['command'],
  },
};

export const pcUiElements: ToolDef = {
  name: 'pc_ui_elements',
  description:
    'List all interactive UI elements currently on screen (buttons, inputs, links, etc.) ' +
    'using the OS accessibility API (AT-SPI2 / UI Automation). ' +
    'Use before pc_activate or pc_set_text to find exact element names.',
  parameters: {
    type: 'object',
    properties: {
      window_title: {
        type: 'string',
        description: 'Limit results to windows whose title contains this string.',
      },
    },
  },
};

export const pcActivate: ToolDef = {
  name: 'pc_activate',
  description:
    'Click or invoke a UI element by its accessible name using the OS accessibility API. ' +
    'No mouse simulation — works even if the window is not in focus. ' +
    'Use pc_ui_elements first to find the exact element name.',
  parameters: {
    type: 'object',
    properties: {
      name: {
        type: 'string',
        description: "Element's name as returned by pc_ui_elements (case-insensitive substring match).",
      },
      window_title: {
        type: 'string',
        description: 'Scope to a specific window. Required when multiple windows have the same element name.',
      },
    },
    required: ['name'],
  },
};

export const pcSetText: ToolDef = {
  name: 'pc_set_text',
  description:
    'Type text into a UI input field by its accessible name using the OS accessibility API. ' +
    'Use pc_ui_elements first to confirm the input field name.',
  parameters: {
    type: 'object',
    properties: {
      name: {
        type: 'string',
        description: "Input field's name as returned by pc_ui_elements.",
      },
      text: {
        type: 'string',
        description: 'Text to type into the field.',
      },
      window_title: {
        type: 'string',
        description: 'Scope to a specific window.',
      },
    },
    required: ['name', 'text'],
  },
};

export const pcOpenUrl: ToolDef = {
  name: 'pc_open_url',
  description: 'Open a URL in the default browser. Faster than navigating through the browser UI.',
  parameters: {
    type: 'object',
    properties: {
      url: {
        type: 'string',
        description: 'The http/https URL to open.',
      },
    },
    required: ['url'],
  },
};

export const pcScreenshot: ToolDef = {
  name: 'pc_screenshot',
  description:
    'Capture a screenshot for VISUAL VERIFICATION ONLY. ' +
    'Do not use as the primary control method — use system_run or pc_ui_elements instead. ' +
    'Reserve for: confirming a task completed, reading inaccessible text, or diagnosing unexpected state.',
  parameters: {
    type: 'object',
    properties: {
      display: {
        type: 'number',
        description: 'Display index to capture (default: 0 = primary monitor).',
        default: 0,
      },
    },
  },
};

export const pcGetPlatform: ToolDef = {
  name: 'pc_get_platform',
  description: 'Return the OS and architecture (e.g. linux/x86_64). Call once at session start to adapt commands.',
  parameters: {
    type: 'object',
    properties: {},
  },
};

/** All PC control tools in priority order. */
export const PC_TOOLS: ToolDef[] = [
  systemRun,       // Layer 1 — primary
  pcUiElements,    // Layer 2 — structured UI
  pcActivate,
  pcSetText,
  pcOpenUrl,
  pcGetPlatform,
  pcScreenshot,    // Layer 3 — verify only
];
