import { useState, useEffect, useCallback, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import '../../style/AccessibilityDialog.css';

const DONT_SHOW_KEY = 'phoneclaw_accessibility_dont_show';
const IS_ANDROID = navigator.userAgent.includes('Android');

export const AccessibilityDialog = memo(function AccessibilityDialog() {
  const [visible, setVisible] = useState(false);
  const [dontShow, setDontShow] = useState(false);

  const check = useCallback(() => {
    if (!IS_ANDROID) return;
    if (localStorage.getItem(DONT_SHOW_KEY) === 'true') return;
    invoke<boolean>('check_accessibility_enabled')
      .then((enabled) => {
        if (enabled) {
          localStorage.removeItem(DONT_SHOW_KEY);
          setVisible(false);
        } else {
          setVisible(true);
        }
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    check();
    window.addEventListener('focus', check);
    window.addEventListener('pageshow', check);
    document.addEventListener('visibilitychange', check);

    const interval = window.setInterval(check, 3000);

    return () => {
      window.removeEventListener('focus', check);
      window.removeEventListener('pageshow', check);
      document.removeEventListener('visibilitychange', check);
      window.clearInterval(interval);
    };
  }, [check]);

  if (!visible) return null;

  const dismiss = (openSettings: boolean) => {
    if (dontShow) localStorage.setItem(DONT_SHOW_KEY, 'true');
    if (openSettings) invoke('open_accessibility_settings').catch(() => {});
    setVisible(false);
  };

  return (
    <div className="a11y-backdrop">
      <div className="a11y-dialog">
        <div className="a11y-dialog-body">
          <h2 className="a11y-dialog-title">Accessibility access needed</h2>
          <p className="a11y-dialog-text">
            PhoneClaw needs the Accessibility Service to control your phone on your behalf.
            Enable it in{' '}
            <span className="a11y-dialog-highlight">
              Settings → Accessibility → PhoneClaw
            </span>
            .
          </p>

          <div onClick={() => setDontShow((v) => !v)} className="a11y-checkbox-row">
            <div
              style={{
                width: 20,
                height: 20,
                borderRadius: 4,
                border: `2px solid ${dontShow ? '#7B5EA7' : '#4C4C4C'}`,
                backgroundColor: dontShow ? '#7B5EA7' : 'transparent',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                flexShrink: 0,
                transition: 'background-color 0.15s, border-color 0.15s',
              }}
            >
              {dontShow && (
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                  <path d="M2 6l3 3 5-5" stroke="white" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
            </div>
            <span className="a11y-checkbox-label">Don't show again</span>
          </div>
        </div>

        <div className="a11y-dialog-actions">
          <button onClick={() => dismiss(false)} className="a11y-dialog-btn">
            Cancel
          </button>
          <div className="a11y-dialog-divider" />
          <button onClick={() => dismiss(true)} className="a11y-dialog-btn a11y-dialog-btn--confirm">
            Open Settings
          </button>
        </div>
      </div>
    </div>
  );
});
