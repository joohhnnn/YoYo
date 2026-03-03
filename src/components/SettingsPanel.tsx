import { useState, useEffect } from "react";
import { emit } from "@tauri-apps/api/event";
import { getSettings, saveSettings } from "../services/storage";
import type { Settings } from "../types";

const MODEL_OPTIONS = [
  { value: "claude-haiku-4-5-20251001", label: "Haiku 4.5", desc: "Fast" },
  { value: "claude-sonnet-4-20250514", label: "Sonnet 4", desc: "Balanced" },
];

const DEPTH_OPTIONS = [
  { value: "casual", label: "清闲", desc: "只关注大致活动" },
  { value: "normal", label: "普通", desc: "关注焦点区域" },
  { value: "deep", label: "硬核", desc: "读取所有可见文字" },
];

interface SettingsPanelProps {
  onClose: () => void;
}

export function SettingsPanel({ onClose }: SettingsPanelProps) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    getSettings().then(setSettings);
  }, []);

  const update = async (patch: Partial<Settings>) => {
    if (!settings) return;
    const next = { ...settings, ...patch };
    setSettings(next);
    await saveSettings(next);

    // Emit opacity change if it was updated
    if (patch.bubble_opacity !== undefined) {
      await emit("bubble-opacity-changed", patch.bubble_opacity);
    }

    setSaved(true);
    setTimeout(() => setSaved(false), 800);
  };

  if (!settings) return null;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
        <span className="text-sm font-semibold tracking-wide">Settings</span>
        <div className="flex items-center gap-2">
          {saved && (
            <span className="text-[10px] text-emerald-400">Saved</span>
          )}
          <button
            onClick={onClose}
            className="text-zinc-400 hover:text-white transition-colors text-xs"
          >
            Done
          </button>
        </div>
      </div>

      {/* Scrollable settings */}
      <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2 space-y-4">
        {/* AI Mode */}
        <SettingRow label="AI Mode">
          <div className="flex gap-1">
            {(["cli", "api"] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => update({ ai_mode: mode })}
                className={`flex-1 text-[10px] py-1 rounded transition-colors ${
                  settings.ai_mode === mode
                    ? "bg-blue-600 text-white"
                    : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                }`}
              >
                {mode.toUpperCase()}
              </button>
            ))}
          </div>
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
                focus:border-blue-500/50"
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
                    ? "bg-blue-600 text-white"
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

        {/* Analysis Depth */}
        <SettingRow label="Analysis Depth">
          <div className="flex gap-1">
            {DEPTH_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                onClick={() => update({ analysis_depth: opt.value })}
                className={`flex-1 text-[10px] py-1 rounded transition-colors ${
                  settings.analysis_depth === opt.value
                    ? "bg-blue-600 text-white"
                    : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                }`}
              >
                {opt.label}
              </button>
            ))}
          </div>
          <p className="text-[9px] text-zinc-600 mt-1">
            {DEPTH_OPTIONS.find((o) => o.value === settings.analysis_depth)
              ?.desc ?? "关注焦点区域"}
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
                    ? "bg-blue-600 text-white"
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
                settings.auto_analyze ? "bg-blue-600" : "bg-zinc-700"
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
      </div>
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
