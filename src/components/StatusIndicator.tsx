interface StatusIndicatorProps {
  loading: boolean;
  error: string | null;
}

export function StatusIndicator({ loading, error }: StatusIndicatorProps) {
  if (error) {
    return (
      <div className="flex items-start gap-1.5 px-3 py-1.5 text-xs text-red-400 bg-red-950/30 rounded">
        <span className="w-1.5 h-1.5 rounded-full bg-red-500 flex-shrink-0 mt-1" />
        <span className="break-words min-w-0">Error: {error}</span>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-blue-400 bg-blue-950/30 rounded">
        <span className="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse" />
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
