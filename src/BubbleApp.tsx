import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { ActionButtons } from "./components/ActionButtons";
import { ChatView } from "./components/ChatView";
import { useActions } from "./hooks/useActions";
import {
  checkNeedsOnboarding,
  startOnboarding,
  sendOnboardingMessage,
} from "./services/onboarding";
import type { AnalysisResult, ChatMessage, Settings, SuggestedAction, TaskItem } from "./types";

type BubbleMode = "normal" | "chat";

export default function BubbleApp() {
  const [mode, setMode] = useState<BubbleMode>("normal");
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [opacity, setOpacity] = useState(0.85);
  const [visible, setVisible] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [actionDone, setActionDone] = useState(false);
  const { executing, execute } = useActions();

  // Chat state
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>([]);
  const [chatLoading, setChatLoading] = useState(false);

  // Main quest
  const [mainQuest, setMainQuest] = useState<TaskItem | null>(null);

  useEffect(() => {
    // Load opacity setting
    invoke<Settings>("get_settings").then((s) => {
      if (s.bubble_opacity !== undefined) setOpacity(s.bubble_opacity);
    });

    // Load main quest
    invoke<TaskItem[]>("get_tasks").then((tasks) => {
      const main = tasks.find((t) => t.quest_type === "main" && !t.done);
      setMainQuest(main ?? null);
    });

    // Check if onboarding is needed
    checkNeedsOnboarding()
      .then((needed) => {
        if (needed) {
          setMode("chat");
          setVisible(true);
          setChatLoading(true);
          startOnboarding()
            .then((msg) => {
              setChatMessages([msg]);
            })
            .catch(console.error)
            .finally(() => setChatLoading(false));
        } else {
          // On mount, fetch cached result
          invoke<AnalysisResult | null>("get_last_analysis").then((cached) => {
            if (cached) {
              setResult(cached);
              setVisible(true);
            }
          });
        }
      })
      .catch(console.error);

    // Show refreshing indicator on app switch; content updates when analysis completes
    const unlistenSwitch = listen("app-switched", () => {
      setRefreshing(true);
    });

    // Listen for new analysis results — keeps bubble content in sync on app switch
    const unlistenAnalysis = listen<AnalysisResult>(
      "analysis-complete",
      (event) => {
        setResult(event.payload);
        setVisible(true);
        setRefreshing(false);
      }
    );

    // Listen for opacity changes from tray panel
    const unlistenOpacity = listen<number>("bubble-opacity-changed", (event) => {
      setOpacity(event.payload);
    });

    // Listen for onboarding completion
    const unlistenOnboarding = listen("onboarding-complete", () => {
      setMode("normal");
    });

    // Refresh main quest when tasks change
    const unlistenTasks = listen("tasks-changed", () => {
      invoke<TaskItem[]>("get_tasks").then((tasks) => {
        const main = tasks.find((t) => t.quest_type === "main" && !t.done);
        setMainQuest(main ?? null);
      });
    });

    return () => {
      unlistenSwitch.then((fn) => fn());
      unlistenAnalysis.then((fn) => fn());
      unlistenOpacity.then((fn) => fn());
      unlistenOnboarding.then((fn) => fn());
      unlistenTasks.then((fn) => fn());
    };
  }, []);

  const handleExecute = async (action: SuggestedAction) => {
    await execute(action);
    setActionDone(true);
    setTimeout(() => {
      setActionDone(false);
    }, 1200);
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

  // Chat mode: always show
  if (mode === "chat") {
    return (
      <div
        className="bubble-container bubble-enter"
        style={{ opacity }}
      >
        <div className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
          shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
          text-white select-none overflow-hidden h-[370px] flex flex-col">
          {/* Header */}
          <div className="flex items-center justify-between px-4 pt-3 pb-2 flex-shrink-0">
            <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-blue-400 shadow-[0_0_6px_rgba(96,165,250,0.5)]" />
              <span className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
                YoYo Setup
              </span>
            </div>
          </div>

          {/* Chat content */}
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

  // Normal mode: need result
  if (!result) {
    return null;
  }

  return (
    <div
      className={`bubble-container ${visible ? "bubble-enter" : "bubble-exit"}`}
      style={{ opacity }}
    >
      <div className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
        shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
        text-white select-none overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 pt-3 pb-2">
          <div className="flex items-center gap-2">
            {refreshing ? (
              <span className="w-2 h-2 border border-zinc-400 border-t-transparent rounded-full animate-spin" />
            ) : (
              <div className="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]" />
            )}
            <span className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
              YoYo
            </span>
          </div>
        </div>

        {/* Context */}
        <div className="px-4 pb-2">
          <p className="text-[13px] text-zinc-300 leading-snug">{result.context}</p>
        </div>

        {/* Main Quest Tracker */}
        {mainQuest && (
          <div className="mx-4 px-2.5 py-1.5 mb-2 bg-amber-500/[0.06] border border-amber-500/15 rounded-lg">
            <div className="flex items-center gap-1.5">
              <svg viewBox="0 0 12 12" className="w-3 h-3 text-amber-500 flex-shrink-0" fill="currentColor">
                <path d="M6 1l1.5 3.2L11 4.7 8.5 7.1l.6 3.4L6 8.8 2.9 10.5l.6-3.4L1 4.7l3.5-.5z" />
              </svg>
              <span className="text-[11px] text-zinc-300 flex-1 truncate">{mainQuest.text}</span>
              {mainQuest.target !== undefined && (
                <span className="text-[10px] text-amber-400 tabular-nums flex-shrink-0">
                  {mainQuest.progress ?? 0}/{mainQuest.target}
                </span>
              )}
            </div>
            {mainQuest.target !== undefined && (
              <div className="mt-1 h-1 bg-black/30 rounded-full overflow-hidden">
                <div
                  className="h-full bg-amber-500/70 rounded-full transition-all"
                  style={{
                    width: `${Math.min(100, ((mainQuest.progress ?? 0) / mainQuest.target) * 100)}%`,
                  }}
                />
              </div>
            )}
          </div>
        )}

        {/* Separator */}
        <div className="mx-4 border-t border-white/[0.06]" />

        {/* Actions or status overlay */}
        {executing || actionDone ? (
          <div className="px-4 py-6 flex flex-col items-center gap-2">
            {actionDone ? (
              <>
                <svg viewBox="0 0 24 24" fill="none" className="w-6 h-6 text-emerald-400">
                  <path d="M5 13l4 4L19 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"/>
                </svg>
                <span className="text-[12px] text-zinc-400">Done</span>
              </>
            ) : (
              <>
                <span className="w-5 h-5 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
                <span className="text-[12px] text-zinc-400">Processing...</span>
              </>
            )}
          </div>
        ) : (
          <>
            <ActionButtons
              actions={result.actions}
              executing={executing}
              onExecute={handleExecute}
              compact
            />

            {/* Footer */}
            <div className="px-4 py-2 flex items-center border-t border-white/[0.06]">
              <span className="text-[10px] text-zinc-600">
                <kbd className="px-1 py-0.5 rounded bg-white/[0.06] text-zinc-500 font-mono text-[9px]">
                  Cmd+Shift+R
                </kbd>
                {" "}refresh
              </span>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
