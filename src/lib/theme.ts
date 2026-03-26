const DARK_MEDIA_QUERY = "(prefers-color-scheme: dark)";
const DEFAULT_UI_SCALE = 1;
const MIN_UI_SCALE = 0.6;
const MAX_UI_SCALE = 1.5;

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

export function normalizeUiScale(value: number) {
  if (!Number.isFinite(value)) {
    return DEFAULT_UI_SCALE;
  }

  return Math.min(MAX_UI_SCALE, Math.max(MIN_UI_SCALE, value));
}

export function applyUiScale(uiScale: number) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.style.setProperty("--ui-scale", String(normalizeUiScale(uiScale)));
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
