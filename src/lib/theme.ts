const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";

function resolveTheme(theme: string) {
  if (theme === "dark" || theme === "light") {
    return theme;
  }

  if (typeof window !== "undefined" && window.matchMedia(DARK_MEDIA_QUERY).matches) {
    return "dark";
  }

  return "light";
}

export function applyTheme(theme: string) {
  if (typeof document === "undefined") {
    return;
  }

  const root = document.documentElement;
  root.dataset.themePreference = theme;
  root.dataset.theme = resolveTheme(theme);
}

export function watchSystemTheme(onChange: () => void) {
  if (typeof window === "undefined") {
    return () => {};
  }

  const mediaQuery = window.matchMedia(DARK_MEDIA_QUERY);
  const handler = () => onChange();

  if ("addEventListener" in mediaQuery) {
    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  }

  const legacyMediaQuery = mediaQuery as MediaQueryList & {
    addListener: (listener: (event: MediaQueryListEvent) => void) => void;
    removeListener: (listener: (event: MediaQueryListEvent) => void) => void;
  };

  legacyMediaQuery.addListener(handler);
  return () => legacyMediaQuery.removeListener(handler);
}
