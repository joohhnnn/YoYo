import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { ActionButtons } from "./components/ActionButtons";
import { useActions } from "./hooks/useActions";
import { useWindowAutoResize } from "./hooks/useWindowAutoResize";
import type { AnalysisResult, Settings, SuggestedAction } from "./types";

export default function BubbleApp() {
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [opacity, setOpacity] = useState(0.85);
  const [visible, setVisible] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [analysisStage, setAnalysisStage] = useState<string | null>(null);
  const [actionDone, setActionDone] = useState(false);
  const { executing, execute } = useActions();

  // Dynamic window resize
  const { bubbleRef, contentRef } = useWindowAutoResize();

  useEffect(() => {
    // Load opacity setting
    invoke<Settings>("get_settings").then((s) => {
      if (s.bubble_opacity !== undefined) setOpacity(s.bubble_opacity);
    });

    // Load cached analysis result
    invoke<AnalysisResult | null>("get_last_analysis").then((cached) => {
      if (cached) {
        setResult(cached);
        setVisible(true);
      }
    });

    const unlistenSwitch = listen("app-switched", () => {
      setRefreshing(true);
      setAnalysisStage(null);
    });

    const unlistenProgress = listen<string>("analysis-progress", (event) => {
      setAnalysisStage(event.payload);
    });

    const unlistenAnalysis = listen<AnalysisResult>(
      "analysis-complete",
      (event) => {
        setResult(event.payload);
        setVisible(true);
        setRefreshing(false);
        setAnalysisStage(null);
      }
    );

    const unlistenOpacity = listen<number>(
      "bubble-opacity-changed",
      (event) => {
        setOpacity(event.payload);
      }
    );

    return () => {
      [
        unlistenSwitch,
        unlistenProgress,
        unlistenAnalysis,
        unlistenOpacity,
      ].forEach((u) => u.then((fn) => fn()));
    };
  }, []);

  const handleExecute = async (action: SuggestedAction) => {
    await execute(action);
    setActionDone(true);
    setTimeout(() => setActionDone(false), 1200);
  };

  return (
    <div
      className={`bubble-container ${visible ? "bubble-enter" : "bubble-exit"}`}
      style={{ opacity }}
    >
      <div
        ref={bubbleRef}
        className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
        shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
        text-white select-none overflow-hidden flex flex-col max-h-screen"
      >
        {/* Header — pinned top */}
        <div className="flex items-center justify-between px-4 pt-3 pb-2 flex-shrink-0">
          <div className="flex items-center gap-2">
            {refreshing ? (
              <span className="w-2 h-2 border border-zinc-400 border-t-transparent rounded-full animate-spin" />
            ) : (
              <div className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]" />
            )}
            <span className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
              {refreshing && analysisStage ? analysisStage : "YoYo"}
            </span>
          </div>
        </div>

        {/* Scrollable content area */}
        <div className="flex-1 min-h-0 overflow-y-auto">
          <div ref={contentRef}>
            {/* Context (from analysis) */}
            {result && (
              <div className="px-4 pb-2">
                <p className="text-[13px] text-zinc-300 leading-snug">
                  {result.context}
                </p>
              </div>
            )}

            {/* Separator */}
            {result && (
              <div className="mx-4 border-t border-white/[0.06]" />
            )}

            {/* Actions or status overlay */}
            {executing || actionDone ? (
              <div className="px-4 py-6 flex flex-col items-center gap-2">
                {actionDone ? (
                  <>
                    <svg
                      viewBox="0 0 24 24"
                      fill="none"
                      className="w-6 h-6 text-emerald-400"
                    >
                      <path
                        d="M5 13l4 4L19 7"
                        stroke="currentColor"
                        strokeWidth="2"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                    <span className="text-[12px] text-zinc-400">Done</span>
                  </>
                ) : (
                  <>
                    <span className="w-5 h-5 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
                    <span className="text-[12px] text-zinc-400">
                      Processing...
                    </span>
                  </>
                )}
              </div>
            ) : (
              <>
                {/* Action buttons */}
                {result && (
                  <ActionButtons
                    actions={result.actions}
                    executing={executing}
                    onExecute={handleExecute}
                    compact
                  />
                )}
              </>
            )}
          </div>
        </div>

        {/* Footer — pinned bottom */}
        <div className="flex-shrink-0">
          <div className="px-4 py-2 flex items-center border-t border-white/[0.06]">
            <span className="text-[10px] text-zinc-600">
              <kbd className="px-1 py-0.5 rounded bg-white/[0.06] text-zinc-500 font-mono text-[9px]">
                Cmd+Shift+R
              </kbd>{" "}
              refresh
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
