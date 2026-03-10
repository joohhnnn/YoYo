import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import BubbleApp from "../BubbleApp";
import type { AnalysisResult, IntentResult, KnowledgeRecord } from "../types";

// --- Test fixtures ---

const mockResult: AnalysisResult = {
  context: "You are reading the React documentation about hooks",
  actions: [
    { type: "open_url", label: "Open React Docs", params: { url: "https://react.dev" } },
    { type: "copy_to_clipboard", label: "Copy useEffect example", params: { text: "useEffect(() => {}, [])" } },
  ],
};

const mockIntentResult: IntentResult = {
  understanding: "User wants to open the React documentation",
  plan: [
    { action_type: "open_url", label: "Open React website", params: { url: "https://react.dev" } },
    { action_type: "notify", label: "Notify when done", params: { message: "React docs opened" } },
  ],
  needs_confirmation: true,
};

const mockKnowledge: KnowledgeRecord = {
  id: 10,
  kind: "vocab",
  content: "useEffect",
  source: "Chrome (https://react.dev/docs)",
  metadata: JSON.stringify({
    definition: "A React Hook that lets you synchronize a component with an external system",
    review_count: 0,
    interval_level: 0,
    next_review: "2026-03-10 12:00:00",
    last_reviewed: null,
  }),
  created_at: "2026-03-10 11:00:00",
};

const mockSettings = {
  ai_mode: "cli",
  api_key: "",
  model: "claude-haiku-4-5-20251001",
  shortcut_toggle: "CmdOrCtrl+Shift+Y",
  shortcut_analyze: "CmdOrCtrl+Shift+R",
  analysis_cooldown_secs: 2,
  bubble_opacity: 0.9,
  language: "zh",
  auto_analyze: true,
  analysis_depth: "normal" as const,
  app_blacklist: [],
  onboarding_completed: true,
  preferred_mic_device: "",
  sound_enabled: true,
  bubble_x: null,
  bubble_y: null,
  current_scene: null,
};

// --- Helpers ---

function setupInvokeMock(overrides: { result?: AnalysisResult | null; intentResult?: IntentResult | null } = {}) {
  const { result = null, intentResult = null } = overrides;
  (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_settings": return Promise.resolve(mockSettings);
      case "get_last_analysis": return Promise.resolve(result);
      case "analyze_screen": return Promise.resolve(result ?? mockResult);
      case "understand_intent": return Promise.resolve(intentResult ?? mockIntentResult);
      case "execute_action": return Promise.resolve(null);
      case "check_voice_permission": return Promise.resolve("granted");
      case "request_voice_permission": return Promise.resolve(true);
      case "start_recording": return Promise.resolve("/tmp/test.wav");
      case "stop_and_transcribe": return Promise.resolve("hello world");
      case "start_execution": return Promise.resolve(null);
      case "record_execution": return Promise.resolve(42);
      case "complete_execution": return Promise.resolve(null);
      case "feedback_execution": return Promise.resolve(null);
      case "save_workflow": return Promise.resolve(1);
      case "update_workflow_count": return Promise.resolve(null);
      case "get_due_knowledge": return Promise.resolve([]);
      case "review_knowledge": return Promise.resolve(null);
      case "delete_knowledge": return Promise.resolve(null);
      case "get_knowledge_stats": return Promise.resolve({ total: 0, due: 0 });
      case "check_inserted_text": return Promise.resolve({ found: true, reverted: false });
      case "get_recent_executions": return Promise.resolve([]);
      case "play_sound": return Promise.resolve(null);
      default: return Promise.resolve(null);
    }
  });
}

async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

// --- Tests ---

describe("BubbleApp State Machine", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (listen as ReturnType<typeof vi.fn>).mockImplementation(() =>
      Promise.resolve(() => {})
    );
  });

  describe("Ambient state (default)", () => {
    it("renders breathing dot", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      const dot = container.querySelector(".dot-breathing");
      expect(dot).toBeTruthy();
      expect(dot?.className).toContain("bg-violet-500");
    });

    it("dot container is 48x48", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      const dotContainer = container.querySelector(".w-12");
      expect(dotContainer).toBeTruthy();
      expect(dotContainer?.className).toContain("h-12");
    });

    it("does NOT render input field or actions", async () => {
      setupInvokeMock();
      render(<BubbleApp />);
      await flush();

      expect(screen.queryByPlaceholderText("Ask YoYo anything...")).not.toBeInTheDocument();
      expect(screen.queryByText("Open React Docs")).not.toBeInTheDocument();
      expect(screen.queryByText("refresh")).not.toBeInTheDocument();
    });
  });

  describe("Active state", () => {
    it("clicking dot transitions to active with input field", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      expect(screen.getByPlaceholderText("Ask YoYo anything...")).toBeInTheDocument();
      expect(screen.getByText("Enter to analyze")).toBeInTheDocument();
    });

    it("shows glass container with violet header dot", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      const glass = container.querySelector(".backdrop-blur-xl");
      expect(glass).toBeTruthy();

      const headerDot = container.querySelector(".bg-violet-400");
      expect(headerDot).toBeTruthy();
    });

    it("Esc returns to ambient", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Press Esc
      await act(async () => {
        fireEvent.keyDown(window, { key: "Escape" });
      });

      // Should be back to ambient
      const dot = container.querySelector(".dot-breathing");
      expect(dot).toBeTruthy();
    });
  });

  describe("Working state", () => {
    it("shows cancel button when analyzing", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Enter → trigger analysis → working
      const input = screen.getByPlaceholderText("Ask YoYo anything...");
      await act(async () => {
        fireEvent.keyDown(input, { key: "Enter" });
      });

      expect(screen.getByText("Cancel")).toBeInTheDocument();
    });
  });

  describe("Done state", () => {
    it("shows analysis result with context and actions", async () => {
      setupInvokeMock();
      let analysisCallback: ((event: { payload: AnalysisResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "analysis-complete") analysisCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        analysisCallback?.({ payload: mockResult });
      });

      expect(screen.getByText(mockResult.context)).toBeInTheDocument();
      expect(screen.getByText("Open React Docs")).toBeInTheDocument();
      expect(screen.getByText("Copy useEffect example")).toBeInTheDocument();
      expect(screen.getByText("Dismiss")).toBeInTheDocument();
      expect(screen.getByText("refresh")).toBeInTheDocument();
    });

    it("dismiss returns to ambient", async () => {
      setupInvokeMock();
      let analysisCallback: ((event: { payload: AnalysisResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "analysis-complete") analysisCallback = cb;
        return Promise.resolve(() => {});
      });

      const { container } = render(<BubbleApp />);
      await flush();

      await act(async () => {
        analysisCallback?.({ payload: mockResult });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Dismiss"));
      });

      const dot = container.querySelector(".dot-breathing");
      expect(dot).toBeTruthy();
    });
  });

  describe("Layout", () => {
    it("expanded states use glass container with flex layout", async () => {
      setupInvokeMock();
      let analysisCallback: ((event: { payload: AnalysisResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "analysis-complete") analysisCallback = cb;
        return Promise.resolve(() => {});
      });

      const { container } = render(<BubbleApp />);
      await flush();

      await act(async () => {
        analysisCallback?.({ payload: mockResult });
      });

      const innerDiv = container.querySelector(".backdrop-blur-xl");
      expect(innerDiv).toBeTruthy();
      expect(innerDiv?.className).toContain("flex");
      expect(innerDiv?.className).toContain("flex-col");
      expect(innerDiv?.className).toContain("overflow-hidden");
      expect(innerDiv?.className).toContain("max-h-screen");
    });
  });

  describe("Intent Pipeline", () => {
    it("empty enter triggers analyze_screen", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Enter with empty input → passive analysis
      const input = screen.getByPlaceholderText("Ask YoYo anything...");
      await act(async () => {
        fireEvent.keyDown(input, { key: "Enter" });
      });

      expect(invoke).toHaveBeenCalledWith("analyze_screen");
    });

    it("text + enter triggers understand_intent", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Type text and press Enter
      const input = screen.getByPlaceholderText("Ask YoYo anything...");
      await act(async () => {
        fireEvent.change(input, { target: { value: "open react docs" } });
      });
      await act(async () => {
        fireEvent.keyDown(input, { key: "Enter" });
      });

      expect(invoke).toHaveBeenCalledWith("understand_intent", { userInput: "open react docs" });
    });

    it("shows understanding text and plan steps", async () => {
      setupInvokeMock();
      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      expect(screen.getByText(mockIntentResult.understanding)).toBeInTheDocument();
      expect(screen.getByText("Open React website")).toBeInTheDocument();
      expect(screen.getByText("Notify when done")).toBeInTheDocument();
      expect(screen.getByText("Confirm")).toBeInTheDocument();
    });

    it("confirm button executes plan steps", async () => {
      setupInvokeMock();
      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      // Should have called execute_action for each step
      expect(invoke).toHaveBeenCalledWith("execute_action", {
        actionType: "open_url",
        params: { url: "https://react.dev" },
      });
      expect(invoke).toHaveBeenCalledWith("execute_action", {
        actionType: "notify",
        params: { message: "React docs opened" },
      });
    });

    it("cancel during working calls cancel_execution", async () => {
      // Make understand_intent hang so we stay in working state
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "understand_intent": return new Promise(() => {}); // never resolves
          case "cancel_execution": return Promise.resolve(null);
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Type text and submit to enter working state
      const input = screen.getByPlaceholderText("Ask YoYo anything...");
      await act(async () => {
        fireEvent.change(input, { target: { value: "do something" } });
      });
      await act(async () => {
        fireEvent.keyDown(input, { key: "Enter" });
      });

      // Should be in working state with Cancel button visible
      expect(screen.getByText("Cancel")).toBeInTheDocument();

      // Click Cancel
      await act(async () => {
        fireEvent.click(screen.getByText("Cancel"));
      });

      expect(invoke).toHaveBeenCalledWith("cancel_execution");
    });

    it("shows retry button when step fails", async () => {
      let callCount = 0;
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "start_execution": return Promise.resolve(null);
          case "execute_action":
            callCount++;
            if (callCount === 2) return Promise.reject("Network error");
            return Promise.resolve(null);
          default: return Promise.resolve(null);
        }
      });

      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;
      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      // Trigger intent result with needs_confirmation
      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      // Click Confirm to start execution
      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      // Should show retry button for step 2
      expect(screen.getByText("Retry step 2")).toBeInTheDocument();
    });

    it("shows claude_code step in plan", async () => {
      setupInvokeMock();
      const claudeIntent: IntentResult = {
        understanding: "User wants to fix a bug",
        plan: [
          { action_type: "claude_code", label: "Fix bug with Claude", params: { prompt: "fix the bug", directory: "/tmp" } },
        ],
        needs_confirmation: true,
      };

      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;
      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: claudeIntent });
      });

      expect(screen.getByText("Fix bug with Claude")).toBeInTheDocument();
    });

    it("hint text changes based on input content", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Empty input shows "Enter to analyze"
      expect(screen.getByText("Enter to analyze")).toBeInTheDocument();

      // Type text → hint changes to "Enter to ask"
      const input = screen.getByPlaceholderText("Ask YoYo anything...");
      await act(async () => {
        fireEvent.change(input, { target: { value: "hello" } });
      });

      expect(screen.getByText("Enter to ask")).toBeInTheDocument();
    });
  });

  describe("Voice Input", () => {
    it("mic button visible in active state", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      const micButton = container.querySelector("button[title='Voice input']");
      expect(micButton).toBeTruthy();
    });

    it("mic button starts recording", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Click mic button
      const micButton = container.querySelector("button[title='Voice input']");
      await act(async () => {
        fireEvent.click(micButton!);
      });

      expect(invoke).toHaveBeenCalledWith("check_voice_permission");
      expect(invoke).toHaveBeenCalledWith("start_recording");
    });

    it("stop button stops and transcribes", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Start recording
      const micButton = container.querySelector("button[title='Voice input']");
      await act(async () => {
        fireEvent.click(micButton!);
      });

      // Click stop button
      const stopButton = container.querySelector("button[title='Stop recording']");
      await act(async () => {
        fireEvent.click(stopButton!);
      });

      expect(invoke).toHaveBeenCalledWith("stop_and_transcribe");

      // Input should be filled with transcribed text
      const input = screen.getByPlaceholderText("Ask YoYo anything...") as HTMLInputElement;
      expect(input.value).toBe("hello world");
    });

    it("recording shows timer text", async () => {
      setupInvokeMock();
      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Start recording
      const micButton = container.querySelector("button[title='Voice input']");
      await act(async () => {
        fireEvent.click(micButton!);
      });

      expect(screen.getByText("Recording... 0s")).toBeInTheDocument();
    });

    it("permission denied shows error", async () => {
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "check_voice_permission": return Promise.resolve("denied");
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Click dot → active
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Click mic button
      const micButton = container.querySelector("button[title='Voice input']");
      await act(async () => {
        fireEvent.click(micButton!);
      });

      expect(screen.getByText("Microphone permission denied")).toBeInTheDocument();
    });
  });

  describe("Workflow Learning", () => {
    it("records execution when plan runs", async () => {
      setupInvokeMock();
      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      expect(invoke).toHaveBeenCalledWith("record_execution", expect.objectContaining({
        planJson: expect.any(String),
      }));
      expect(invoke).toHaveBeenCalledWith("complete_execution", expect.objectContaining({
        id: 42,
        status: "success",
      }));
    });

    it("shows Save as workflow after successful execution", async () => {
      setupInvokeMock();
      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      expect(screen.getByText("Save as workflow")).toBeInTheDocument();
    });

    it("save workflow form works", async () => {
      setupInvokeMock();
      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      // Confirm execution
      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      // Click "Save as workflow"
      await act(async () => {
        fireEvent.click(screen.getByText("Save as workflow"));
      });

      // Fill in name and save
      const nameInput = screen.getByPlaceholderText("Workflow name...");
      await act(async () => {
        fireEvent.change(nameInput, { target: { value: "My Workflow" } });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Save"));
      });

      expect(invoke).toHaveBeenCalledWith("save_workflow", expect.objectContaining({
        name: "My Workflow",
        stepsJson: expect.any(String),
      }));
    });

    it("feedback buttons call feedback_execution", async () => {
      setupInvokeMock();
      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: mockIntentResult });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      // Click "Good" feedback
      await act(async () => {
        fireEvent.click(screen.getByText("Good"));
      });

      expect(invoke).toHaveBeenCalledWith("feedback_execution", { id: 42, feedback: "good" });
      expect(screen.getByText("Thanks!")).toBeInTheDocument();
    });

    it("shows workflow badge when matched", async () => {
      setupInvokeMock();
      const workflowIntent: IntentResult = {
        understanding: "Matched workflow: Deploy app",
        plan: mockIntentResult.plan,
        needs_confirmation: true,
        workflow_id: 5,
      };

      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;
      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: workflowIntent });
      });

      expect(screen.getByText("Saved workflow")).toBeInTheDocument();
      // Should NOT show "Save as workflow" since it already is one
      expect(screen.queryByText("Save as workflow")).not.toBeInTheDocument();
    });
  });

  describe("Knowledge Nudge", () => {
    it("shows blue dot when due knowledge exists", async () => {
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([mockKnowledge]);
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Blue dot should be visible (animate-pulse class)
      const blueDot = container.querySelector(".bg-blue-400");
      expect(blueDot).toBeTruthy();
    });

    it("clicking bubble with nudge opens quiz", async () => {
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([mockKnowledge]);
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Click the bubble (should open quiz)
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Quiz card should show
      expect(screen.getByText("useEffect")).toBeInTheDocument();
      expect(screen.getByText("Vocabulary")).toBeInTheDocument();
      expect(screen.getByText("Show Answer")).toBeInTheDocument();
    });

    it("Show Answer reveals definition", async () => {
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([mockKnowledge]);
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Click bubble to open quiz
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Click Show Answer
      await act(async () => {
        fireEvent.click(screen.getByText("Show Answer"));
      });

      expect(screen.getByText("A React Hook that lets you synchronize a component with an external system")).toBeInTheDocument();
      expect(screen.getByText("Got it")).toBeInTheDocument();
      expect(screen.getByText("Again")).toBeInTheDocument();
    });

    it("Got it calls review_knowledge with success", async () => {
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([mockKnowledge]);
          case "review_knowledge": return Promise.resolve(null);
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Open quiz
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      // Reveal answer
      await act(async () => {
        fireEvent.click(screen.getByText("Show Answer"));
      });

      // Click Got it
      await act(async () => {
        fireEvent.click(screen.getByText("Got it"));
      });

      expect(invoke).toHaveBeenCalledWith("review_knowledge", { id: 10, success: true });
    });

    it("Skip dismisses quiz", async () => {
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([mockKnowledge]);
          default: return Promise.resolve(null);
        }
      });

      const { container } = render(<BubbleApp />);
      await flush();

      // Open quiz
      const dotContainer = container.querySelector(".w-12");
      await act(async () => {
        fireEvent.click(dotContainer!);
      });

      expect(screen.getByText("useEffect")).toBeInTheDocument();

      // Click Skip
      await act(async () => {
        fireEvent.click(screen.getByText("Skip"));
      });

      // Quiz should be gone, should show input instead
      expect(screen.queryByText("useEffect")).not.toBeInTheDocument();
      expect(screen.getByPlaceholderText("Ask YoYo anything...")).toBeInTheDocument();
    });
  });

  describe("Edit Tracking", () => {
    const insertTextIntent: IntentResult = {
      understanding: "User wants to insert a greeting",
      plan: [
        { action_type: "insert_text", label: "Insert greeting", params: { text: "Hello, world!" } },
      ],
      needs_confirmation: true,
    };

    it("shows checking status after insert_text step", async () => {
      setupInvokeMock();

      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;
      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      await act(async () => {
        intentCallback?.({ payload: insertTextIntent });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
      });

      // "checking..." should appear after execution completes (8s timer not yet fired)
      expect(screen.getByText("checking...")).toBeInTheDocument();
    });

    it("shows kept badge when text is found", async () => {
      vi.useFakeTimers();
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "start_execution": return Promise.resolve(null);
          case "execute_action": return Promise.resolve(null);
          case "record_execution": return Promise.resolve(42);
          case "complete_execution": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([]);
          case "check_inserted_text": return Promise.resolve({ found: true, reverted: false });
          default: return Promise.resolve(null);
        }
      });

      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;
      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await act(async () => { await vi.advanceTimersByTimeAsync(1); });

      await act(async () => {
        intentCallback?.({ payload: insertTextIntent });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
        await vi.advanceTimersByTimeAsync(1);
      });

      // Advance timer past the 8-second check
      await act(async () => {
        await vi.advanceTimersByTimeAsync(9000);
      });

      expect(screen.getByText("kept")).toBeInTheDocument();

      vi.useRealTimers();
    });

    it("shows edited badge when text is reverted", async () => {
      vi.useFakeTimers();
      (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
        switch (cmd) {
          case "get_settings": return Promise.resolve(mockSettings);
          case "get_last_analysis": return Promise.resolve(null);
          case "start_execution": return Promise.resolve(null);
          case "execute_action": return Promise.resolve(null);
          case "record_execution": return Promise.resolve(42);
          case "complete_execution": return Promise.resolve(null);
          case "get_due_knowledge": return Promise.resolve([]);
          case "check_inserted_text": return Promise.resolve({ found: false, reverted: true });
          case "feedback_execution": return Promise.resolve(null);
          default: return Promise.resolve(null);
        }
      });

      let intentCallback: ((event: { payload: IntentResult }) => void) | null = null;
      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "intent-complete") intentCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await act(async () => { await vi.advanceTimersByTimeAsync(1); });

      await act(async () => {
        intentCallback?.({ payload: insertTextIntent });
      });

      await act(async () => {
        fireEvent.click(screen.getByText("Confirm"));
        await vi.advanceTimersByTimeAsync(1);
      });

      // Advance timer past the 8-second check
      await act(async () => {
        await vi.advanceTimersByTimeAsync(9000);
      });

      expect(screen.getByText("edited")).toBeInTheDocument();
      // Should auto-feedback as reverted
      expect(invoke).toHaveBeenCalledWith("feedback_execution", { id: 42, feedback: "reverted" });

      vi.useRealTimers();
    });
  });
});
