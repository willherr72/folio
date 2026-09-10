import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { usePreferences, PREFERENCES_KEY } from "../src/editor/preferences";

beforeEach(() => localStorage.clear());
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe("editor preferences", () => {
  it("persists edits and resets stored defaults across mounts", () => {
    const first = renderHook(usePreferences);
    act(() => first.result.current.setPreferences({ theme: "dark", viewMode: "single", defaultZoom: 125, penColor: "#123456", penWidth: 5 }));
    first.unmount();
    const next = renderHook(usePreferences);
    expect(next.result.current.preferences).toEqual({ theme: "dark", viewMode: "single", defaultZoom: 125, penColor: "#123456", penWidth: 5 });
    act(() => next.result.current.resetPreferences());
    next.unmount();
    expect(renderHook(usePreferences).result.current.preferences).toEqual({ theme: "system", viewMode: "continuous", defaultZoom: 90, penColor: "#2D2A26", penWidth: 2 });
  });

  it("recovers invalid fields while preserving valid stored preferences", () => {
    localStorage.setItem(PREFERENCES_KEY, JSON.stringify({ theme: "wrong", viewMode: "single", defaultZoom: -1, penColor: "red", penWidth: 0 }));
    expect(renderHook(usePreferences).result.current.preferences).toEqual({ theme: "system", viewMode: "single", defaultZoom: 90, penColor: "#2D2A26", penWidth: 2 });
  });

  it("survives corrupt storage and blocked writes", () => {
    localStorage.setItem(PREFERENCES_KEY, "{broken");
    const prefs = renderHook(usePreferences);
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("Storage unavailable"); });
    act(() => prefs.result.current.setPreferences({ ...prefs.result.current.preferences, theme: "dark" }));
    expect(prefs.result.current.preferences.theme).toBe("dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("tracks the operating system theme until an explicit theme is selected", () => {
    const media = new EventTarget() as EventTarget & { matches: boolean };
    media.matches = true;
    vi.stubGlobal("matchMedia", () => media);
    const prefs = renderHook(usePreferences);
    expect(document.documentElement.dataset.theme).toBe("dark");
    act(() => { media.matches = false; media.dispatchEvent(new Event("change")); });
    expect(document.documentElement.dataset.theme).toBe("light");
    act(() => prefs.result.current.setPreferences({ ...prefs.result.current.preferences, theme: "dark" }));
    act(() => media.dispatchEvent(new Event("change")));
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});
