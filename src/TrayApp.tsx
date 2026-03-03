import { useEffect, useState } from "react";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { StatusIndicator } from "./components/StatusIndicator";
import { QuestBoard } from "./components/QuestBoard";
import { SettingsPanel } from "./components/SettingsPanel";
import { useScreenContext } from "./hooks/useScreenContext";
import { useTasks } from "./hooks/useTasks";
import type { AnalysisResult } from "./types";

export default function TrayApp() {
  const { loading, error, analyze } = useScreenContext();
  const { tasks, addTask, toggleTask, removeTask, updateProgress } =
    useTasks();
  const [showSettings, setShowSettings] = useState(false);
  const [suggestedQuest, setSuggestedQuest] = useState<string | null>(null);

  useEffect(() => {
    const registerShortcuts = async () => {
      try {
        await register("CmdOrCtrl+Shift+Y", async (event) => {
          if (event.state === "Released") return;
          const win = getCurrentWebviewWindow();
          const visible = await win.isVisible();
          if (visible) {
            await win.hide();
          } else {
            await win.show();
            await win.setFocus();
          }
        });
      } catch (e) {
        console.warn("Failed to register toggle shortcut:", e);
      }

      try {
        await register("CmdOrCtrl+Shift+R", (event) => {
          if (event.state === "Released") return;
          invoke("analyze_screen").catch(console.error);
        });
      } catch (e) {
        console.warn("Failed to register analyze shortcut:", e);
      }
    };

    registerShortcuts();

    // Listen for suggested quest from analysis
    const unlistenAnalysis = listen<AnalysisResult>(
      "analysis-complete",
      (event) => {
        if (event.payload.suggested_quest) {
          setSuggestedQuest(event.payload.suggested_quest);
        }
      }
    );

    return () => {
      unlistenAnalysis.then((fn) => fn());
    };
  }, []);

  if (showSettings) {
    return (
      <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
        <SettingsPanel onClose={() => setShowSettings(false)} />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
        <span className="text-sm font-semibold tracking-wide">YoYo</span>
        <div className="flex items-center gap-1.5">
          <button
            onClick={() => setShowSettings(true)}
            className="p-1 text-zinc-500 hover:text-zinc-300 transition-colors"
            title="Settings"
          >
            <svg viewBox="0 0 16 16" fill="none" className="w-3.5 h-3.5">
              <path
                d="M6.5 1.5h3l.5 2 1.5.7 1.8-1 2.1 2.1-1 1.8.7 1.5 2 .5v3l-2 .5-1.5.7-1 1.8-2.1 2.1-1.8-1-1.5.7-.5 2h-3l-.5-2-1.5-.7-1.8 1-2.1-2.1 1-1.8-.7-1.5-2-.5v-3l2-.5 1.5-.7 1-1.8 2.1-2.1 1.8 1z"
                stroke="currentColor"
                strokeWidth="1.2"
                strokeLinejoin="round"
              />
              <circle
                cx="8"
                cy="8"
                r="2"
                stroke="currentColor"
                strokeWidth="1.2"
              />
            </svg>
          </button>
          <button
            onClick={() => analyze(0)}
            disabled={loading}
            className="px-2.5 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded
              disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {loading ? "Analyzing..." : "Analyze"}
          </button>
        </div>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {/* Status */}
        <div className="px-3 py-1.5">
          <StatusIndicator loading={loading} error={error} />
        </div>

        <div className="border-t border-zinc-800" />

        {/* Suggested Quest */}
        {suggestedQuest && (
          <div className="mx-3 mt-2 px-3 py-1.5 bg-blue-500/[0.06] border border-blue-500/15 rounded-lg flex items-center gap-2">
            <svg viewBox="0 0 12 12" className="w-3.5 h-3.5 text-blue-400 flex-shrink-0" fill="none" stroke="currentColor" strokeWidth="1.5">
              <path d="M6 1.5a2.5 2.5 0 012.5 2.5c0 1.2-.8 1.8-1.2 2.3-.3.3-.5.6-.5 1v.2M6 9.5v.5" strokeLinecap="round" />
            </svg>
            <span className="text-[12px] text-zinc-300 flex-1 truncate">{suggestedQuest}</span>
            <button
              onClick={() => {
                addTask(suggestedQuest, "main");
                setSuggestedQuest(null);
              }}
              className="p-1 text-emerald-400 hover:text-emerald-300 transition-colors flex-shrink-0"
              title="Accept as main quest"
            >
              <svg viewBox="0 0 12 12" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M2.5 6l2.5 2.5 4.5-5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            <button
              onClick={() => setSuggestedQuest(null)}
              className="p-1 text-zinc-500 hover:text-zinc-300 transition-colors flex-shrink-0"
              title="Dismiss"
            >
              <svg viewBox="0 0 12 12" className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M3 3l6 6M9 3l-6 6" strokeLinecap="round" />
              </svg>
            </button>
          </div>
        )}

        {/* Quest Board */}
        <QuestBoard
          tasks={tasks}
          onToggle={toggleTask}
          onAdd={addTask}
          onRemove={removeTask}
          onUpdateProgress={updateProgress}
        />
      </div>
    </div>
  );
}
