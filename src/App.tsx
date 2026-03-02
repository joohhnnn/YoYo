import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { TaskBar } from "./components/TaskBar";
import type { AppSwitchEvent } from "./types";

function App() {
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
            // Dispatch a custom event that components can listen to
            window.dispatchEvent(new CustomEvent("yoyo-auto-analyze"));
          }
        }, 2000);
      }
    );

    // Register global shortcuts
    const registerShortcuts = async () => {
      try {
        await register("CmdOrCtrl+Shift+Y", async (event) => {
          // The callback fires on both press and release — only handle press
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

  return <TaskBar />;
}

export default App;
