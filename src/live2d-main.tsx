import React from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { Live2DWindowApp } from './components/live2d/Live2DWindowApp';
import './style/Live2DWindow.css';

// Patch XHR so pixi-live2d-display can load local files via Tauri IPC.
// In dev mode the live2d window origin is http://localhost:1420, so XHR
// cannot reach asset:// URLs due to cross-scheme restrictions in WebKit.
// We intercept any URL that looks like a local filesystem path and serve
// it through the existing `read_file_as_base64` Rust command instead.
(function patchXHRForLocalFiles() {
  const OrigXHR = window.XMLHttpRequest;

  class PatchedXHR extends OrigXHR {
    private _localPath: string | null = null;
    private _responseType2: XMLHttpRequestResponseType = '';
    private _readyStateCallbacks: (() => void)[] = [];

    open(method: string, url: string, ...rest: Parameters<XMLHttpRequest['open']> extends [string, string, ...infer R] ? R : never[]) {
      // Detect asset:// or absolute filesystem paths
      let localPath: string | null = null;
      if (url.startsWith('asset://localhost/')) {
        localPath = '/' + decodeURIComponent(url.slice('asset://localhost/'.length));
      } else if (url.startsWith('asset://localhost')) {
        localPath = decodeURIComponent(url.slice('asset://localhost'.length));
      }

      if (localPath) {
        this._localPath = localPath;
        // Don't call super.open — we'll fake the whole lifecycle
        Object.defineProperty(this, 'readyState', { get: () => 0, configurable: true });
      } else {
        this._localPath = null;
        (super.open as Function)(method, url, ...rest);
      }
    }

    get responseType() { return this._responseType2; }
    set responseType(v: XMLHttpRequestResponseType) {
      this._responseType2 = v;
      if (!this._localPath) super.responseType = v;
    }

    send(_body?: Document | XMLHttpRequestBodyInit | null) {
      if (!this._localPath) { super.send(_body); return; }

      const path = this._localPath;
      const self = this as unknown as XMLHttpRequest;

      invoke<string>('read_file_as_base64', { path })
        .then((b64) => {
          // Determine mime type from extension
          const ext = path.split('.').pop()?.toLowerCase() ?? '';
          const mime: Record<string, string> = {
            json: 'application/json',
            moc3: 'application/octet-stream',
            png:  'image/png',
            jpg:  'image/jpeg',
            jpeg: 'image/jpeg',
            motion3: 'application/octet-stream',
          };
          const mimeType = mime[ext] ?? 'application/octet-stream';
          const binary = atob(b64);
          const bytes = new Uint8Array(binary.length);
          for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
          const blob = new Blob([bytes], { type: mimeType });

          if (this._responseType2 === 'blob') {
            Object.defineProperty(self, 'response', { value: blob, configurable: true });
          } else if (this._responseType2 === 'arraybuffer') {
            Object.defineProperty(self, 'response', { value: bytes.buffer, configurable: true });
          } else if (this._responseType2 === 'json') {
            // Native XHR with responseType='json' returns a parsed object, not a string.
            // Returning a string caused "Attempted to assign to readonly property" in WebKit
            // when pixi-live2d-display did `data.url = ...` on the string result.
            const text = new TextDecoder().decode(bytes);
            let parsed: unknown = null;
            try { parsed = JSON.parse(text); } catch { parsed = null; }
            Object.defineProperty(self, 'response', { value: parsed, configurable: true });
          } else {
            // text / default
            const text = new TextDecoder().decode(bytes);
            Object.defineProperty(self, 'response', { value: text, configurable: true });
            Object.defineProperty(self, 'responseText', { value: text, configurable: true });
          }

          Object.defineProperty(self, 'status', { value: 200, configurable: true });
          Object.defineProperty(self, 'readyState', { value: 4, configurable: true });

          if (typeof self.onload === 'function') self.onload(new ProgressEvent('load'));
          if (typeof self.onreadystatechange === 'function') self.onreadystatechange(new Event('readystatechange'));
          self.dispatchEvent(new ProgressEvent('load'));
        })
        .catch((err) => {
          Object.defineProperty(self, 'status', { value: 0, configurable: true });
          if (typeof self.onerror === 'function') self.onerror(new ProgressEvent('error'));
          console.error('[Live2D XHR patch] failed to load', path, err);
        });
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  (window as any).XMLHttpRequest = PatchedXHR;
})();

// Patch <img> src so textures loaded by pixi-live2d-display via asset:// URLs
// are served through Tauri IPC instead of the asset protocol (whose scope
// cannot easily be configured to allow arbitrary absolute filesystem paths).
(function patchImageForLocalFiles() {
  const desc = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, 'src');
  if (!desc?.set) return;
  const nativeSet = desc.set;
  Object.defineProperty(HTMLImageElement.prototype, 'src', {
    get: desc.get,
    set(url: string) {
      if (url.startsWith('asset://localhost/')) {
        const path = '/' + decodeURIComponent(url.slice('asset://localhost/'.length));
        const ext = path.split('.').pop()?.toLowerCase() ?? '';
        const mimes: Record<string, string> = {
          png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', webp: 'image/webp',
        };
        const mime = mimes[ext] ?? 'image/png';
        const self = this;
        invoke<string>('read_file_as_base64', { path })
          .then((b64) => { nativeSet.call(self, `data:${mime};base64,${b64}`); })
          .catch(() => { nativeSet.call(self, url); });
      } else {
        nativeSet.call(this, url);
      }
    },
    configurable: true,
  });
})();

// StrictMode intentionally omitted — it double-invokes effects in dev,
// which resets state and causes a second PIXI canvas to be created.
ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <Live2DWindowApp />,
);
