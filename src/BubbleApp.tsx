import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { register } from "@tauri-apps/plugin-global-shortcut";
import { ActionButtons } from "./components/ActionButtons";
import { useActions } from "./hooks/useActions";
import { useWindowAutoResize } from "./hooks/useWindowAutoResize";
import type { AnalysisResult, AppSwitchEvent, BubbleState, IntentResult, KnowledgeMetadata, KnowledgeRecord, PlanStep, Settings, SuggestedAction } from "./types";

export default function BubbleApp() {
  const [state, setState] = useState<BubbleState>("ambient");
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [opacity, setOpacity] = useState(0.85);
  const [analysisStage, setAnalysisStage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [appInfo, setAppInfo] = useState({ name: "", title: "" });
  const [actionDone, setActionDone] = useState(false);
  const [intentResult, setIntentResult] = useState<IntentResult | null>(null);
  const [executingPlan, setExecutingPlan] = useState(false);
  const [currentStep, setCurrentStep] = useState(-1);
  const [inputValue, setInputValue] = useState("");
  const [failedStep, setFailedStep] = useState(-1);
  const [isRecording, setIsRecording] = useState(false);
  const [recordingTime, setRecordingTime] = useState(0);
  const [executionId, setExecutionId] = useState<number | null>(null);
  const [showTeachMe, setShowTeachMe] = useState(false);
  const [workflowName, setWorkflowName] = useState("");
  const [feedbackGiven, setFeedbackGiven] = useState(false);
  const [hasNudge, setHasNudge] = useState(false);
  const [nudgeItem, setNudgeItem] = useState<KnowledgeRecord | null>(null);
  const [showQuiz, setShowQuiz] = useState(false);
  const [showAnswer, setShowAnswer] = useState(false);
  const { executing, execute } = useActions();
  const inputRef = useRef<HTMLInputElement>(null);
  const dismissTimer = useRef<ReturnType<typeof setTimeout>>();
  const prevState = useRef<BubbleState>("ambient");

  // Dynamic window resize based on state
  const { bubbleRef, contentRef } = useWindowAutoResize(state);

  // Register global shortcuts
  useEffect(() => {
    const registerShortcuts = async () => {
      try {
        await register("CmdOrCtrl+Shift+Y", (event) => {
          if (event.state === "Released") return;
          setState((s) => (s === "ambient" ? "active" : "ambient"));
        });
      } catch (e) {
        console.warn("Failed to register toggle shortcut:", e);
      }

      try {
        await register("CmdOrCtrl+Shift+R", (event) => {
          if (event.state === "Released") return;
          triggerAnalysis();
        });
      } catch (e) {
        console.warn("Failed to register analyze shortcut:", e);
      }
    };
    registerShortcuts();
  }, []);

  // Load settings + cached result on mount
  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      if (s.bubble_opacity !== undefined) setOpacity(s.bubble_opacity);
    });

    invoke<AnalysisResult | null>("get_last_analysis").then((cached) => {
      if (cached) {
        setResult(cached);
      }
    });
  }, []);

  // Event listeners
  useEffect(() => {
    const unlistenSwitch = listen<AppSwitchEvent>("app-switched", (event) => {
      setAppInfo({
        name: event.payload.app_name,
        title: "",
      });
      // Auto-analysis is handled by Rust; show working state
      setState((s) => {
        if (s === "working") return s;
        return "working";
      });
      setAnalysisStage(null);
      setError(null);
    });

    const unlistenProgress = listen<string>("analysis-progress", (event) => {
      setAnalysisStage(event.payload);
    });

    const unlistenAnalysis = listen<AnalysisResult>("analysis-complete", (event) => {
      setResult(event.payload);
      setState("done");
      setAnalysisStage(null);
      setError(null);
      startDismissTimer();
    });

    const unlistenIntent = listen<IntentResult>("intent-complete", (event) => {
      setIntentResult(event.payload);
      setResult(null);
      setState("done");
      setAnalysisStage(null);
      setError(null);
      if (!event.payload.needs_confirmation) {
        executePlan(event.payload.plan);
      }
    });

    const unlistenOpacity = listen<number>("bubble-opacity-changed", (event) => {
      setOpacity(event.payload);
    });

    const unlistenNudge = listen<number>("nudge-available", async () => {
      try {
        const due = await invoke<KnowledgeRecord[]>("get_due_knowledge", { limit: 1 });
        if (due.length > 0) {
          setNudgeItem(due[0]);
          setHasNudge(true);
        }
      } catch { /* ignore */ }
    });

    // Check for due knowledge on mount
    invoke<KnowledgeRecord[]>("get_due_knowledge", { limit: 1 })
      .then((due) => { if (due.length > 0) { setNudgeItem(due[0]); setHasNudge(true); } })
      .catch(() => {});

    return () => {
      [unlistenSwitch, unlistenProgress, unlistenAnalysis, unlistenIntent, unlistenOpacity, unlistenNudge].forEach(
        (u) => u.then((fn) => fn())
      );
    };
  }, []);

  // Focus input when entering active state
  useEffect(() => {
    if (state === "active") {
      setTimeout(() => inputRef.current?.focus(), 100);
    }
    prevState.current = state;
  }, [state]);

  // Global keyboard handler for Esc
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && state !== "ambient") {
        goAmbient();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [state]);

  // Recording timer
  useEffect(() => {
    if (!isRecording) return;
    setRecordingTime(0);
    const interval = setInterval(() => setRecordingTime((t) => t + 1), 1000);
    return () => clearInterval(interval);
  }, [isRecording]);

  const goAmbient = () => {
    setState("ambient");
    setInputValue("");
    setIntentResult(null);
    setExecutingPlan(false);
    setCurrentStep(-1);
    setFailedStep(-1);
    setIsRecording(false);
    setRecordingTime(0);
    setExecutionId(null);
    setShowTeachMe(false);
    setWorkflowName("");
    setFeedbackGiven(false);
    setShowQuiz(false);
    setShowAnswer(false);
    setError(null);
    clearDismissTimer();
  };

  const handleCancel = async () => {
    await invoke("cancel_execution").catch(() => {});
    if (executionId) {
      await invoke("complete_execution", { id: executionId, status: "cancelled" }).catch(() => {});
    }
    goAmbient();
  };

  const triggerAnalysis = () => {
    setState("working");
    setAnalysisStage(null);
    setError(null);
    setIntentResult(null);
    clearDismissTimer();
    invoke("analyze_screen").catch((e) => {
      setError(String(e));
      setState("active");
    });
  };

  const triggerIntent = (userInput: string) => {
    setState("working");
    setAnalysisStage("Understanding...");
    setError(null);
    setIntentResult(null);
    clearDismissTimer();
    invoke<IntentResult>("understand_intent", { userInput })
      .then((res) => {
        setIntentResult(res);
        setResult(null);
        setState("done");
        setAnalysisStage(null);
        if (!res.needs_confirmation) {
          executePlan(res.plan);
        }
      })
      .catch((e) => {
        setError(String(e));
        setState("active");
      });
  };

  const handleStartRecording = async () => {
    try {
      const permission = await invoke<string>("check_voice_permission");
      if (permission === "not_determined") {
        const granted = await invoke<boolean>("request_voice_permission");
        if (!granted) { setError("Microphone permission denied"); return; }
      } else if (permission !== "granted") {
        setError("Microphone permission denied"); return;
      }
      await invoke("start_recording");
      setIsRecording(true);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleStopRecording = async () => {
    setIsRecording(false);
    setAnalysisStage("Transcribing...");
    try {
      const text = await invoke<string>("stop_and_transcribe");
      setAnalysisStage(null);
      if (text.trim()) {
        setInputValue(text.trim());
      }
    } catch (e) {
      setError(String(e));
      setAnalysisStage(null);
    }
  };

  const handleSubmit = () => {
    const text = inputValue.trim();
    if (!text) {
      triggerAnalysis();
      return;
    }
    triggerIntent(text);
  };

  const executePlan = async (steps: PlanStep[], startFrom = 0) => {
    await invoke("start_execution"); // reset abort flag
    setExecutingPlan(true);
    setFailedStep(-1);
    setError(null);

    // Record execution (only for fresh starts)
    let execId = executionId;
    if (startFrom === 0) {
      try {
        execId = await invoke<number>("record_execution", {
          inputText: inputValue,
          planJson: JSON.stringify(steps),
          workflowId: intentResult?.workflow_id ?? null,
        });
        setExecutionId(execId);
      } catch { /* non-critical */ }
    }

    for (let i = startFrom; i < steps.length; i++) {
      setCurrentStep(i);
      try {
        await invoke("execute_action", {
          actionType: steps[i].action_type,
          params: steps[i].params,
        });
      } catch (e) {
        setError(`Step ${i + 1} failed: ${e}`);
        setFailedStep(i);
        setExecutingPlan(false);
        setCurrentStep(-1);
        if (execId) {
          invoke("complete_execution", { id: execId, status: "failed", resultJson: String(e) }).catch(() => {});
        }
        if (intentResult?.workflow_id) {
          invoke("update_workflow_count", { id: intentResult.workflow_id, success: false }).catch(() => {});
        }
        return;
      }
    }
    setExecutingPlan(false);
    setCurrentStep(-1);
    setActionDone(true);
    setTimeout(() => setActionDone(false), 1200);
    if (execId) {
      invoke("complete_execution", { id: execId, status: "success" }).catch(() => {});
    }
    if (intentResult?.workflow_id) {
      invoke("update_workflow_count", { id: intentResult.workflow_id, success: true }).catch(() => {});
    }
    startDismissTimer();
  };

  const handleSaveWorkflow = async () => {
    if (!intentResult || !workflowName.trim()) return;
    try {
      await invoke("save_workflow", {
        name: workflowName.trim(),
        triggerContext: inputValue || intentResult.understanding,
        stepsJson: JSON.stringify(intentResult.plan),
      });
      setShowTeachMe(false);
      setWorkflowName("");
    } catch (e) {
      setError(String(e));
    }
  };

  const handleFeedback = async (feedback: string) => {
    if (!executionId || feedbackGiven) return;
    setFeedbackGiven(true);
    await invoke("feedback_execution", { id: executionId, feedback }).catch(() => {});
  };

  const handleOpenQuiz = () => {
    setShowQuiz(true);
    setShowAnswer(false);
    setState("active");
    clearDismissTimer();
  };

  const handleReviewKnowledge = async (success: boolean) => {
    if (!nudgeItem) return;
    try {
      await invoke("review_knowledge", { id: nudgeItem.id, success });
    } catch { /* ignore */ }
    setShowQuiz(false);
    setShowAnswer(false);
    setHasNudge(false);
    setNudgeItem(null);
    goAmbient();
  };

  const handleDismissNudge = () => {
    setShowQuiz(false);
    setShowAnswer(false);
    setHasNudge(false);
  };

  const handleExecute = async (action: SuggestedAction) => {
    await execute(action);
    setActionDone(true);
    setTimeout(() => setActionDone(false), 1200);
  };

  const startDismissTimer = () => {
    clearDismissTimer();
    dismissTimer.current = setTimeout(() => {
      goAmbient();
    }, 15000);
  };

  const clearDismissTimer = () => {
    if (dismissTimer.current) {
      clearTimeout(dismissTimer.current);
      dismissTimer.current = undefined;
    }
  };

  // --- Render ---

  if (state === "ambient") {
    return (
      <div
        className="w-12 h-12 flex items-center justify-center cursor-pointer relative"
        onClick={() => hasNudge ? handleOpenQuiz() : setState("active")}
        title="YoYo"
      >
        <div
          className="dot-breathing w-3 h-3 rounded-full bg-violet-500
            shadow-[0_0_12px_rgba(139,92,246,0.6)]"
        />
        {hasNudge && (
          <div className="absolute top-1.5 right-1.5 w-2 h-2 rounded-full bg-blue-400
            shadow-[0_0_6px_rgba(96,165,250,0.6)] animate-pulse" />
        )}
      </div>
    );
  }

  // Expanded states share the same glass container
  const animClass = prevState.current === "ambient" ? "bubble-expand" : "";

  return (
    <div className={animClass} style={{ opacity }}>
      <div
        ref={bubbleRef}
        className="backdrop-blur-xl bg-black/70 rounded-2xl border border-white/[0.08]
          shadow-[0_8px_32px_rgba(0,0,0,0.5),0_0_0_1px_rgba(255,255,255,0.05)]
          text-white select-none overflow-hidden flex flex-col max-h-screen"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-3 pt-2.5 pb-1.5 flex-shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            {state === "working" ? (
              <span className="w-2 h-2 border border-violet-400 border-t-transparent rounded-full animate-spin flex-shrink-0" />
            ) : (
              <div className="w-2 h-2 rounded-full bg-violet-400 shadow-[0_0_6px_rgba(139,92,246,0.5)] flex-shrink-0" />
            )}
            <span className="text-[11px] text-zinc-400 truncate">
              {state === "working" && analysisStage
                ? analysisStage
                : appInfo.name || "YoYo"}
            </span>
          </div>
          <div className="flex items-center gap-1.5 flex-shrink-0">
            {state === "working" && (
              <button
                onClick={handleCancel}
                className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors"
              >
                Cancel
              </button>
            )}
            {state === "done" && (
              <button
                onClick={goAmbient}
                className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors"
              >
                Dismiss
              </button>
            )}
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 min-h-0 overflow-y-auto">
          <div ref={contentRef}>
            {/* Error message */}
            {error && (state === "active" || state === "done") && (
              <div className="px-3 pb-2">
                <p className="text-[12px] text-red-400 bg-red-950/30 rounded px-2 py-1.5">
                  {error}
                </p>
              </div>
            )}

            {/* Active state: VocabQuiz card */}
            {state === "active" && showQuiz && nudgeItem && (() => {
              const meta: KnowledgeMetadata = JSON.parse(nudgeItem.metadata || "{}");
              return (
                <div className="px-3 pb-3">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-[10px] text-blue-400/70 bg-blue-500/10 px-1.5 py-0.5 rounded">
                      {nudgeItem.kind === "vocab" ? "Vocabulary" : nudgeItem.kind === "concept" ? "Concept" : "Reading"}
                    </span>
                    <button onClick={handleDismissNudge} className="text-[10px] text-zinc-600 hover:text-zinc-400">
                      Skip
                    </button>
                  </div>
                  <p className="text-[14px] text-white font-medium mb-1">{nudgeItem.content}</p>
                  <p className="text-[10px] text-zinc-600 mb-3">from {nudgeItem.source}</p>
                  {showAnswer ? (
                    <>
                      <div className="bg-zinc-800/50 rounded-lg px-3 py-2 mb-3">
                        <p className="text-[12px] text-zinc-300 leading-snug">
                          {meta.definition || "No details available"}
                        </p>
                      </div>
                      <div className="flex gap-2">
                        <button onClick={() => handleReviewKnowledge(true)}
                          className="flex-1 text-[12px] px-3 py-1.5 rounded-lg bg-green-600/80 hover:bg-green-500 text-white transition-colors">
                          Got it
                        </button>
                        <button onClick={() => handleReviewKnowledge(false)}
                          className="flex-1 text-[12px] px-3 py-1.5 rounded-lg bg-amber-600/80 hover:bg-amber-500 text-white transition-colors">
                          Again
                        </button>
                      </div>
                    </>
                  ) : (
                    <button onClick={() => setShowAnswer(true)}
                      className="w-full text-[12px] px-3 py-2 rounded-lg bg-blue-600/80 hover:bg-blue-500 text-white transition-colors">
                      Show Answer
                    </button>
                  )}
                </div>
              );
            })()}

            {/* Active state: text input + mic button */}
            {state === "active" && !showQuiz && (
              <div className="px-3 pb-3">
                <div className="flex gap-2">
                  <input
                    ref={inputRef}
                    type="text"
                    value={inputValue}
                    onChange={(e) => setInputValue(e.target.value)}
                    placeholder="Ask YoYo anything..."
                    className="flex-1 bg-zinc-800/50 border border-zinc-700/50 rounded-lg px-3 py-2
                      text-[13px] text-white placeholder-zinc-500 outline-none
                      focus:border-violet-500/50 transition-colors"
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleSubmit();
                      if (e.key === "Escape") goAmbient();
                    }}
                    disabled={isRecording}
                  />
                  <button
                    onClick={isRecording ? handleStopRecording : handleStartRecording}
                    className={`px-2 py-2 rounded-lg transition-colors flex-shrink-0 ${
                      isRecording
                        ? "bg-red-600 hover:bg-red-500 text-white"
                        : "bg-zinc-800/50 hover:bg-zinc-700/50 text-zinc-400"
                    }`}
                    title={isRecording ? "Stop recording" : "Voice input"}
                  >
                    {isRecording ? (
                      <span className="w-3 h-3 rounded-sm bg-white block" />
                    ) : (
                      <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                        <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
                        <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                        <line x1="12" y1="19" x2="12" y2="23" />
                        <line x1="8" y1="23" x2="16" y2="23" />
                      </svg>
                    )}
                  </button>
                </div>
                <div className="flex items-center justify-between mt-1.5">
                  {isRecording ? (
                    <span className="text-[10px] text-red-400">
                      Recording... {recordingTime}s
                    </span>
                  ) : (
                    <span className="text-[10px] text-zinc-600">
                      Enter to {inputValue.trim() ? "ask" : "analyze"}
                    </span>
                  )}
                  <kbd className="px-1 py-0.5 rounded bg-white/[0.06] text-zinc-500 font-mono text-[9px]">
                    Esc
                  </kbd>
                </div>
              </div>
            )}

            {/* Working state */}
            {state === "working" && (
              <div className="px-3 pb-3 pt-1">
                <div className="flex items-center gap-2 text-[12px] text-zinc-500">
                  <span className="w-1 h-1 rounded-full bg-violet-400/50 animate-pulse" />
                  <span>{analysisStage || "Processing..."}</span>
                </div>
              </div>
            )}

            {/* Done state: passive analysis result */}
            {state === "done" && result && !intentResult && (
              <>
                <div className="px-3 pb-2">
                  <p className="text-[13px] text-zinc-300 leading-snug">
                    {result.context}
                  </p>
                </div>

                <div className="mx-3 border-t border-white/[0.06]" />

                {executing || actionDone ? (
                  <div className="px-3 py-4 flex flex-col items-center gap-2">
                    {actionDone ? (
                      <>
                        <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-violet-400">
                          <path d="M5 13l4 4L19 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                        </svg>
                        <span className="text-[11px] text-zinc-400">Done</span>
                      </>
                    ) : (
                      <>
                        <span className="w-4 h-4 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin" />
                        <span className="text-[11px] text-zinc-400">Processing...</span>
                      </>
                    )}
                  </div>
                ) : (
                  <ActionButtons
                    actions={result.actions}
                    executing={executing}
                    onExecute={handleExecute}
                    compact
                  />
                )}
              </>
            )}

            {/* Done state: intent plan */}
            {state === "done" && intentResult && (
              <>
                <div className="px-3 pb-2">
                  <p className="text-[13px] text-zinc-300 leading-snug">
                    {intentResult.understanding}
                  </p>
                  {intentResult.workflow_id && (
                    <span className="inline-block mt-1 text-[10px] text-violet-400/70 bg-violet-500/10 px-1.5 py-0.5 rounded">
                      Saved workflow
                    </span>
                  )}
                </div>

                <div className="mx-3 border-t border-white/[0.06]" />

                {executingPlan ? (
                  <div className="px-3 py-2 space-y-1.5">
                    {intentResult.plan.map((step, i) => (
                      <div key={i} className="flex items-center gap-2">
                        {i < currentStep ? (
                          <svg className="w-3 h-3 text-violet-400 flex-shrink-0" viewBox="0 0 24 24" fill="none">
                            <path d="M5 13l4 4L19 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                          </svg>
                        ) : i === currentStep ? (
                          <span className="w-3 h-3 border border-violet-400 border-t-transparent rounded-full animate-spin flex-shrink-0" />
                        ) : (
                          <span className="w-3 h-3 rounded-full border border-zinc-600 flex-shrink-0" />
                        )}
                        <span className={`text-[12px] ${i <= currentStep ? "text-zinc-300" : "text-zinc-600"}`}>
                          {step.label}
                        </span>
                      </div>
                    ))}
                  </div>
                ) : actionDone ? (
                  showTeachMe ? (
                    <div className="px-3 py-2">
                      <p className="text-[11px] text-zinc-500 mb-1.5">Save as workflow:</p>
                      <input
                        type="text"
                        value={workflowName}
                        onChange={(e) => setWorkflowName(e.target.value)}
                        placeholder="Workflow name..."
                        className="w-full bg-zinc-800/50 border border-zinc-700/50 rounded-lg px-2.5 py-1.5
                          text-[12px] text-white placeholder-zinc-500 outline-none
                          focus:border-violet-500/50 transition-colors"
                        onKeyDown={(e) => { if (e.key === "Enter") handleSaveWorkflow(); }}
                        autoFocus
                      />
                      <div className="mt-1.5 space-y-1">
                        {intentResult.plan.map((step, i) => (
                          <div key={i} className="text-[11px] text-zinc-500">
                            {i + 1}. {step.label}
                          </div>
                        ))}
                      </div>
                      <div className="mt-2 flex gap-2">
                        <button
                          onClick={handleSaveWorkflow}
                          disabled={!workflowName.trim()}
                          className="flex-1 text-[12px] px-3 py-1.5 rounded-lg bg-violet-600 hover:bg-violet-500 text-white transition-colors disabled:opacity-40"
                        >
                          Save
                        </button>
                        <button
                          onClick={() => setShowTeachMe(false)}
                          className="text-[12px] px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-400 transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <div className="px-3 py-3 flex flex-col items-center gap-2">
                      <svg viewBox="0 0 24 24" fill="none" className="w-5 h-5 text-violet-400">
                        <path d="M5 13l4 4L19 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                      </svg>
                      <span className="text-[11px] text-zinc-400">Done</span>
                      {/* Feedback buttons */}
                      {executionId && !feedbackGiven && (
                        <div className="flex gap-3 mt-1">
                          <button
                            onClick={() => handleFeedback("good")}
                            className="text-[11px] text-zinc-500 hover:text-green-400 transition-colors"
                          >
                            Good
                          </button>
                          <button
                            onClick={() => handleFeedback("bad")}
                            className="text-[11px] text-zinc-500 hover:text-red-400 transition-colors"
                          >
                            Not right
                          </button>
                        </div>
                      )}
                      {feedbackGiven && (
                        <span className="text-[10px] text-zinc-600">Thanks!</span>
                      )}
                      {/* Teach Me button */}
                      {!intentResult.workflow_id && (
                        <button
                          onClick={() => { setShowTeachMe(true); clearDismissTimer(); }}
                          className="text-[11px] text-violet-400/70 hover:text-violet-400 transition-colors mt-1"
                        >
                          Save as workflow
                        </button>
                      )}
                    </div>
                  )
                ) : failedStep >= 0 ? (
                  <>
                    <div className="px-3 py-2 space-y-1.5">
                      {intentResult.plan.map((step, i) => (
                        <div key={i} className="flex items-center gap-2">
                          {i < failedStep ? (
                            <svg className="w-3 h-3 text-violet-400 flex-shrink-0" viewBox="0 0 24 24" fill="none">
                              <path d="M5 13l4 4L19 7" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                            </svg>
                          ) : i === failedStep ? (
                            <svg className="w-3 h-3 text-red-400 flex-shrink-0" viewBox="0 0 24 24" fill="none">
                              <path d="M6 18L18 6M6 6l12 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
                            </svg>
                          ) : (
                            <span className="w-3 h-3 rounded-full border border-zinc-600 flex-shrink-0" />
                          )}
                          <span className={`text-[12px] ${i < failedStep ? "text-zinc-300" : i === failedStep ? "text-red-400" : "text-zinc-600"}`}>
                            {step.label}
                          </span>
                        </div>
                      ))}
                    </div>
                    <div className="px-3 pb-3 pt-1 flex gap-2">
                      <button
                        onClick={() => executePlan(intentResult.plan, failedStep)}
                        className="flex-1 text-[12px] px-3 py-1.5 rounded-lg bg-amber-600 hover:bg-amber-500 text-white transition-colors"
                      >
                        Retry step {failedStep + 1}
                      </button>
                      <button
                        onClick={goAmbient}
                        className="text-[12px] px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-400 transition-colors"
                      >
                        Dismiss
                      </button>
                    </div>
                  </>
                ) : (
                  <>
                    <div className="px-3 py-2 space-y-1.5">
                      {intentResult.plan.map((step, i) => (
                        <div key={i} className="flex items-start gap-2">
                          <span className="text-[11px] text-zinc-500 font-mono mt-0.5 flex-shrink-0">{i + 1}.</span>
                          <span className="text-[12px] text-zinc-400">{step.label}</span>
                        </div>
                      ))}
                    </div>

                    {intentResult.needs_confirmation && (
                      <div className="px-3 pb-3 pt-1 flex gap-2">
                        <button
                          onClick={() => executePlan(intentResult.plan)}
                          className="flex-1 text-[12px] px-3 py-1.5 rounded-lg bg-violet-600 hover:bg-violet-500 text-white transition-colors"
                        >
                          Confirm
                        </button>
                        <button
                          onClick={goAmbient}
                          className="text-[12px] px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-400 transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    )}
                  </>
                )}
              </>
            )}
          </div>
        </div>

        {/* Footer — only in done state */}
        {state === "done" && (
          <div className="flex-shrink-0">
            <div className="px-3 py-1.5 flex items-center border-t border-white/[0.06]">
              <span className="text-[10px] text-zinc-600">
                <kbd className="px-1 py-0.5 rounded bg-white/[0.06] text-zinc-500 font-mono text-[9px]">
                  Cmd+Shift+R
                </kbd>{" "}
                refresh
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
