import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { ActionButtons } from "./components/ActionButtons";
import { ChatView } from "./components/ChatView";
import { useActions } from "./hooks/useActions";
import {
  checkNeedsOnboarding,
  startOnboarding,
  sendOnboardingMessage,
} from "./services/onboarding";
import {
  startSession,
  endSession,
  getActiveSession,
  getSessionTimeline,
  sendSessionMessage,
} from "./services/sessions";
import type {
  AnalysisResult,
  ChatMessage,
  Session,
  Settings,
  SuggestedAction,
  TimelineEntry,
} from "./types";

type BubbleMode = "normal" | "chat";

export default function BubbleApp() {
  const [mode, setMode] = useState<BubbleMode>("normal");
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [opacity, setOpacity] = useState(0.85);
  const [visible, setVisible] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [analysisStage, setAnalysisStage] = useState<string | null>(null);
  const [actionDone, setActionDone] = useState(false);
  const { executing, execute } = useActions();

  // Chat state (onboarding)
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [chatLoading, setChatLoading] = useState(false);

  // Session state
  const [session, setSession] = useState<Session | null>(null);
  const [timeline, setTimeline] = useState<TimelineEntry[]>([]);
  const [goalInput, setGoalInput] = useState("");
  const [startingSession, setStartingSession] = useState(false);
  const [endingSession, setEndingSession] = useState(false);
  const [chatInput, setChatInput] = useState("");
  const [sessionChatLoading, setSessionChatLoading] = useState(false);

  // Timer
  const [elapsed, setElapsed] = useState("");
  const timerRef = useRef<ReturnType<typeof setInterval>>();

  // Dynamic window resize — measure the rendered bubble element via getBoundingClientRect.
  // This is unaffected by body overflow:hidden (unlike scrollHeight).
  const bubbleRef = useRef<HTMLDivElement>(null);
  const lastHeight = useRef(0);
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      if (!bubbleRef.current) return;
      const h = Math.ceil(bubbleRef.current.getBoundingClientRect().height);
      const clamped = Math.min(Math.max(h, 80), 520);
      if (clamped === lastHeight.current) return;
      lastHeight.current = clamped;
      getCurrentWebviewWindow()
        .setSize(new LogicalSize(340, clamped))
        .catch(() => {});
    });
    return () => cancelAnimationFrame(frame);
  });

  useEffect(() => {
    // Load opacity setting
    invoke<Settings>("get_settings").then((s) => {
      if (s.bubble_opacity !== undefined) setOpacity(s.bubble_opacity);
    });

    // Load active session
    getActiveSession().then((s) => {
      setSession(s);
      if (s) {
        getSessionTimeline(s.id).then(setTimeline);
        setVisible(true);
      }
    });

    // Check if onboarding is needed
    checkNeedsOnboarding()
      .then((needed) => {
        if (needed) {
          setMode("chat");
          setVisible(true);
          setChatLoading(true);
          startOnboarding()
            .then((msg) => setChatMessages([msg]))
            .catch(console.error)
            .finally(() => setChatLoading(false));
        } else {
          invoke<AnalysisResult | null>("get_last_analysis").then((cached) => {
            if (cached) {
              setResult(cached);
              setVisible(true);
            }
          });
        }
      })
      .catch(console.error);

    const unlistenSwitch = listen("app-switched", () => {
      setRefreshing(true);
      setAnalysisStage(null);
    });

    const unlistenProgress = listen<string>("analysis-progress", (event) => {
      setAnalysisStage(event.payload);
    });

    const unlistenAnalysis = listen<AnalysisResult>(
      "analysis-complete",
      (event) => {
        setResult(event.payload);
        setVisible(true);
        setRefreshing(false);
        setAnalysisStage(null);
      }
    );

    const unlistenOpacity = listen<number>(
      "bubble-opacity-changed",
      (event) => {
        setOpacity(event.payload);
      }
    );

    const unlistenOnboarding = listen("onboarding-complete", () => {
      setMode("normal");
    });

    // Session events
    const unlistenSessionStart = listen<Session>(
      "session-started",
      (event) => {
        setSession(event.payload);
        setTimeline([]);
        setVisible(true);
      }
    );

    const unlistenSessionEnd = listen("session-ended", () => {
      setSession(null);
      setTimeline([]);
    });

    const unlistenTimeline = listen<any>(
      "session-timeline-update",
      () => {
        // Re-fetch from DB to stay in sync (avoids stale/duplicate entries)
        getActiveSession().then((s) => {
          if (s) getSessionTimeline(s.id).then(setTimeline);
        });
      }
    );

    return () => {
      [
        unlistenSwitch,
        unlistenProgress,
        unlistenAnalysis,
        unlistenOpacity,
        unlistenOnboarding,
        unlistenSessionStart,
        unlistenSessionEnd,
        unlistenTimeline,
      ].forEach((u) => u.then((fn) => fn()));
    };
  }, []);

  // Session timer
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

  const handleExecute = async (action: SuggestedAction) => {
    await execute(action);
    setActionDone(true);
    setTimeout(() => setActionDone(false), 1200);
  };

  const handleChatSend = async (text: string) => {
    setChatMessages((prev) => [...prev, { role: "user", content: text }]);
    setChatLoading(true);
    try {
      const reply = await sendOnboardingMessage(text);
      setChatMessages((prev) => [...prev, reply]);
    } catch (e) {
      setChatMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${e}` },
      ]);
    } finally {
      setChatLoading(false);
    }
  };

  const handleStartSession = async () => {
    if (!goalInput.trim() || startingSession) return;
    setStartingSession(true);
    try {
      await startSession(goalInput.trim());
      setGoalInput("");
    } finally {
      setStartingSession(false);
    }
  };

  const handleEndSession = async () => {
    setEndingSession(true);
    try {
      await endSession();
    } finally {
      setEndingSession(false);
    }
  };

  const handleSessionChat = async () => {
    if (!chatInput.trim() || sessionChatLoading) return;
    setSessionChatLoading(true);
    try {
      await sendSessionMessage(chatInput.trim());
      setChatInput("");
    } finally {
      setSessionChatLoading(false);
    }
  };

  // Chat mode (onboarding)
  if (mode === "chat") {
    return (
      <div className="bubble-container bubble-enter" style={{ opacity }}>
        <div
          className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
          shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
          text-white select-none overflow-hidden min-h-[300px] max-h-[460px] flex flex-col"
        >
          <div className="flex items-center justify-between px-4 pt-3 pb-2 flex-shrink-0">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-blue-400 shadow-[0_0_6px_rgba(96,165,250,0.5)]" />
              <span className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
                YoYo Setup
              </span>
            </div>
          </div>
          <div className="flex-1 min-h-0">
            <ChatView
              messages={chatMessages}
              loading={chatLoading}
              onSend={handleChatSend}
            />
          </div>
        </div>
      </div>
    );
  }

  // --- MAIN BUBBLE VIEW ---
  return (
    <div
      className={`bubble-container ${visible ? "bubble-enter" : "bubble-exit"}`}
      style={{ opacity }}
    >
      <div
        ref={bubbleRef}
        className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
        shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
        text-white select-none"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 pt-3 pb-2">
          <div className="flex items-center gap-2">
            {refreshing ? (
              <span className="w-2 h-2 border border-zinc-400 border-t-transparent rounded-full animate-spin" />
            ) : session ? (
              <div className="w-2 h-2 rounded-full bg-blue-400 shadow-[0_0_6px_rgba(96,165,250,0.5)]" />
            ) : (
              <div className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]" />
            )}
            <span className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
              {refreshing && analysisStage
                ? analysisStage
                : session
                  ? `Session · ${elapsed}`
                  : "YoYo"}
            </span>
          </div>
          {/* End session button */}
          {session && (
            <button
              onClick={handleEndSession}
              disabled={endingSession}
              className="text-[10px] px-1.5 py-0.5 rounded bg-red-600/80 hover:bg-red-500 disabled:opacity-50 transition-colors"
            >
              {endingSession ? "..." : "End"}
            </button>
          )}
        </div>

        {/* Content area — normal flow, no flex collapse */}
        <div>
          {/* Session goal banner */}
          {session && (
            <div className="mx-4 mb-2 px-2.5 py-1.5 bg-blue-500/[0.06] border border-blue-500/15 rounded-lg">
              <div className="text-[11px] text-zinc-300 truncate">
                {session.goal}
              </div>
            </div>
          )}

          {/* Context (from analysis) */}
          {result && (
            <div className="px-4 pb-2">
              <p className="text-[13px] text-zinc-300 leading-snug">
                {result.context}
              </p>
            </div>
          )}

          {/* Session timeline (compact, last 3) */}
          {session && timeline.length > 0 && (
            <div className="mx-4 mb-2 space-y-0.5">
              {timeline.slice(-3).map((e) => (
                <div
                  key={e.id}
                  className="text-[10px] text-zinc-500 truncate pl-1"
                >
                  <span className="text-zinc-600 font-mono">
                    {e.timestamp.slice(11, 16)}
                  </span>{" "}
                  {e.context}
                </div>
              ))}
            </div>
          )}

          {/* Key Concepts (Learning mode) */}
          {result?.key_concepts && result.key_concepts.length > 0 && (
            <div className="mx-4 mb-2">
              <div className="flex flex-wrap gap-1">
                {result.key_concepts.map((concept, i) => (
                  <span
                    key={i}
                    className="px-1.5 py-0.5 text-[10px] bg-violet-500/10 text-violet-300
                      border border-violet-500/20 rounded"
                  >
                    {concept}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Separator */}
          {(result || session) && (
            <div className="mx-4 border-t border-white/[0.06]" />
          )}

          {/* Actions or status overlay */}
          {executing || actionDone ? (
            <div className="px-4 py-6 flex flex-col items-center gap-2">
              {actionDone ? (
                <>
                  <svg
                    viewBox="0 0 24 24"
                    fill="none"
                    className="w-6 h-6 text-emerald-400"
                  >
                    <path
                      d="M5 13l4 4L19 7"
                      stroke="currentColor"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                  <span className="text-[12px] text-zinc-400">Done</span>
                </>
              ) : (
                <>
                  <span className="w-5 h-5 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
                  <span className="text-[12px] text-zinc-400">
                    Processing...
                  </span>
                </>
              )}
            </div>
          ) : (
            <>
              {/* Action buttons */}
              {result && (
                <ActionButtons
                  actions={result.actions}
                  executing={executing}
                  onExecute={handleExecute}
                  compact
                />
              )}
            </>
          )}
        </div>

        {/* Bottom area */}
        <div>
          {/* Session chat input (during active session) */}
          {session && (
            <div className="mx-4 mb-2">
              <input
                type="text"
                value={chatInput}
                onChange={(e) => setChatInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                    e.preventDefault();
                    handleSessionChat();
                  }
                }}
                placeholder="Ask YoYo..."
                disabled={sessionChatLoading}
                className="w-full bg-white/[0.06] text-white text-[11px] rounded px-2 py-1.5 outline-none focus:ring-1 focus:ring-blue-500/50 placeholder-zinc-600 disabled:opacity-50 border border-white/[0.06]"
              />
            </div>
          )}

          {/* Session start input (when idle — no active session) */}
          {!session && (
            <div className="mx-4 my-2">
              <div className="flex gap-1.5">
                <input
                  type="text"
                  value={goalInput}
                  onChange={(e) => setGoalInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.nativeEvent.isComposing) {
                      e.preventDefault();
                      handleStartSession();
                    }
                  }}
                  placeholder="Start a session..."
                  disabled={startingSession}
                  className="flex-1 bg-white/[0.06] text-white text-[11px] rounded px-2 py-1.5 outline-none focus:ring-1 focus:ring-blue-500/50 placeholder-zinc-600 disabled:opacity-50 border border-white/[0.06]"
                />
                <button
                  onClick={handleStartSession}
                  disabled={!goalInput.trim() || startingSession}
                  className="text-[10px] px-2 py-1.5 rounded bg-blue-600/80 hover:bg-blue-500 disabled:opacity-30 transition-colors"
                >
                  Go
                </button>
              </div>
            </div>
          )}

          {/* Footer */}
          <div className="px-4 py-2 flex items-center border-t border-white/[0.06]">
            <span className="text-[10px] text-zinc-600">
              <kbd className="px-1 py-0.5 rounded bg-white/[0.06] text-zinc-500 font-mono text-[9px]">
                Cmd+Shift+R
              </kbd>{" "}
              refresh
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
