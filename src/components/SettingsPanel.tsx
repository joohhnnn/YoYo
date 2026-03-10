import { useState, useEffect, useRef } from "react";
import { emit } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getSettings, saveSettings } from "../services/storage";
import { getProfile, saveProfile, getContext, saveContext } from "../services/userdata";
import { HistoryPanel } from "./HistoryPanel";
import type { AudioDevice, Settings } from "../types";

const MODEL_OPTIONS = [
  { value: "claude-haiku-4-5-20251001", label: "Haiku 4.5", desc: "Fast" },
  { value: "claude-sonnet-4-20250514", label: "Sonnet 4", desc: "Balanced" },
];

const PRESET_OPTIONS = [
  {
    label: "Developer",
    icon: "{ }",
    config: { model: "claude-haiku-4-5-20251001", auto_analyze: true, analysis_cooldown_secs: 5 },
  },
  {
    label: "Student",
    icon: "📖",
    config: { model: "claude-haiku-4-5-20251001", auto_analyze: true, analysis_cooldown_secs: 10 },
  },
  {
    label: "Writer",
    icon: "✍",
    config: { model: "claude-haiku-4-5-20251001", auto_analyze: false, analysis_cooldown_secs: 15 },
  },
];

type Tab = "settings" | "profile" | "context" | "history";

interface SettingsPanelProps {
  onClose: () => void;
}

export function SettingsPanel({ onClose }: SettingsPanelProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState(false);
  const [tab, setTab] = useState<Tab>("settings");

  // Profile / Context editing state
  const [profileText, setProfileText] = useState("");
  const [contextText, setContextText] = useState("");
  const [editorDirty, setEditorDirty] = useState(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout>>();
  const [audioDevices, setAudioDevices] = useState<AudioDevice[]>([]);
  const [blacklistInput, setBlacklistInput] = useState("");

  useEffect(() => {
    getSettings().then(setSettings);
    invoke<AudioDevice[]>("list_audio_devices")
      .then(setAudioDevices)
      .catch(() => {});
  }, []);

  // Load profile/context when switching tabs
  useEffect(() => {
    if (tab === "profile") {
      getProfile().then((t) => { setProfileText(t); setEditorDirty(false); });
    } else if (tab === "context") {
      getContext().then((t) => { setContextText(t); setEditorDirty(false); });
    }
  }, [tab]);

  const update = async (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    await saveSettings(next);

    if (patch.bubble_opacity !== undefined) {
      await emit("bubble-opacity-changed", patch.bubble_opacity);
    }

    setSaved(true);
    setTimeout(() => setSaved(false), 800);
  };

  const handleShortcutChange = async (key: "shortcut_toggle" | "shortcut_analyze", value: string) => {
    if (!settings) return;
    const next = { ...settings, [key]: value };
    setSettings(next);
    await saveSettings(next);
    await emit("shortcuts-changed", {
      toggle: next.shortcut_toggle,
      analyze: next.shortcut_analyze,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 800);
  };

  const addToBlacklist = async () => {
    if (!settings || !blacklistInput.trim()) return;
    const id = blacklistInput.trim();
    if (settings.app_blacklist.includes(id)) return;
    await update({ app_blacklist: [...settings.app_blacklist, id] });
    setBlacklistInput("");
  };

  const removeFromBlacklist = async (id: string) => {
    if (!settings) return;
    await update({ app_blacklist: settings.app_blacklist.filter((b) => b !== id) });
  };

  // Auto-save profile/context with debounce
  const handleEditorChange = (text: string, type: "profile" | "context") => {
    if (type === "profile") setProfileText(text);
    else setContextText(text);
    setEditorDirty(true);

    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(async () => {
      if (type === "profile") await saveProfile(text);
      else await saveContext(text);
      setEditorDirty(false);
      setSaved(true);
      setTimeout(() => setSaved(false), 800);
    }, 600);
  };

  if (!settings) return null;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
        <div className="flex items-center gap-1">
          {(["settings", "profile", "context", "history"] as Tab[]).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`text-[10px] px-1.5 py-0.5 rounded transition-colors ${
                tab === t
                  ? "bg-zinc-700 text-white"
                  : "text-zinc-500 hover:text-zinc-300"
              }`}
            >
              {t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-2">
          {saved && <span className="text-[10px] text-emerald-400">Saved</span>}
          {editorDirty && <span className="text-[10px] text-amber-400">Editing...</span>}
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-white transition-colors text-xs"
          >
            Done
          </button>
        </div>
      </div>

      {/* Tab content */}
      {tab === "settings" ? (
        <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2 space-y-4">
          {/* Presets */}
          <SettingRow label="Quick Setup">
            <div className="flex gap-1">
              {PRESET_OPTIONS.map((preset) => (
                <button
                  key={preset.label}
                  onClick={() => update(preset.config)}
                  className="flex-1 text-[10px] py-1.5 rounded bg-zinc-800 text-zinc-400
                    hover:bg-zinc-700 hover:text-zinc-200 transition-colors"
                >
                  <span className="block text-[12px]">{preset.icon}</span>
                  {preset.label}
                </button>
              ))}
            </div>
            <p className="text-[9px] text-zinc-600 mt-1">
              One-click preset for common workflows
            </p>
          </SettingRow>

          {/* AI Mode */}
          <SettingRow label="AI Mode">
            <div className="flex gap-1">
              {(["cli", "api"] as const).map((mode) => (
                <button
                  key={mode}
                  onClick={() => update({ ai_mode: mode })}
                  className={`flex-1 text-[10px] py-1 rounded transition-colors ${
                    settings.ai_mode === mode
                      ? "bg-violet-600 text-white"
                      : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                  }`}
                >
                  {mode.toUpperCase()}
                </button>
              ))}
            </div>
            <p className="text-[9px] text-zinc-600 mt-1">
              {settings.ai_mode === "cli" ? "Uses locally installed Claude CLI" : "Direct API calls (needs API key)"}
            </p>
          </SettingRow>

          {/* API Key (only visible in API mode) */}
          {settings.ai_mode === "api" && (
            <SettingRow label="API Key">
              <input
                type="password"
                value={settings.api_key}
                onChange={(e) => update({ api_key: e.target.value })}
                placeholder="sk-ant-..."
                className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1
                  text-[11px] text-white placeholder-zinc-600 outline-none
                  focus:border-violet-500/50"
              />
            </SettingRow>
          )}

          {/* Model */}
          <SettingRow label="Model">
            <div className="flex gap-1">
              {MODEL_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => update({ model: opt.value })}
                  className={`flex-1 text-[10px] py-1 rounded transition-colors ${
                    settings.model === opt.value
                      ? "bg-violet-600 text-white"
                      : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                  }`}
                >
                  {opt.label}
                </button>
              ))}
            </div>
            <p className="text-[9px] text-zinc-600 mt-1">
              {MODEL_OPTIONS.find((o) => o.value === settings.model)?.desc ??
                settings.model}
            </p>
          </SettingRow>

          {/* Language */}
          <SettingRow label="Language">
            <div className="flex gap-1">
              {(["zh", "en"] as const).map((lang) => (
                <button
                  key={lang}
                  onClick={() => update({ language: lang })}
                  className={`flex-1 text-[10px] py-1 rounded transition-colors ${
                    settings.language === lang
                      ? "bg-violet-600 text-white"
                      : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                  }`}
                >
                  {lang === "zh" ? "中文" : "EN"}
                </button>
              ))}
            </div>
          </SettingRow>

          {/* Auto Analyze */}
          <SettingRow label="Auto Analyze">
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-zinc-500">
                Analyze on app switch
              </span>
              <button
                onClick={() => update({ auto_analyze: !settings.auto_analyze })}
                className={`w-8 h-[18px] rounded-full transition-colors relative ${
                  settings.auto_analyze ? "bg-violet-600" : "bg-zinc-700"
                }`}
              >
                <span
                  className={`absolute top-[2px] w-[14px] h-[14px] rounded-full bg-white transition-transform ${
                    settings.auto_analyze ? "left-[16px]" : "left-[2px]"
                  }`}
                />
              </button>
            </div>
          </SettingRow>

          {/* Sound */}
          <SettingRow label="Sound">
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-zinc-500">
                Play sound effects
              </span>
              <button
                onClick={() => update({ sound_enabled: !settings.sound_enabled })}
                className={`w-8 h-[18px] rounded-full transition-colors relative ${
                  settings.sound_enabled ? "bg-violet-600" : "bg-zinc-700"
                }`}
              >
                <span
                  className={`absolute top-[2px] w-[14px] h-[14px] rounded-full bg-white transition-transform ${
                    settings.sound_enabled ? "left-[16px]" : "left-[2px]"
                  }`}
                />
              </button>
            </div>
          </SettingRow>

          {/* Cooldown */}
          <SettingRow label="Cooldown">
            <div className="flex items-center gap-2">
              <input
                type="range"
                min="2"
                max="30"
                step="1"
                value={settings.analysis_cooldown_secs}
                onChange={(e) =>
                  update({ analysis_cooldown_secs: Number(e.target.value) })
                }
                className="flex-1 h-1 appearance-none bg-zinc-700 rounded-full cursor-pointer
                  [&::-webkit-slider-thumb]:appearance-none
                  [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3
                  [&::-webkit-slider-thumb]:bg-zinc-300 [&::-webkit-slider-thumb]:rounded-full
                  [&::-webkit-slider-thumb]:cursor-pointer
                  [&::-webkit-slider-thumb]:hover:bg-white"
              />
              <span className="text-[10px] text-zinc-500 w-6 text-right">
                {settings.analysis_cooldown_secs}s
              </span>
            </div>
          </SettingRow>

          {/* Bubble Opacity */}
          <SettingRow label="Opacity">
            <div className="flex items-center gap-2">
              <input
                type="range"
                min="0.3"
                max="1"
                step="0.05"
                value={settings.bubble_opacity}
                onChange={(e) =>
                  update({ bubble_opacity: Number(e.target.value) })
                }
                className="flex-1 h-1 appearance-none bg-zinc-700 rounded-full cursor-pointer
                  [&::-webkit-slider-thumb]:appearance-none
                  [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3
                  [&::-webkit-slider-thumb]:bg-zinc-300 [&::-webkit-slider-thumb]:rounded-full
                  [&::-webkit-slider-thumb]:cursor-pointer
                  [&::-webkit-slider-thumb]:hover:bg-white"
              />
              <span className="text-[10px] text-zinc-500 w-7 text-right">
                {Math.round(settings.bubble_opacity * 100)}%
              </span>
            </div>
          </SettingRow>

          {/* Hotkeys */}
          <SettingRow label="Hotkeys">
            <div className="space-y-1.5">
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-zinc-500 w-14">Toggle</span>
                <input
                  value={settings.shortcut_toggle}
                  onChange={(e) => handleShortcutChange("shortcut_toggle", e.target.value)}
                  className="flex-1 bg-zinc-800 border border-zinc-700 rounded px-2 py-1
                    text-[11px] text-white outline-none focus:border-violet-500/50 font-mono"
                />
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-zinc-500 w-14">Analyze</span>
                <input
                  value={settings.shortcut_analyze}
                  onChange={(e) => handleShortcutChange("shortcut_analyze", e.target.value)}
                  className="flex-1 bg-zinc-800 border border-zinc-700 rounded px-2 py-1
                    text-[11px] text-white outline-none focus:border-violet-500/50 font-mono"
                />
              </div>
            </div>
            <p className="text-[9px] text-zinc-600 mt-1">
              Format: CmdOrCtrl+Shift+Key
            </p>
          </SettingRow>

          {/* App Blacklist */}
          <SettingRow label="App Blacklist">
            <div className="space-y-1 max-h-24 overflow-y-auto">
              {settings.app_blacklist.map((id) => (
                <div key={id} className="flex items-center gap-2 bg-zinc-800 rounded px-2 py-1">
                  <span className="flex-1 text-[10px] text-zinc-400 truncate">{id}</span>
                  <button
                    onClick={() => removeFromBlacklist(id)}
                    className="text-zinc-600 hover:text-zinc-300 text-[12px] flex-shrink-0"
                  >
                    ×
                  </button>
                </div>
              ))}
            </div>
            <div className="flex gap-1 mt-1.5">
              <input
                value={blacklistInput}
                onChange={(e) => setBlacklistInput(e.target.value)}
                placeholder="com.example.app"
                className="flex-1 bg-zinc-800 border border-zinc-700 rounded px-2 py-1
                  text-[10px] text-white placeholder-zinc-600 outline-none
                  focus:border-violet-500/50"
                onKeyDown={(e) => { if (e.key === "Enter") addToBlacklist(); }}
              />
              <button
                onClick={addToBlacklist}
                className="text-[10px] px-2 py-1 rounded bg-zinc-700 hover:bg-zinc-600
                  text-zinc-300 transition-colors"
              >
                Add
              </button>
            </div>
            <p className="text-[9px] text-zinc-600 mt-1">
              Apps that YoYo won't analyze (bundle IDs)
            </p>
          </SettingRow>

          {/* Microphone Device */}
          <SettingRow label="Microphone">
            <select
              value={settings.preferred_mic_device}
              onChange={(e) => update({ preferred_mic_device: e.target.value })}
              className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1
                text-[11px] text-white outline-none focus:border-violet-500/50
                appearance-none cursor-pointer"
            >
              <option value="">System Default</option>
              {audioDevices.map((d) => (
                <option key={d.name} value={d.name}>{d.name}</option>
              ))}
            </select>
            <p className="text-[9px] text-zinc-600 mt-1">
              Audio input device for voice recording
            </p>
          </SettingRow>
        </div>
      ) : tab === "history" ? (
        <HistoryPanel />
      ) : (
        /* Profile / Context editor */
        <div className="flex-1 min-h-0 flex flex-col">
          <div className="px-3 py-1.5">
            <p className="text-[9px] text-zinc-500">
              {tab === "profile"
                ? "Tell YoYo about yourself — this personalizes AI suggestions (~/.yoyo/profile.md)"
                : "What you're working on right now — updated by you and AI reflections (~/.yoyo/context.md)"}
            </p>
          </div>
          <textarea
            value={tab === "profile" ? profileText : contextText}
            onChange={(e) => handleEditorChange(e.target.value, tab as "profile" | "context")}
            className="flex-1 mx-3 mb-2 p-2 text-[11px] leading-relaxed bg-zinc-800 border border-zinc-700
              rounded text-zinc-200 placeholder-zinc-600 outline-none resize-none
              focus:border-violet-500/50 font-mono"
            placeholder={tab === "profile" ? "# About Me\n..." : "# Current Context\n..."}
          />
        </div>
      )}
    </div>
  );
}

function SettingRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1">
      <span className="text-[10px] text-zinc-500 uppercase tracking-wider">
        {label}
      </span>
      {children}
    </div>
  );
}
