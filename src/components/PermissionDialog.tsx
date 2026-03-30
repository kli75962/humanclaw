import { invoke } from '@tauri-apps/api/core';
import { ShieldAlert } from 'lucide-react';
import '../style/PermissionDialog.css';

export interface PermissionRequest {
  id: string;
  tool: string;
  permission: string;
  args?: Record<string, unknown>;
}

const PERMISSION_LABELS: Record<string, string> = {
  mouse_control:   'Mouse Control',
  keyboard_input:  'Keyboard Input',
  take_screenshot: 'Screenshot',
  launch_app:      'Open URL / App',
};

function str(v: unknown): string {
  return typeof v === 'string' ? v : JSON.stringify(v) ?? '';
}

function buildDescription(tool: string, args: Record<string, unknown> = {}): { summary: string; detail?: string } {
  switch (tool) {
    case 'pc_mouse_click': {
      const btn = args.button ?? 'left';
      const dbl = args.double ? ' (double-click)' : '';
      const pos = args.x != null ? ` at (${args.x}, ${args.y})` : '';
      return { summary: `Click ${btn} mouse button${dbl}${pos}` };
    }
    case 'pc_type_text':
      return { summary: 'Type text into the focused window', detail: str(args.text) };
    case 'pc_screenshot': {
      const d = args.display ?? 0;
      return { summary: `Capture screenshot of display ${d}` };
    }
    case 'pc_open_url':
      return { summary: 'Open URL or file', detail: str(args.url) };
    default:
      return { summary: `Use tool: ${tool}` };
  }
}

export function PermissionRequest({
  request,
  onDone,
}: {
  request: PermissionRequest;
  onDone: () => void;
}) {
  async function respond(allowed: boolean) {
    await invoke('respond_pc_permission', { id: request.id, allowed });
    onDone();
  }

  const permLabel = PERMISSION_LABELS[request.permission] ?? request.permission;
  const { summary, detail } = buildDescription(request.tool, request.args);

  return (
    <div className="perm-request">
      <div className="perm-request-header">
        <ShieldAlert size={14} className="perm-request-icon" />
        <span className="perm-request-tag">Permission required · {permLabel}</span>
      </div>
      <p className="perm-request-action">{summary}</p>
      {detail && <pre className="perm-request-detail">{detail}</pre>}
      <div className="perm-request-btns">
        <button className="perm-request-btn perm-request-btn--deny"  onClick={() => respond(false)}>Deny</button>
        <button className="perm-request-btn perm-request-btn--allow" onClick={() => respond(true)}>Allow</button>
      </div>
    </div>
  );
}
