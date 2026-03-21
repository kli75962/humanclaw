import { useState, useEffect, useCallback, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';

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

    // Android may not always fire focus reliably after returning from Settings.
    // Poll while mounted so the dialog disappears immediately once enabled.
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
    <div
      className="fixed inset-0 flex items-center justify-center bg-black/60 "
      style={{ zIndex: 80, padding: '0 32px'}}
    >
      <div className="w-full max-w-sm bg-[#1E1F20] rounded-2xl shadow-2xl overflow-hidden">
        <div style={{ padding: '28px 20px 20px' }}>
          <h2 className="text-base font-semibold text-[#E3E3E3] mb-2">
            Accessibility access needed
          </h2>
          <p className="text-sm text-[#A0A0A0] leading-relaxed">
            PhoneClaw needs the Accessibility Service to control your phone on your behalf.
            Enable it in{' '}
            <span className="text-[#E3E3E3]">
              Settings → Accessibility → PhoneClaw
            </span>
            .
          </p>

          <div
            onClick={() => setDontShow((v) => !v)}
            className="flex items-center gap-3 mt-5 cursor-pointer select-none"
          >
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
                  <path
                    d="M2 6l3 3 5-5"
                    stroke="white"
                    strokeWidth="1.8"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              )}
            </div>
            <span className="text-sm text-[#A0A0A0]">Don't show again</span>
          </div>
        </div>

        <div className="flex border-t border-[#2C2C2C]">
          <button
            onClick={() => dismiss(false)}
            className="flex-1 py-3.5 text-sm font-medium text-[#A0A0A0] hover:bg-[#2C2C2C] transition-colors"
          >
            Cancel
          </button>
          <div className="w-px bg-[#2C2C2C]" />
          <button
            onClick={() => dismiss(true)}
            className="flex-1 py-3.5 text-sm font-medium text-[#9B7BC4] hover:bg-[#2C2C2C] transition-colors"
          >
            Open Settings
          </button>
        </div>
      </div>
    </div>
  );
});
