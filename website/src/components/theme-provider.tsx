import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "clipsy-theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

type ThemeContextValue = {
  theme: Theme;
  setTheme: (theme: Theme) => void;
};

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

function readStoredTheme(): Theme {
  try {
    const value = window.localStorage.getItem(STORAGE_KEY);
    if (value === "light" || value === "dark" || value === "system") {
      return value;
    }
  } catch {
    // Storage blocked (private mode, disabled site data): use the default.
  }
  return "system";
}

function prefersDark(): boolean {
  return typeof window.matchMedia === "function"
    ? window.matchMedia(DARK_QUERY).matches
    : false;
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(readStoredTheme);
  const [systemDark, setSystemDark] = useState<boolean>(prefersDark);

  // Follow the OS setting live while the theme is "system".
  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const query = window.matchMedia(DARK_QUERY);
    const onChange = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);

  const resolvedDark = theme === "dark" || (theme === "system" && systemDark);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedDark);
    root.style.colorScheme = resolvedDark ? "dark" : "light";
  }, [resolvedDark]);

  const setTheme = useCallback((next: Theme) => {
    // Cross-fade colours for a moment (CSS skips this under reduced motion).
    const root = document.documentElement;
    root.classList.add("theme-transition");
    window.setTimeout(() => root.classList.remove("theme-transition"), 260);
    setThemeState(next);
    try {
      window.localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Not persisted; the choice still applies for this visit.
    }
  }, []);

  const value = useMemo(() => ({ theme, setTheme }), [theme, setTheme]);

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useTheme must be used inside <ThemeProvider>");
  }
  return context;
}
