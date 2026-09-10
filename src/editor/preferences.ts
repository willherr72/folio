import { useCallback, useEffect, useState } from "react";

export interface Preferences {
  theme: "system" | "light" | "dark";
  viewMode: "continuous" | "single";
  defaultZoom: number;
  penColor: string;
  penWidth: number;
}

export const PREFERENCES_KEY = "folio.preferences.v1";
export const DEFAULT_PREFERENCES: Readonly<Preferences> = {
  theme: "system", viewMode: "continuous", defaultZoom: 90, penColor: "#2D2A26", penWidth: 2,
};

export function validatePreferences(value: unknown): Preferences {
  const saved = value && typeof value === "object" ? value as Record<string, unknown> : {};
  return {
    theme: saved.theme === "light" || saved.theme === "dark" ? saved.theme : "system",
    viewMode: saved.viewMode === "single" ? "single" : "continuous",
    defaultZoom: typeof saved.defaultZoom === "number" && Number.isFinite(saved.defaultZoom) && saved.defaultZoom >= 50 && saved.defaultZoom <= 200 ? saved.defaultZoom : 90,
    penColor: typeof saved.penColor === "string" && /^#[0-9a-f]{6}$/i.test(saved.penColor) ? saved.penColor : "#2D2A26",
    penWidth: typeof saved.penWidth === "number" && Number.isFinite(saved.penWidth) && saved.penWidth >= 0.5 && saved.penWidth <= 20 ? saved.penWidth : 2,
  };
}

function readPreferences(): Preferences {
  try { return validatePreferences(JSON.parse(localStorage.getItem(PREFERENCES_KEY) ?? "null")); }
  catch { return { ...DEFAULT_PREFERENCES }; }
}

export function usePreferences() {
  const [preferences, updatePreferences] = useState<Preferences>(readPreferences);
  const setPreferences = useCallback((next: Preferences) => updatePreferences(validatePreferences(next)), []);
  const resetPreferences = useCallback(() => updatePreferences({ ...DEFAULT_PREFERENCES }), []);

  useEffect(() => {
    try { localStorage.setItem(PREFERENCES_KEY, JSON.stringify(preferences)); }
    catch { /* Preferences still work for this session if storage is unavailable. */ }
  }, [preferences]);

  useEffect(() => {
    const media = window.matchMedia?.("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      document.documentElement.dataset.theme = preferences.theme === "system" ? (media?.matches ? "dark" : "light") : preferences.theme;
    };
    applyTheme();
    if (preferences.theme !== "system") return;
    media?.addEventListener("change", applyTheme);
    return () => media?.removeEventListener("change", applyTheme);
  }, [preferences.theme]);

  return { preferences, setPreferences, resetPreferences };
}
