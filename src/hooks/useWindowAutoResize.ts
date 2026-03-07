import { useEffect, useLayoutEffect, useRef } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalSize } from "@tauri-apps/api/dpi";

const WIDTH = 340;
const MIN_HEIGHT = 80;
const MAX_HEIGHT = 520;

/**
 * Auto-resize the Tauri window to match content height.
 *
 * Uses three complementary mechanisms:
 * 1. useLayoutEffect on every render — catches React state-driven changes before paint
 * 2. ResizeObserver — detects element size changes (e.g. CSS transitions, image loads)
 * 3. MutationObserver — catches DOM mutations that don't immediately change element size
 *
 * Measures `scrollHeight` which returns full content height even when the element
 * has overflow:hidden / max-height constraints.
 */
export function useWindowAutoResize() {
  const ref = useRef<HTMLDivElement>(null);
  const lastHeight = useRef(0);

  // Sync window size to content — called from multiple sources
  const sync = useRef(() => {
    const el = ref.current;
    if (!el) return;
    const h = Math.ceil(el.scrollHeight);
    const clamped = Math.min(Math.max(h, MIN_HEIGHT), MAX_HEIGHT);
    if (clamped === lastHeight.current) return;
    lastHeight.current = clamped;
    getCurrentWebviewWindow()
      .setSize(new LogicalSize(WIDTH, clamped))
      .catch(() => {});
  });

  // 1. Sync on every React render (before paint)
  useLayoutEffect(() => {
    sync.current();
  });

  // 2+3. ResizeObserver + MutationObserver for async / non-React changes
  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    const ro = new ResizeObserver(() => sync.current());
    ro.observe(el);

    const mo = new MutationObserver(() => sync.current());
    mo.observe(el, { childList: true, subtree: true, characterData: true });

    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }, []);

  return ref;
}
