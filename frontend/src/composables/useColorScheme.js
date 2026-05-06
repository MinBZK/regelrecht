/**
 * Color scheme picker — automatic (follows OS), light, or dark.
 *
 * Thin facade over `useUserSettings`: persistence is server-backed when the
 * user is authenticated and falls back to localStorage on 401/network errors,
 * so the editor still respects the picker for anonymous users. The
 * `data-scheme` attribute on <html> (which the design system keys its dark
 * tokens off) is applied centrally inside `useUserSettings`.
 */
import { useUserSettings } from './useUserSettings.js';

export function useColorScheme() {
  const { theme, setTheme } = useUserSettings();
  return {
    colorScheme: theme,
    setColorScheme: setTheme,
  };
}
