import { useState, useEffect } from 'react';

export type Theme = 'dark' | 'light' | 'gruvbox';

const THEME_KEY = 'phoneclaw_theme';

export function useTheme(): [Theme, (t: Theme) => void] {
  const [theme, setThemeState] = useState<Theme>(
    () => (localStorage.getItem(THEME_KEY) as Theme) ?? 'dark',
  );

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  const setTheme = (t: Theme) => {
    localStorage.setItem(THEME_KEY, t);
    setThemeState(t);
  };

  return [theme, setTheme];
}
