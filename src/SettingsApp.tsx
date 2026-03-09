import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { SettingsPanel } from "./components/SettingsPanel";

export default function SettingsApp() {
  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
      <SettingsPanel onClose={() => getCurrentWebviewWindow().hide()} />
    </div>
  );
}
