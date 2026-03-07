import { useEffect, useState, useRef } from "react";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { StatusIndicator } from "./components/StatusIndicator";
import { SettingsPanel } from "./components/SettingsPanel";
import type { Session, TimelineEntry, SessionSummary } from "./types";
import {
  startSession,
  endSession,
  getActiveSession,
  getSessionHistory,
  getSessionTimeline,
  sendSessionMessage,
} from "./services/sessions";

export default function TrayApp() {
  const [session, setSession] = useState<Session | null>(null);
  const [timeline, setTimeline] = useState<TimelineEntry[]>([]);
  const [history, setHistory] = useState<Session[]>([]);
  const [input, setInput] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [loading, setLoading] = useState(false);
  const [ending, setEnding] = useState(false);
  const [chatInput, setChatInput] = useState("");
  const [chatLoading, setChatLoading] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval>>();
  const [elapsed, setElapsed] = useState("");
  const timelineEndRef = useRef<HTMLDivElement>(null);

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

  // Load active session on mount
  useEffect(() => {
    getActiveSession().then((s) => {
      setSession(s);
      if (s) {
        getSessionTimeline(s.id).then(setTimeline);
      } else {
        getSessionHistory(5).then(setHistory);
      }
    });
  }, []);

  // Timer tick
  useEffect(() => {
    if (!session) {
      clearInterval(timerRef.current);
      setElapsed("");
      return;
    }
    const tick = () => {
      const start = new Date(session.started_at.replace(" ", "T")).getTime();
      const diff = Date.now() - start;
      const h = Math.floor(diff / 3600000);
      const m = Math.floor((diff % 3600000) / 60000);
      setElapsed(h > 0 ? `${h}h${m}m` : `${m}m`);
    };
    tick();
    timerRef.current = setInterval(tick, 10000);
    return () => clearInterval(timerRef.current);
  }, [session]);

  // Listen for timeline updates
  useEffect(() => {
    const u1 = listen<any>("session-timeline-update", (e) => {
      setTimeline((prev) => [
        ...prev,
        {
          id: Date.now(),
          session_id: e.payload.session_id,
          timestamp: new Date().toLocaleTimeString("zh-CN", {
            hour: "2-digit",
            minute: "2-digit",
          }),
          context: e.payload.context,
          app_name: e.payload.app_name,
        },
      ]);
    });
    const u2 = listen<Session>("session-started", (e) => {
      setSession(e.payload);
      setTimeline([]);
    });
    const u3 = listen<SessionSummary>("session-ended", () => {
      setSession(null);
      setTimeline([]);
      getSessionHistory(5).then(setHistory);
    });
    return () => {
      [u1, u2, u3].forEach((u) => u.then((f) => f()));
    };
  }, []);

  // Auto-scroll timeline
  useEffect(() => {
    timelineEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [timeline]);

  const handleStart = async () => {
    if (!input.trim()) return;
    setLoading(true);
    try {
      await startSession(input.trim());
      setInput("");
    } finally {
      setLoading(false);
    }
  };

  const handleEnd = async () => {
    setEnding(true);
    try {
      await endSession();
    } finally {
      setEnding(false);
    }
  };

  const handleChat = async () => {
    if (!chatInput.trim() || chatLoading) return;
    setChatLoading(true);
    try {
      await sendSessionMessage(chatInput.trim());
      setChatInput("");
    } finally {
      setChatLoading(false);
    }
  };

  if (showSettings) {
    return (
      <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
        <SettingsPanel onClose={() => setShowSettings(false)} />
      </div>
    );
  }

  // --- IN-SESSION VIEW ---
  if (session) {
    return (
      <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
        {/* Header: timer + end button */}
        <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
          <div className="flex items-center gap-2">
            <span className="text-green-400 font-mono text-sm">
              ⏱ {elapsed}
            </span>
          </div>
          <button
            onClick={handleEnd}
            disabled={ending}
            className="text-xs px-2 py-1 rounded bg-red-600 hover:bg-red-500 disabled:opacity-50 transition-colors"
          >
            {ending ? "Ending..." : "End"}
          </button>
        </div>
        {/* Goal */}
        <div className="px-3 py-1.5 text-sm text-zinc-300 border-b border-zinc-800 truncate">
          {session.goal}
        </div>

        {/* Timeline (scrollable) */}
        <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2 space-y-2">
          {timeline.map((entry) => (
            <div key={entry.id} className="flex flex-col">
              <div className="flex items-baseline gap-1.5">
                <span className="text-zinc-600 text-[10px] font-mono shrink-0">
                  {entry.timestamp.length > 5
                    ? entry.timestamp.slice(11, 16)
                    : entry.timestamp}
                </span>
                <span className="text-zinc-300 text-xs leading-snug">
                  {entry.context}
                </span>
              </div>
              <span className="text-zinc-600 text-[10px] ml-10">
                {entry.app_name}
              </span>
            </div>
          ))}
          {timeline.length === 0 && (
            <div className="text-zinc-600 text-xs text-center mt-8">
              Session started. Activity will appear here...
            </div>
          )}
          <div ref={timelineEndRef} />
        </div>

        {/* Chat input + status */}
        <div className="px-3 py-2 border-t border-zinc-800 space-y-1.5">
          <div className="flex gap-2">
            <input
              type="text"
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  handleChat();
                }
              }}
              placeholder="Ask YoYo..."
              disabled={chatLoading}
              className="flex-1 bg-zinc-800 text-white text-xs rounded px-2 py-1.5 outline-none focus:ring-1 focus:ring-blue-500 placeholder-zinc-600 disabled:opacity-50"
            />
          </div>
          <StatusIndicator loading={chatLoading} error={null} />
        </div>
      </div>
    );
  }

  // --- IDLE VIEW ---
  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
        <span className="text-sm font-semibold tracking-wide">YoYo</span>
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
      </div>

      {/* Session history */}
      <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2">
        <div className="text-[10px] text-zinc-500 uppercase tracking-wider mb-2">
          Recent Sessions
        </div>
        {history.length === 0 && (
          <div className="text-zinc-600 text-xs text-center mt-8 space-y-1">
            <div>No sessions yet.</div>
            <div>Type a goal below to start!</div>
          </div>
        )}
        {history.map((s) => (
          <button
            key={s.id}
            onClick={() => setInput(s.goal)}
            className="w-full text-left px-2 py-1.5 rounded hover:bg-zinc-800 transition-colors mb-1"
          >
            <div className="text-xs text-zinc-300 truncate">{s.goal}</div>
            <div className="text-[10px] text-zinc-600 flex items-center gap-1.5">
              <span>{s.started_at.slice(0, 10)}</span>
              <span>{s.status === "completed" ? "✓" : "—"}</span>
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
          </button>
        ))}
      </div>

      {/* Input box */}
      <div className="px-3 py-2 border-t border-zinc-800">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleStart();
              }
            }}
            placeholder="Type a goal to start session..."
            disabled={loading}
            className="flex-1 bg-zinc-800 text-white text-sm rounded px-2 py-1.5 outline-none focus:ring-1 focus:ring-blue-500 placeholder-zinc-600 disabled:opacity-50"
          />
          <button
            onClick={handleStart}
            disabled={!input.trim() || loading}
            className="text-xs px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 disabled:opacity-40 transition-colors"
          >
            Go
          </button>
        </div>
      </div>
    </div>
  );
}
