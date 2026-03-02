import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { StatusIndicator } from "./components/StatusIndicator";
import { TaskList } from "./components/TaskList";
import { useScreenContext } from "./hooks/useScreenContext";
import { useTasks } from "./hooks/useTasks";
import type { AppSwitchEvent } from "./types";

export default function TrayApp() {
  const { loading, error, analyze } = useScreenContext();
  const { tasks, addTask, toggleTask, removeTask } = useTasks();

  useEffect(() => {
    let analyzeTimeout: ReturnType<typeof setTimeout> | null = null;
    let lastAnalysis = 0;

    // Listen for app switch events
    const unlistenAppSwitch = listen<AppSwitchEvent>(
      "app-switched",
      (_event) => {
        // Debounce: wait 2 seconds after app switch before analyzing
        if (analyzeTimeout) clearTimeout(analyzeTimeout);
        analyzeTimeout = setTimeout(() => {
          const now = Date.now();
          if (now - lastAnalysis > 10000) {
            lastAnalysis = now;
            window.dispatchEvent(new CustomEvent("yoyo-auto-analyze"));
          }
        }, 2000);
      }
    );

    // Register global shortcuts
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
          window.dispatchEvent(new CustomEvent("yoyo-auto-analyze"));
        });
      } catch (e) {
        console.warn("Failed to register analyze shortcut:", e);
      }
    };

    registerShortcuts();

    return () => {
      unlistenAppSwitch.then((fn) => fn());
      if (analyzeTimeout) clearTimeout(analyzeTimeout);
    };
  }, []);

  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
        <span className="text-sm font-semibold tracking-wide">YoYo</span>
        <button
          onClick={() => analyze(0)}
          disabled={loading}
          className="px-2.5 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded
            disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {loading ? "Analyzing..." : "Analyze"}
        </button>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {/* Status */}
        <div className="px-3 py-1.5">
          <StatusIndicator loading={loading} error={error} />
        </div>

        {/* Divider */}
        <div className="border-t border-zinc-800" />

        {/* Tasks */}
        <TaskList
          tasks={tasks}
          onToggle={toggleTask}
          onAdd={addTask}
          onRemove={removeTask}
        />
      </div>
    </div>
  );
}
