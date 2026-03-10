import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type { Settings } from "../types";

type Step = "permissions" | "hotkeys" | "ready";
const STEPS: Step[] = ["permissions", "hotkeys", "ready"];

interface OnboardingPanelProps {
  onComplete: () => void;
}

interface PermissionState {
  accessibility: boolean;
  microphone: "granted" | "denied" | "not_determined";
}

export function OnboardingPanel({ onComplete }: OnboardingPanelProps) {
  const [step, setStep] = useState<Step>("permissions");
  const [permissions, setPermissions] = useState<PermissionState>({
    accessibility: false,
    microphone: "not_determined",
  });
  const [micError, setMicError] = useState("");

  const checkPermissions = useCallback(async () => {
    const [ax, voice] = await Promise.all([
      invoke<boolean>("check_ax_permission").catch(() => false),
      invoke<string>("check_voice_permission").catch(() => "not_determined"),
    ]);
    setPermissions({
      accessibility: ax,
      microphone: voice as PermissionState["microphone"],
    });
  }, []);

  // Poll permissions every 2s on permissions step
  useEffect(() => {
    if (step !== "permissions") return;
    checkPermissions();
    const interval = setInterval(checkPermissions, 2000);
    return () => clearInterval(interval);
  }, [step, checkPermissions]);

  const handleComplete = async () => {
    try {
      const settings = await invoke<Settings>("get_settings");
      await invoke("save_settings", {
        settings: { ...settings, onboarding_completed: true },
      });
    } catch {
      /* non-critical */
    }
    onComplete();
    getCurrentWebviewWindow().hide();
  };

  const handleSkip = () => {
    handleComplete();
  };

  const currentIdx = STEPS.indexOf(step);

  const goNext = () => {
    const next = STEPS[currentIdx + 1];
    if (next) setStep(next);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Step indicator */}
      <div className="flex items-center justify-center gap-2 pt-4 pb-2 flex-shrink-0">
        {STEPS.map((s, i) => (
          <div
            key={s}
            className={`w-2 h-2 rounded-full transition-colors ${
              i < currentIdx
                ? "bg-green-400"
                : i === currentIdx
                  ? "bg-violet-500"
                  : "bg-zinc-600"
            }`}
          />
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 px-5 overflow-y-auto">
        {step === "permissions" && (
          <PermissionsStep
            permissions={permissions}
            micError={micError}
            onRequestMic={async () => {
              try {
                await invoke("request_voice_permission");
                setMicError("");
                checkPermissions();
              } catch {
                setMicError("Mic permission requires the built app. Run: cargo tauri build --debug");
              }
            }}
            onOpenAxSettings={() => invoke("open_ax_settings").catch(() => {})}
          />
        )}
        {step === "hotkeys" && <HotkeysStep />}
        {step === "ready" && <ReadyStep />}
      </div>

      {/* Navigation */}
      <div className="flex items-center justify-between px-5 py-3 border-t border-white/[0.06] flex-shrink-0">
        <button
          onClick={handleSkip}
          className="text-[11px] text-zinc-500 hover:text-zinc-300 transition-colors"
        >
          Skip
        </button>
        {step === "ready" ? (
          <button
            onClick={handleComplete}
            className="text-[12px] px-4 py-1.5 rounded-lg bg-violet-600 hover:bg-violet-500 text-white transition-colors"
          >
            Start
          </button>
        ) : (
          <button
            onClick={goNext}
            className="text-[12px] px-4 py-1.5 rounded-lg bg-violet-600 hover:bg-violet-500 text-white transition-colors"
          >
            Next
          </button>
        )}
      </div>
    </div>
  );
}

// --- Sub-steps ---

function PermissionsStep({
  permissions,
  micError,
  onRequestMic,
  onOpenAxSettings,
}: {
  permissions: PermissionState;
  micError: string;
  onRequestMic: () => void;
  onOpenAxSettings: () => void;
}) {
  return (
    <div className="pt-3">
      <h2 className="text-[15px] font-medium mb-1">Permissions</h2>
      <p className="text-[11px] text-zinc-500 mb-4">
        YoYo needs these permissions to work properly.
      </p>

      <div className="space-y-3">
        {/* Accessibility */}
        <div className="flex items-center gap-3 bg-zinc-800/50 rounded-lg px-3 py-2.5">
          <StatusDot granted={permissions.accessibility} />
          <div className="flex-1 min-w-0">
            <p className="text-[12px] text-zinc-200">Accessibility</p>
            <p className="text-[10px] text-zinc-500">
              Read screen content and text
            </p>
          </div>
          {!permissions.accessibility && (
            <button
              onClick={onOpenAxSettings}
              className="text-[11px] px-2.5 py-1 rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-300 transition-colors flex-shrink-0"
            >
              Open Settings
            </button>
          )}
        </div>

        {/* Microphone */}
        <div className="bg-zinc-800/50 rounded-lg px-3 py-2.5">
          <div className="flex items-center gap-3">
            <StatusDot granted={permissions.microphone === "granted"} />
            <div className="flex-1 min-w-0">
              <p className="text-[12px] text-zinc-200">Microphone</p>
              <p className="text-[10px] text-zinc-500">
                Voice input for hands-free use
              </p>
            </div>
            {permissions.microphone !== "granted" && (
              <button
                onClick={onRequestMic}
                className="text-[11px] px-2.5 py-1 rounded bg-zinc-700 hover:bg-zinc-600 text-zinc-300 transition-colors flex-shrink-0"
              >
                Grant
              </button>
            )}
          </div>
          {micError && (
            <p className="text-[10px] text-amber-400/80 mt-1.5 ml-8">
              {micError}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}

function HotkeysStep() {
  return (
    <div className="pt-3">
      <h2 className="text-[15px] font-medium mb-1">Hotkeys</h2>
      <p className="text-[11px] text-zinc-500 mb-4">
        Use these shortcuts to control YoYo from anywhere.
      </p>

      <div className="space-y-3">
        <div className="flex items-center gap-3 bg-zinc-800/50 rounded-lg px-3 py-3">
          <div className="flex gap-1 flex-shrink-0">
            <Kbd>⌘</Kbd>
            <Kbd>Shift</Kbd>
            <Kbd>Y</Kbd>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-[12px] text-zinc-200">Toggle YoYo</p>
            <p className="text-[10px] text-zinc-500">
              Show or hide the input panel
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3 bg-zinc-800/50 rounded-lg px-3 py-3">
          <div className="flex gap-1 flex-shrink-0">
            <Kbd>⌘</Kbd>
            <Kbd>Shift</Kbd>
            <Kbd>R</Kbd>
          </div>
          <div className="flex-1 min-w-0">
            <p className="text-[12px] text-zinc-200">Analyze Screen</p>
            <p className="text-[10px] text-zinc-500">
              Read and understand what's on screen
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

function ReadyStep() {
  return (
    <div className="pt-6 flex flex-col items-center text-center">
      <div className="w-10 h-10 rounded-full bg-violet-500/20 flex items-center justify-center mb-3">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          className="w-5 h-5 text-violet-400"
        >
          <path
            d="M5 13l4 4L19 7"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>
      <h2 className="text-[15px] font-medium mb-2">You're all set!</h2>
      <p className="text-[12px] text-zinc-400 leading-relaxed max-w-[240px]">
        YoYo will watch your screen and suggest helpful actions as you work.
        Click the dot or press{" "}
        <span className="text-zinc-300 font-mono text-[11px]">⌘⇧Y</span> to
        interact.
      </p>
    </div>
  );
}

// --- Shared UI ---

function StatusDot({ granted }: { granted: boolean }) {
  return (
    <div
      className={`w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0 ${
        granted ? "bg-green-500/20" : "bg-amber-500/20"
      }`}
    >
      {granted ? (
        <svg viewBox="0 0 16 16" fill="none" className="w-3 h-3 text-green-400">
          <path
            d="M4 8.5l3 3 5-6"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      ) : (
        <svg viewBox="0 0 16 16" fill="none" className="w-3 h-3 text-amber-400">
          <path
            d="M8 5v3.5M8 11h.01"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </svg>
      )}
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="px-1.5 py-0.5 rounded bg-white/[0.08] text-zinc-400 font-mono text-[10px] border border-white/[0.06]">
      {children}
    </kbd>
  );
}
