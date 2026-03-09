interface StatusIndicatorProps {
  loading: boolean;
  error: string | null;
}

// Map technical errors to user-friendly messages with suggestions
function friendlyError(error: string): { message: string; hint?: string } {
  const lower = error.toLowerCase();
  if (lower.includes("api key") || lower.includes("authentication") || lower.includes("401")) {
    return { message: "API key invalid or missing", hint: "Check Settings > API Key" };
  }
  if (lower.includes("network") || lower.includes("fetch") || lower.includes("connect")) {
    return { message: "Network error", hint: "Check your internet connection" };
  }
  if (lower.includes("claude cli") || lower.includes("not found")) {
    return { message: "Claude CLI not found", hint: "Install with: npm i -g @anthropic-ai/claude-code" };
  }
  if (lower.includes("rate limit") || lower.includes("429")) {
    return { message: "Rate limited", hint: "Wait a moment and try again" };
  }
  if (lower.includes("skipped")) {
    return { message: error.replace("Skipped: ", ""), hint: undefined };
  }
  // Truncate long technical errors
  const msg = error.length > 80 ? error.slice(0, 77) + "..." : error;
  return { message: msg };
}

export function StatusIndicator({ loading, error }: StatusIndicatorProps) {
  if (error) {
    const { message, hint } = friendlyError(error);
    return (
      <div className="flex items-start gap-1.5 px-3 py-1.5 text-xs text-red-400 bg-red-950/30 rounded">
        <span className="w-1.5 h-1.5 rounded-full bg-red-500 flex-shrink-0 mt-1" />
        <div className="min-w-0">
          <span className="break-words">{message}</span>
          {hint && (
            <span className="block text-[10px] text-red-400/60 mt-0.5">{hint}</span>
          )}
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-violet-400 bg-violet-950/30 rounded">
        <span className="w-1.5 h-1.5 rounded-full bg-violet-500 animate-pulse" />
        Analyzing screen...
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-green-400 bg-green-950/30 rounded">
      <span className="w-1.5 h-1.5 rounded-full bg-green-500" />
      Ready
    </div>
  );
}
