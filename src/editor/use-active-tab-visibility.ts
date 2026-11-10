import { useLayoutEffect, useRef } from "react";

/** Reveal selection/layout changes without taking focus or following manual scroll. */
export function useActiveTabVisibility(activeId: string | null, tabCount: number) {
  const stripRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const strip = stripRef.current;
    const tab = strip?.querySelector<HTMLElement>(".document-tab.active");
    if (!strip || !tab || !activeId) return;
    let alive = true;
    const reveal = () => {
      if (!alive || !strip.isConnected || !tab.isConnected || strip.clientWidth <= 0) return;
      const viewport = strip.getBoundingClientRect();
      const bounds = tab.getBoundingClientRect();
      const left = viewport.left + strip.clientLeft;
      const right = left + strip.clientWidth;
      const delta = bounds.width > strip.clientWidth || bounds.left < left
        ? bounds.left - left : bounds.right > right ? bounds.right - right : 0;
      if (Math.abs(delta) > 0.5) {
        strip.scrollLeft = Math.max(0, Math.min(strip.scrollWidth - strip.clientWidth, strip.scrollLeft + delta));
      }
    };
    reveal();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(reveal);
    observer?.observe(strip);
    observer?.observe(tab);
    window.addEventListener("resize", reveal);
    return () => { alive = false; observer?.disconnect(); window.removeEventListener("resize", reveal); };
  }, [activeId, tabCount]);
  return stripRef;
}
