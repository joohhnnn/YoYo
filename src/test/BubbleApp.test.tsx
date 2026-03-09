import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  app_blacklist: [],
};

// --- Helpers ---

function setupInvokeMock(overrides: { result?: AnalysisResult | null } = {}) {
  const { result = null } = overrides;
  (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_settings": return Promise.resolve(mockSettings);
      case "get_last_analysis": return Promise.resolve(result);
      case "analyze_screen": return Promise.resolve(result ?? mockResult);
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
});
