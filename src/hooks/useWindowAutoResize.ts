import { useEffect, useLayoutEffect, useRef } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import type { BubbleState } from "../types";

const DOT_SIZE = 48;
const EXPANDED_WIDTH = 340;
const MIN_HEIGHT = 80;
const MAX_HEIGHT = 400;
const MARGIN_RIGHT = 20;
const MARGIN_TOP = 40;

/**
 * Auto-resize the Tauri window based on bubble state and content.
 *
 * - Ambient: fixed 48×48 dot, positioned at screen top-right edge
 * - Active/Working/Done: 340×auto, content-driven height, right edge stays fixed
 *
 * Repositions on every resize so the right edge stays anchored.
 */
export function useWindowAutoResize(state: BubbleState) {
  const bubbleRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const lastWidth = useRef(0);
  const lastHeight = useRef(0);

  const sync = () => {
    const win = getCurrentWebviewWindow();

    if (state === "ambient") {
      if (lastWidth.current !== DOT_SIZE || lastHeight.current !== DOT_SIZE) {
        lastWidth.current = DOT_SIZE;
        lastHeight.current = DOT_SIZE;
        win.setSize(new LogicalSize(DOT_SIZE, DOT_SIZE)).catch(() => {});
        positionTopRight(win, DOT_SIZE);
      }
      return;
    }

    // Expanded states: measure content height
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

    if (clamped !== lastHeight.current || lastWidth.current !== EXPANDED_WIDTH) {
      lastHeight.current = clamped;
      lastWidth.current = EXPANDED_WIDTH;
      win.setSize(new LogicalSize(EXPANDED_WIDTH, clamped)).catch(() => {});
      positionTopRight(win, EXPANDED_WIDTH);
    }
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
  }, [state]);

  return { bubbleRef, contentRef };
}

function positionTopRight(win: ReturnType<typeof getCurrentWebviewWindow>, width: number) {
  // Use browser screen API for monitor dimensions (works in Tauri webview)
  const screenW = window.screen.width;
  const x = screenW - width - MARGIN_RIGHT;
  win.setPosition(new LogicalPosition(x, MARGIN_TOP)).catch(() => {});
}
