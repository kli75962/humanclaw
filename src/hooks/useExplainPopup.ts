import { useState, useEffect, useCallback } from 'react';

export function useExplainPopup() {
  const [explainText, setExplainText] = useState('');
  const [showExplain, setShowExplain] = useState(false);
  const [floatBtn, setFloatBtn] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    function onMouseUp() {
      const sel = window.getSelection();
      if (!sel || sel.isCollapsed || !sel.toString().trim()) {
        setFloatBtn(null);
        return;
      }
      const range = sel.getRangeAt(0);
      const messagesEl = document.querySelector('.app-messages');
      if (!messagesEl?.contains(range.commonAncestorContainer)) {
        setFloatBtn(null);
        return;
      }
      const rect = range.getBoundingClientRect();
      setFloatBtn({ x: rect.left + rect.width / 2, y: rect.bottom + 8 });
    }
    document.addEventListener('mouseup', onMouseUp);
    return () => document.removeEventListener('mouseup', onMouseUp);
  }, []);

  const handleExplainClick = useCallback(() => {
    const sel = window.getSelection();
    const text = sel?.toString().trim() ?? '';
    if (!text) return;
    setExplainText(text);
    setShowExplain(true);
    setFloatBtn(null);
    sel?.removeAllRanges();
  }, []);

  return {
    explainText,
    showExplain,
    setShowExplain,
    floatBtn,
    handleExplainClick,
  };
}
