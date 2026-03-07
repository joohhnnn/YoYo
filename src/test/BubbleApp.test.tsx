import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import BubbleApp from "../BubbleApp";
import type { AnalysisResult, Session, TimelineEntry } from "../types";

// --- Test fixtures ---

const mockSession: Session = {
  id: "sess-1",
  goal: "Learn React hooks deeply",
  started_at: "2026-03-08 10:00:00",
  status: "active",
};

const mockTimeline: TimelineEntry[] = [
  { id: 1, session_id: "sess-1", timestamp: "2026-03-08 10:05:00", context: "Reading React docs", app_name: "Chrome" },
  { id: 2, session_id: "sess-1", timestamp: "2026-03-08 10:12:00", context: "Editing useEffect hook", app_name: "VS Code" },
  { id: 3, session_id: "sess-1", timestamp: "2026-03-08 10:20:00", context: "Testing component", app_name: "Terminal" },
];

const mockResult: AnalysisResult = {
  context: "You are reading the React documentation about hooks",
  actions: [
    { type: "open_url", label: "Open React Docs", params: { url: "https://react.dev" } },
    { type: "copy_to_clipboard", label: "Copy useEffect example", params: { text: "useEffect(() => {}, [])" } },
  ],
  key_concepts: ["useEffect", "useState", "Custom Hooks"],
  on_track: true,
};

const mockResultWithDrift: AnalysisResult = {
  ...mockResult,
  on_track: false,
  drift_message: "You seem to be browsing social media instead of studying React",
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
  scene_mode: "general" as const,
  obsidian_enabled: false,
  obsidian_vault_path: "",
};

// --- Helpers ---

/**
 * Configure invoke mock to return specific data based on command name.
 */
function setupInvokeMock(overrides: {
  session?: Session | null;
  timeline?: TimelineEntry[];
  result?: AnalysisResult | null;
  needsOnboarding?: boolean;
} = {}) {
  const { session = null, timeline = [], result = null, needsOnboarding = false } = overrides;

  (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_settings": return Promise.resolve(mockSettings);
      case "get_active_session": return Promise.resolve(session);
      case "get_session_timeline": return Promise.resolve(timeline);
      case "get_last_analysis": return Promise.resolve(result);
      case "check_needs_onboarding": return Promise.resolve(needsOnboarding);
      case "start_onboarding": return Promise.resolve({ role: "assistant", content: "Welcome!" });
      default: return Promise.resolve(null);
    }
  });
}

/**
 * Helper to flush all pending promises and state updates.
 */
async function flush() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 0));
  });
}

/**
 * Get the setSize mock to check window resize calls.
 */
function getSetSizeMock() {
  return (getCurrentWebviewWindow() as any).setSize as ReturnType<typeof vi.fn>;
}

// --- Tests ---

describe("BubbleApp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Default listen returns unlisten noop
    (listen as ReturnType<typeof vi.fn>).mockImplementation(() =>
      Promise.resolve(() => {})
    );
  });

  describe("State 1: Idle — no session, no analysis", () => {
    it("renders header, session start input, and footer", async () => {
      setupInvokeMock();
      render(<BubbleApp />);
      await flush();

      // Header text
      expect(screen.getByText("YoYo")).toBeInTheDocument();

      // Session start input
      expect(screen.getByPlaceholderText("Start a session...")).toBeInTheDocument();

      // Go button
      expect(screen.getByText("Go")).toBeInTheDocument();

      // Footer shortcut hint
      expect(screen.getByText("refresh")).toBeInTheDocument();

      // Should NOT show session-specific UI
      expect(screen.queryByText("End")).not.toBeInTheDocument();
      expect(screen.queryByPlaceholderText("Ask YoYo...")).not.toBeInTheDocument();
    });
  });

  describe("State 2: Idle — no session, with analysis result", () => {
    it("renders context, actions, key concepts, and session start input", async () => {
      setupInvokeMock({ result: mockResult });
      render(<BubbleApp />);
      await flush();

      // Context text
      expect(screen.getByText(mockResult.context)).toBeInTheDocument();

      // Action buttons
      expect(screen.getByText("Open React Docs")).toBeInTheDocument();
      expect(screen.getByText("Copy useEffect example")).toBeInTheDocument();

      // Key concepts
      expect(screen.getByText("useEffect")).toBeInTheDocument();
      expect(screen.getByText("useState")).toBeInTheDocument();
      expect(screen.getByText("Custom Hooks")).toBeInTheDocument();

      // Session start input still present
      expect(screen.getByPlaceholderText("Start a session...")).toBeInTheDocument();
      expect(screen.getByText("Go")).toBeInTheDocument();

      // Footer
      expect(screen.getByText("refresh")).toBeInTheDocument();
    });
  });

  describe("State 3: Active session — no analysis", () => {
    it("renders session header, goal banner, chat input, and End button", async () => {
      setupInvokeMock({ session: mockSession });
      render(<BubbleApp />);
      await flush();

      // Session indicator in header
      expect(screen.getByText(/Session/)).toBeInTheDocument();

      // End button
      expect(screen.getByText("End")).toBeInTheDocument();

      // Goal banner
      expect(screen.getByText("Learn React hooks deeply")).toBeInTheDocument();

      // Chat input (not start session input)
      expect(screen.getByPlaceholderText("Ask YoYo...")).toBeInTheDocument();
      expect(screen.queryByPlaceholderText("Start a session...")).not.toBeInTheDocument();

      // Footer
      expect(screen.getByText("refresh")).toBeInTheDocument();
    });
  });

  describe("State 4: Active session — with analysis + timeline", () => {
    it("renders ALL sections: header, goal, context, timeline, concepts, actions, chat input, footer", async () => {
      setupInvokeMock({
        session: mockSession,
        timeline: mockTimeline,
        result: mockResult,
      });
      render(<BubbleApp />);
      await flush();

      // --- Header ---
      expect(screen.getByText(/Session/)).toBeInTheDocument();
      expect(screen.getByText("End")).toBeInTheDocument();

      // --- Goal banner ---
      expect(screen.getByText("Learn React hooks deeply")).toBeInTheDocument();

      // --- Context ---
      expect(screen.getByText(mockResult.context)).toBeInTheDocument();

      // --- Timeline (last 3 entries) ---
      expect(screen.getByText("Reading React docs")).toBeInTheDocument();
      expect(screen.getByText("Editing useEffect hook")).toBeInTheDocument();
      expect(screen.getByText("Testing component")).toBeInTheDocument();

      // --- Key concepts ---
      expect(screen.getByText("useEffect")).toBeInTheDocument();
      expect(screen.getByText("useState")).toBeInTheDocument();
      expect(screen.getByText("Custom Hooks")).toBeInTheDocument();

      // --- Actions ---
      expect(screen.getByText("Open React Docs")).toBeInTheDocument();
      expect(screen.getByText("Copy useEffect example")).toBeInTheDocument();

      // --- Chat input ---
      expect(screen.getByPlaceholderText("Ask YoYo...")).toBeInTheDocument();

      // --- Footer ---
      expect(screen.getByText("refresh")).toBeInTheDocument();
    });
  });

  describe("State 5: Active session with many timeline entries", () => {
    it("only shows last 3 timeline entries", async () => {
      const longTimeline: TimelineEntry[] = [
        { id: 1, session_id: "sess-1", timestamp: "2026-03-08 10:00:00", context: "Entry one", app_name: "A" },
        { id: 2, session_id: "sess-1", timestamp: "2026-03-08 10:05:00", context: "Entry two", app_name: "B" },
        { id: 3, session_id: "sess-1", timestamp: "2026-03-08 10:10:00", context: "Entry three", app_name: "C" },
        { id: 4, session_id: "sess-1", timestamp: "2026-03-08 10:15:00", context: "Entry four", app_name: "D" },
        { id: 5, session_id: "sess-1", timestamp: "2026-03-08 10:20:00", context: "Entry five", app_name: "E" },
      ];
      setupInvokeMock({ session: mockSession, timeline: longTimeline });
      render(<BubbleApp />);
      await flush();

      // Only last 3 shown
      expect(screen.queryByText("Entry one")).not.toBeInTheDocument();
      expect(screen.queryByText("Entry two")).not.toBeInTheDocument();
      expect(screen.getByText("Entry three")).toBeInTheDocument();
      expect(screen.getByText("Entry four")).toBeInTheDocument();
      expect(screen.getByText("Entry five")).toBeInTheDocument();
    });
  });

  describe("Layout: flex container with scroll", () => {
    it("inner container uses flex-col with overflow-hidden for proper layout", async () => {
      setupInvokeMock({ session: mockSession, timeline: mockTimeline, result: mockResult });
      const { container } = render(<BubbleApp />);
      await flush();

      const innerDiv = container.querySelector(".backdrop-blur-xl");
      expect(innerDiv).toBeTruthy();
      expect(innerDiv?.className).toContain("flex");
      expect(innerDiv?.className).toContain("flex-col");
      expect(innerDiv?.className).toContain("overflow-hidden");
    });

    it("content area uses flex-1 + min-h-0 for scrollable overflow", async () => {
      setupInvokeMock({ session: mockSession, timeline: mockTimeline, result: mockResult });
      const { container } = render(<BubbleApp />);
      await flush();

      const innerDiv = container.querySelector(".backdrop-blur-xl");
      const contentDiv = innerDiv!.querySelectorAll(":scope > div")[1]; // second child = content area
      expect(contentDiv.className).toContain("flex-1");
      expect(contentDiv.className).toContain("min-h-0");
      expect(contentDiv.className).toContain("overflow-y-auto");
    });

    it("all sections are visible in DOM", async () => {
      setupInvokeMock({ session: mockSession, timeline: mockTimeline, result: mockResult });
      const { container } = render(<BubbleApp />);
      await flush();

      // Header, content, and footer should all be present
      const innerDiv = container.querySelector(".backdrop-blur-xl");
      const children = innerDiv!.querySelectorAll(":scope > div");
      expect(children.length).toBe(3); // header, content, bottom
    });
  });

  describe("IME composition handling", () => {
    it("does not submit session start on Enter during IME composition", async () => {
      setupInvokeMock();
      render(<BubbleApp />);
      await flush();

      const input = screen.getByPlaceholderText("Start a session...");
      fireEvent.change(input, { target: { value: "学习" } });

      // Dispatch native KeyboardEvent with isComposing: true
      // (fireEvent doesn't propagate isComposing to nativeEvent properly)
      input.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true })
      );
      await flush();

      // start_session should NOT have been called
      const startCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c) => c[0] === "start_session"
      );
      expect(startCalls).toHaveLength(0);
    });

    it("submits session start on Enter when NOT composing", async () => {
      setupInvokeMock();
      render(<BubbleApp />);
      await flush();

      const input = screen.getByPlaceholderText("Start a session...");
      fireEvent.change(input, { target: { value: "Learn React" } });

      input.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", isComposing: false, bubbles: true })
      );
      await flush();

      expect(invoke).toHaveBeenCalledWith("start_session", { goal: "Learn React" });
    });

    it("does not submit chat on Enter during IME composition", async () => {
      setupInvokeMock({ session: mockSession });
      render(<BubbleApp />);
      await flush();

      const input = screen.getByPlaceholderText("Ask YoYo...");
      fireEvent.change(input, { target: { value: "什么是" } });

      input.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true })
      );
      await flush();

      const chatCalls = (invoke as ReturnType<typeof vi.fn>).mock.calls.filter(
        (c) => c[0] === "send_session_message"
      );
      expect(chatCalls).toHaveLength(0);
    });
  });
});
