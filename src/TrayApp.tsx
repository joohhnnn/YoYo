import { useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { StatusIndicator } from "./components/StatusIndicator";
import { TaskList } from "./components/TaskList";
import { useScreenContext } from "./hooks/useScreenContext";
import { useTasks } from "./hooks/useTasks";
import { getSettings, saveSettings } from "./services/storage";

export default function TrayApp() {
  const { loading, error, analyze } = useScreenContext();
  const { tasks, addTask, toggleTask, removeTask } = useTasks();
  const [bubbleOpacity, setBubbleOpacity] = useState(0.85);

  // Load opacity on mount
  useEffect(() => {
    getSettings().then((s) => {
      if (s.bubble_opacity !== undefined) setBubbleOpacity(s.bubble_opacity);
    });
  }, []);

  const handleOpacityChange = async (value: number) => {
    setBubbleOpacity(value);
    await emit("bubble-opacity-changed", value);
    const settings = await getSettings();
    await saveSettings({ ...settings, bubble_opacity: value });
  };

  useEffect(() => {
    // Register global shortcuts only
    // Auto-analysis on app-switch is handled by Rust backend
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
          // Trigger analysis via invoke (Rust handles broadcast + bubble)
          invoke("analyze_screen").catch(console.error);
        });
      } catch (e) {
        console.warn("Failed to register analyze shortcut:", e);
      }
    };

    registerShortcuts();
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

        <div className="border-t border-zinc-800" />

        {/* Tasks */}
        <TaskList
          tasks={tasks}
          onToggle={toggleTask}
          onAdd={addTask}
          onRemove={removeTask}
        />
      </div>

      {/* Bubble opacity control */}
      <div className="px-3 py-2 border-t border-zinc-800">
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-zinc-500 flex-shrink-0">Opacity</span>
          <input
            type="range"
            min="0.3"
            max="1"
            step="0.05"
            value={bubbleOpacity}
            onChange={(e) => handleOpacityChange(Number(e.target.value))}
            className="flex-1 h-1 appearance-none bg-zinc-700 rounded-full cursor-pointer
              [&::-webkit-slider-thumb]:appearance-none
              [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3
              [&::-webkit-slider-thumb]:bg-zinc-300 [&::-webkit-slider-thumb]:rounded-full
              [&::-webkit-slider-thumb]:cursor-pointer
              [&::-webkit-slider-thumb]:hover:bg-white"
          />
          <span className="text-[10px] text-zinc-500 w-7 text-right">
            {Math.round(bubbleOpacity * 100)}%
          </span>
        </div>
      </div>
    </div>
  );
}
