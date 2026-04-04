/**
 * PC control tool definitions — OpenClaw style.
 *
 * Strategy:
 *   1. system_run     — direct command execution (primary)
 *   2. pc_open_url    — open URLs in default browser
 *   3. pc_screenshot  — visual verification only (never for control)
 *   4. pc_get_platform — detect OS once at session start
 */

import type { ToolDef } from './types';

export const systemRun: ToolDef = {
  name: 'system_run',
  description:
    'Execute a system command directly on the PC. ' +
    'This is the PRIMARY tool for all PC tasks — use it for opening files, ' +
    'running scripts, querying system state, and anything expressible as a command. ' +
    'Pass each argument separately in args, never combine them into one string.',
  parameters: {
    type: 'object',
    properties: {
      command: {
        type: 'string',
        description: "Executable to run (e.g. 'bash', 'powershell', 'xdg-open', 'notify-send').",
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

export const pcOpenUrl: ToolDef = {
  name: 'pc_open_url',
  description: 'Open a URL in the default browser.',
  parameters: {
    type: 'object',
    properties: {
      url: { type: 'string', description: 'The http/https URL to open.' },
    },
    required: ['url'],
  },
};

export const pcScreenshot: ToolDef = {
  name: 'pc_screenshot',
  description:
    'Capture a screenshot for VISUAL VERIFICATION ONLY. ' +
    'Do not use to decide what to do — use system_run to query state instead. ' +
    'Reserve for: confirming a task completed, reading inaccessible rendered content.',
  parameters: {
    type: 'object',
    properties: {
      display: {
        type: 'number',
        description: 'Display index (default: 0 = primary monitor).',
        default: 0,
      },
    },
  },
};

export const pcGetPlatform: ToolDef = {
  name: 'pc_get_platform',
  description: 'Return OS and architecture. Call once at session start to choose the right commands.',
  parameters: {
    type: 'object',
    properties: {},
  },
};

/** All PC control tools in priority order. */
export const PC_TOOLS: ToolDef[] = [
  systemRun,
  pcOpenUrl,
  pcGetPlatform,
  pcScreenshot,
];
