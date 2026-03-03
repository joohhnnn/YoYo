import type { SuggestedAction } from "../types";

// SF Symbols-style SVG icons for a native macOS feel
function ActionIcon({ type }: { type: string }) {
  const iconMap: Record<string, JSX.Element> = {
    open_url: (
      <svg viewBox="0 0 16 16" fill="none" className="w-4 h-4">
        <path d="M6 3.5h-2.5a1 1 0 00-1 1v8a1 1 0 001 1h8a1 1 0 001-1v-2.5" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/>
        <path d="M10 2.5h3.5v3.5M13.5 2.5L7 9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"/>
      </svg>
    ),
    open_app: (
      <svg viewBox="0 0 16 16" fill="none" className="w-4 h-4">
        <rect x="2" y="3" width="12" height="10" rx="1.5" stroke="currentColor" strokeWidth="1.3"/>
        <path d="M2 6h12" stroke="currentColor" strokeWidth="1.3"/>
        <circle cx="4" cy="4.5" r="0.5" fill="currentColor"/>
        <circle cx="5.5" cy="4.5" r="0.5" fill="currentColor"/>
        <circle cx="7" cy="4.5" r="0.5" fill="currentColor"/>
      </svg>
    ),
    copy_to_clipboard: (
      <svg viewBox="0 0 16 16" fill="none" className="w-4 h-4">
        <rect x="5" y="5" width="8" height="9" rx="1" stroke="currentColor" strokeWidth="1.3"/>
        <path d="M3 11V3a1 1 0 011-1h6" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/>
      </svg>
    ),
    run_command: (
      <svg viewBox="0 0 16 16" fill="none" className="w-4 h-4">
        <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" stroke="currentColor" strokeWidth="1.3"/>
        <path d="M4.5 7.5l2.5 2-2.5 2" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round"/>
        <path d="M8.5 11.5h3" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round"/>
      </svg>
    ),
    notify: (
      <svg viewBox="0 0 16 16" fill="none" className="w-4 h-4">
        <path d="M8 2a4 4 0 00-4 4v2l-1.5 2.5h11L12 8V6a4 4 0 00-4-4z" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round"/>
        <path d="M6.5 12.5a1.5 1.5 0 003 0" stroke="currentColor" strokeWidth="1.3"/>
      </svg>
    ),
  };

  return (
    <span className="text-zinc-400 flex-shrink-0">
      {iconMap[type] || iconMap.notify}
    </span>
  );
}

interface ActionButtonsProps {
  actions: SuggestedAction[];
  executing: string | null;
  onExecute: (action: SuggestedAction) => void;
  compact?: boolean;
}

export function ActionButtons({
  actions,
  executing,
  onExecute,
  compact = false,
}: ActionButtonsProps) {
  if (actions.length === 0) {
    return (
      <div className="px-4 py-3 text-xs text-zinc-500 text-center">
        No actions suggested yet
      </div>
    );
  }

  return (
    <div className={compact ? "py-1" : "px-3 py-2 space-y-1.5"}>
      {!compact && (
        <div className="text-[10px] uppercase tracking-wider text-zinc-500 mb-1">
          Suggested Actions
        </div>
      )}
      {actions.map((action, i) => {
        const isExecuting = executing === action.label;
        return (
          <button
            key={i}
            onClick={() => onExecute(action)}
            disabled={isExecuting}
            className={`w-full flex items-center gap-3 text-[13px] text-left
              transition-all duration-150
              disabled:opacity-50 disabled:cursor-not-allowed
              ${compact
                ? "px-4 py-2.5 hover:bg-white/[0.06] active:bg-white/[0.1]"
                : "px-3 py-2 bg-zinc-800 hover:bg-zinc-700 rounded-lg border border-zinc-700 hover:border-zinc-600"
              }`}
          >
            <ActionIcon type={action.type} />
            <span className="flex-1 text-zinc-200 truncate" title={action.label}>{action.label}</span>
            {isExecuting ? (
              <span className="w-3.5 h-3.5 border-[1.5px] border-zinc-400 border-t-transparent rounded-full animate-spin flex-shrink-0" />
            ) : (
              <svg viewBox="0 0 16 16" fill="none" className="w-3 h-3 text-zinc-600 flex-shrink-0">
                <path d="M6 3l5 5-5 5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            )}
          </button>
        );
      })}
    </div>
  );
}
