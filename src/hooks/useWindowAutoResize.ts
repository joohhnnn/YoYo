import { useEffect, useLayoutEffect, useRef } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalSize } from "@tauri-apps/api/dpi";

const WIDTH = 340;
const MIN_HEIGHT = 80;
const MAX_HEIGHT = 560;

/**
 * Auto-resize the Tauri window to match bubble content height.
 *
 * Key insight: measure the UNCONSTRAINED inner content wrapper (contentRef),
 * not the flex-constrained container. This gives true natural height even
 * when the scroll area is compressed by flex layout.
 *
 * Container uses `max-h-screen` (100vh) which auto-adjusts when the window
 * resizes, creating a self-correcting feedback loop.
 *
 * Three sync mechanisms:
 * 1. useLayoutEffect — catches React state changes before paint
 * 2. ResizeObserver on contentRef — detects content size changes
 * 3. MutationObserver on container — catches DOM structure changes
 */
export function useWindowAutoResize() {
  const bubbleRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const lastHeight = useRef(0);

  const sync = () => {
    const bubble = bubbleRef.current;
    const content = contentRef.current;
    if (!bubble || !content) return;

    // Measure natural height from unconstrained inner wrapper + fixed regions
    const header = bubble.firstElementChild as HTMLElement | null;
    const footer = bubble.lastElementChild as HTMLElement | null;
    const h = Math.ceil(
      (header?.offsetHeight ?? 0) +
      content.offsetHeight +
      (footer?.offsetHeight ?? 0)
    );
    const clamped = Math.min(Math.max(h, MIN_HEIGHT), MAX_HEIGHT);
    if (clamped === lastHeight.current) return;
    lastHeight.current = clamped;
    getCurrentWebviewWindow()
      .setSize(new LogicalSize(WIDTH, clamped))
      .catch(() => {});
  };

  // Sync on every React render (before paint)
  useLayoutEffect(() => {
    sync();
  });

  // Observers for async / non-React changes
  useEffect(() => {
    const content = contentRef.current;
    const bubble = bubbleRef.current;

    const ro = new ResizeObserver(() => sync());
    if (content) ro.observe(content);

    const mo = new MutationObserver(() => sync());
    if (bubble)
      mo.observe(bubble, { childList: true, subtree: true, characterData: true });

    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }, []);

  return { bubbleRef, contentRef };
}
