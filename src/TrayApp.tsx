import { useEffect, useState } from "react";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { StatusIndicator } from "./components/StatusIndicator";
import { QuestBoard } from "./components/QuestBoard";
import { SettingsPanel } from "./components/SettingsPanel";
import { useScreenContext } from "./hooks/useScreenContext";
import { useTasks } from "./hooks/useTasks";

export default function TrayApp() {
  const { loading, error, analyze } = useScreenContext();
  const { tasks, addTask, toggleTask, removeTask, updateProgress } =
    useTasks();
  const [showSettings, setShowSettings] = useState(false);

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
