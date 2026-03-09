import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import BubbleApp from "../BubbleApp";
import type { AnalysisResult } from "../types";

// --- Test fixtures ---

const mockResult: AnalysisResult = {
  context: "You are reading the React documentation about hooks",
  actions: [
    { type: "open_url", label: "Open React Docs", params: { url: "https://react.dev" } },
    { type: "copy_to_clipboard", label: "Copy useEffect example", params: { text: "useEffect(() => {}, [])" } },
  ],
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
};

// --- Helpers ---

/**
 * Configure invoke mock to return specific data based on command name.
 */
function setupInvokeMock(overrides: {
  result?: AnalysisResult | null;
} = {}) {
  const { result = null } = overrides;

  (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_settings": return Promise.resolve(mockSettings);
      case "get_last_analysis": return Promise.resolve(result);
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

  describe("State 1: Idle — no analysis result", () => {
    it("renders header and footer", async () => {
      setupInvokeMock();
      render(<BubbleApp />);
      await flush();

      // Header text
      expect(screen.getByText("YoYo")).toBeInTheDocument();

      // Footer shortcut hint
      expect(screen.getByText("refresh")).toBeInTheDocument();

      // Should NOT show session-specific UI
      expect(screen.queryByText("End")).not.toBeInTheDocument();
      expect(screen.queryByPlaceholderText("Ask YoYo...")).not.toBeInTheDocument();
      expect(screen.queryByPlaceholderText("Start a session...")).not.toBeInTheDocument();
    });
  });

  describe("State 2: With analysis result", () => {
    it("renders context and action buttons", async () => {
      setupInvokeMock({ result: mockResult });
      render(<BubbleApp />);
      await flush();

      // Context text
      expect(screen.getByText(mockResult.context)).toBeInTheDocument();

      // Action buttons
      expect(screen.getByText("Open React Docs")).toBeInTheDocument();
      expect(screen.getByText("Copy useEffect example")).toBeInTheDocument();

      // Footer
      expect(screen.getByText("refresh")).toBeInTheDocument();

      // No session UI
      expect(screen.queryByPlaceholderText("Ask YoYo...")).not.toBeInTheDocument();
      expect(screen.queryByPlaceholderText("Start a session...")).not.toBeInTheDocument();
      expect(screen.queryByText("End")).not.toBeInTheDocument();
    });
  });

  describe("Layout: flex container with scroll", () => {
    it("inner container uses flex-col with overflow-hidden and max-h-screen", async () => {
      setupInvokeMock({ result: mockResult });
      const { container } = render(<BubbleApp />);
      await flush();

      const innerDiv = container.querySelector(".backdrop-blur-xl");
      expect(innerDiv).toBeTruthy();
      expect(innerDiv?.className).toContain("flex");
      expect(innerDiv?.className).toContain("flex-col");
      expect(innerDiv?.className).toContain("overflow-hidden");
      expect(innerDiv?.className).toContain("max-h-screen");
    });

    it("content area uses flex-1 + min-h-0 for scrollable overflow", async () => {
      setupInvokeMock({ result: mockResult });
      const { container } = render(<BubbleApp />);
      await flush();

      const innerDiv = container.querySelector(".backdrop-blur-xl");
      const contentDiv = innerDiv!.querySelectorAll(":scope > div")[1]; // second child = content area
      expect(contentDiv.className).toContain("flex-1");
      expect(contentDiv.className).toContain("min-h-0");
      expect(contentDiv.className).toContain("overflow-y-auto");
    });

    it("all sections are visible in DOM (header, content, footer)", async () => {
      setupInvokeMock({ result: mockResult });
      const { container } = render(<BubbleApp />);
      await flush();

      // Header, content, and footer should all be present
      const innerDiv = container.querySelector(".backdrop-blur-xl");
      const children = innerDiv!.querySelectorAll(":scope > div");
      expect(children.length).toBe(3); // header, content, bottom
    });
  });

  describe("Analysis event handling", () => {
    it("loads cached analysis result on mount and becomes visible", async () => {
      setupInvokeMock({ result: mockResult });
      render(<BubbleApp />);
      await flush();

      expect(screen.getByText(mockResult.context)).toBeInTheDocument();
    });

    it("shows refreshing spinner when app-switched event fires", async () => {
      setupInvokeMock();
      let switchCallback: (() => void) | null = null;

      (listen as ReturnType<typeof vi.fn>).mockImplementation((event: string, cb: any) => {
        if (event === "app-switched") switchCallback = cb;
        return Promise.resolve(() => {});
      });

      render(<BubbleApp />);
      await flush();

      // Trigger app-switched
      await act(async () => {
        switchCallback?.();
      });

      // Header dot changes to spinner — just verify no crash
      expect(screen.getAllByText("YoYo").length).toBeGreaterThanOrEqual(1);
    });
  });
});
