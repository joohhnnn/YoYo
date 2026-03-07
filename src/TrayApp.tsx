import { useEffect, useState } from "react";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { StatusIndicator } from "./components/StatusIndicator";
import { SettingsPanel } from "./components/SettingsPanel";
import type { Session, SessionSummary } from "./types";
import { getActiveSession, getSessionHistory } from "./services/sessions";

export default function TrayApp() {
  const [session, setSession] = useState<Session | null>(null);
  const [history, setHistory] = useState<Session[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [loading, setLoading] = useState(false);

  // Register global shortcuts
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

  // Load state on mount
  useEffect(() => {
    getActiveSession().then(setSession);
    getSessionHistory(10).then(setHistory);
  }, []);

  // Listen for session events
  useEffect(() => {
    const u1 = listen<Session>("session-started", (e) => {
      setSession(e.payload);
    });
    const u2 = listen<SessionSummary>("session-ended", () => {
      setSession(null);
      getSessionHistory(10).then(setHistory);
    });
    return () => {
      [u1, u2].forEach((u) => u.then((f) => f()));
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
            onClick={() => {
              setLoading(true);
              invoke("analyze_screen")
                .catch(console.error)
                .finally(() => setLoading(false));
            }}
            disabled={loading}
            className="px-2.5 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded
              disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {loading ? "..." : "Analyze"}
          </button>
        </div>
      </div>

      {/* Current session indicator */}
      {session && (
        <div className="mx-3 mt-2 px-2.5 py-1.5 bg-blue-500/[0.06] border border-blue-500/15 rounded-lg">
          <div className="text-[10px] text-blue-400 uppercase tracking-wider mb-0.5">
            Active Session
          </div>
          <div className="text-xs text-zinc-300 truncate">{session.goal}</div>
        </div>
      )}

      {/* Session history */}
      <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2">
        <div className="text-[10px] text-zinc-500 uppercase tracking-wider mb-2">
          Session History
        </div>
        {history.length === 0 && (
          <div className="text-zinc-600 text-xs text-center mt-4">
            No sessions yet. Use the floating bubble to start one.
          </div>
        )}
        {history.map((s) => (
          <div
            key={s.id}
            className="px-2 py-1.5 rounded mb-1 bg-zinc-800/30"
          >
            <div className="text-xs text-zinc-300 truncate">{s.goal}</div>
            <div className="text-[10px] text-zinc-600 flex items-center gap-1.5">
              <span>{s.started_at.slice(0, 10)}</span>
              <span>{s.status === "completed" ? "✓" : s.status === "active" ? "●" : "—"}</span>
              {s.ended_at &&
                s.started_at &&
                (() => {
                  const ms =
                    new Date(s.ended_at.replace(" ", "T")).getTime() -
                    new Date(s.started_at.replace(" ", "T")).getTime();
                  const m = Math.floor(ms / 60000);
                  return (
                    <span>
                      {m >= 60
                        ? `${Math.floor(m / 60)}h${m % 60}m`
                        : `${m}m`}
                    </span>
                  );
                })()}
            </div>
            {s.summary && (
              <div className="text-[10px] text-zinc-500 mt-1 line-clamp-2">
                {s.summary}
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Footer status */}
      <div className="px-3 py-1.5 border-t border-zinc-800">
        <StatusIndicator loading={loading} error={null} />
      </div>
    </div>
  );
}
