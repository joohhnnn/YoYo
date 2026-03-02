import { useEffect, useState, useRef, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ActionButtons } from "./components/ActionButtons";
import { useActions } from "./hooks/useActions";
import type { AnalysisResult, SuggestedAction } from "./types";

const AUTO_HIDE_MS = 15_000;

export default function BubbleApp() {
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const { executing, execute } = useActions();
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const hideBubble = useCallback(async () => {
    const win = getCurrentWebviewWindow();
    await win.hide();
  }, []);

  const resetAutoHide = useCallback(() => {
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(hideBubble, AUTO_HIDE_MS);
  }, [hideBubble]);

  useEffect(() => {
    // On mount, fetch cached result (handles first-create timing issue)
    invoke<AnalysisResult | null>("get_last_analysis").then((cached) => {
      if (cached) {
        setResult(cached);
        resetAutoHide();
      }
    });

    // Listen for new analysis results
    const unlisten = listen<AnalysisResult>("analysis-complete", (event) => {
      setResult(event.payload);
      resetAutoHide();
    });

    return () => {
      unlisten.then((fn) => fn());
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [resetAutoHide]);

  const handleExecute = async (action: SuggestedAction) => {
    await execute(action);
    hideBubble();
  };

  if (!result) {
    return null;
  }

  return (
    <div
      className="bg-zinc-900/90 backdrop-blur-sm rounded-2xl border border-zinc-700/50
        shadow-2xl shadow-black/50 text-white select-none overflow-hidden"
      onMouseEnter={resetAutoHide}
    >
      {/* Context */}
      <div className="px-3 py-2 border-b border-zinc-800/50">
        <p className="text-xs text-zinc-400 truncate">{result.context}</p>
      </div>

      {/* Actions */}
      <ActionButtons
        actions={result.actions}
        executing={executing}
        onExecute={handleExecute}
      />

      {/* Dismiss */}
      <div className="px-3 pb-2 flex justify-end">
        <button
          onClick={hideBubble}
          className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
}
