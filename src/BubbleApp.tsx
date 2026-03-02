import { useEffect, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ActionButtons } from "./components/ActionButtons";
import { useActions } from "./hooks/useActions";
import type { AnalysisResult, Settings, SuggestedAction } from "./types";

export default function BubbleApp() {
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [opacity, setOpacity] = useState(0.85);
  const [visible, setVisible] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const { executing, execute } = useActions();

  const hideBubble = useCallback(async () => {
    setVisible(false);
    // Wait for fade-out animation then hide window
    setTimeout(async () => {
      const win = getCurrentWebviewWindow();
      await win.hide();
    }, 200);
  }, []);

  useEffect(() => {
    // Load opacity setting
    invoke<Settings>("get_settings").then((s) => {
      if (s.bubble_opacity !== undefined) setOpacity(s.bubble_opacity);
    });

    // On mount, fetch cached result
    invoke<AnalysisResult | null>("get_last_analysis").then((cached) => {
      if (cached) {
        setResult(cached);
        setVisible(true);
      }
    });

    // Show refreshing indicator on app switch; content updates when analysis completes
    const unlistenSwitch = listen("app-switched", () => {
      setRefreshing(true);
    });

    // Listen for new analysis results — keeps bubble content in sync on app switch
    const unlistenAnalysis = listen<AnalysisResult>(
      "analysis-complete",
      (event) => {
        setResult(event.payload);
        setVisible(true);
        setRefreshing(false);
      }
    );

    // Listen for opacity changes from tray panel
    const unlistenOpacity = listen<number>("bubble-opacity-changed", (event) => {
      setOpacity(event.payload);
    });

    return () => {
      unlistenSwitch.then((fn) => fn());
      unlistenAnalysis.then((fn) => fn());
      unlistenOpacity.then((fn) => fn());
    };
  }, []);

  const handleExecute = async (action: SuggestedAction) => {
    await execute(action);
    hideBubble();
  };

  if (!result) {
    return null;
  }

  return (
    <div
      className={`bubble-container ${visible ? "bubble-enter" : "bubble-exit"}`}
      style={{ opacity }}
    >
      <div className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
        shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
        text-white select-none overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 pt-3 pb-2">
          <div className="flex items-center gap-2">
            {refreshing ? (
              <span className="w-2 h-2 border border-zinc-400 border-t-transparent rounded-full animate-spin" />
            ) : (
              <div className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]" />
            )}
            <span className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
              YoYo
            </span>
          </div>
          <button
            onClick={hideBubble}
            className="w-5 h-5 flex items-center justify-center rounded-full
              text-zinc-500 hover:text-zinc-300 hover:bg-white/10 transition-all"
          >
            <svg viewBox="0 0 12 12" fill="none" className="w-2.5 h-2.5">
              <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round"/>
            </svg>
          </button>
        </div>

        {/* Context */}
        <div className="px-4 pb-2">
          <p className="text-[13px] text-zinc-300 leading-snug">{result.context}</p>
        </div>

        {/* Separator */}
        <div className="mx-4 border-t border-white/[0.06]" />

        {/* Actions — clean row style */}
        <ActionButtons
          actions={result.actions}
          executing={executing}
          onExecute={handleExecute}
          compact
        />

        {/* Footer */}
        <div className="px-4 py-2 flex items-center border-t border-white/[0.06]">
          <span className="text-[10px] text-zinc-600">
            <kbd className="px-1 py-0.5 rounded bg-white/[0.06] text-zinc-500 font-mono text-[9px]">
              Cmd+Shift+R
            </kbd>
            {" "}refresh
          </span>
        </div>
      </div>
    </div>
  );
}
