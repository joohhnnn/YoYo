import { useState, useEffect } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPanel } from "./components/SettingsPanel";
import { OnboardingPanel } from "./components/OnboardingPanel";
import type { Settings } from "./types";

export default function SettingsApp() {
  const [settings, setSettings] = useState<Settings | null>(null);

  useEffect(() => {
    invoke<Settings>("get_settings").then(setSettings);
  }, []);

  if (!settings) return null;

  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
      {settings.onboarding_completed ? (
        <SettingsPanel onClose={() => getCurrentWebviewWindow().hide()} />
      ) : (
        <OnboardingPanel
          onComplete={() =>
            setSettings({ ...settings, onboarding_completed: true })
          }
        />
      )}
    </div>
  );
}
