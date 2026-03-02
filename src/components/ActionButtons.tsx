import type { SuggestedAction } from "../types";

const ACTION_ICONS: Record<string, string> = {
  open_url: "\u{1F310}",
  open_app: "\u{1F4BB}",
  copy_to_clipboard: "\u{1F4CB}",
  run_command: "\u{2699}\u{FE0F}",
  notify: "\u{1F514}",
};

interface ActionButtonsProps {
  actions: SuggestedAction[];
  executing: string | null;
  onExecute: (action: SuggestedAction) => void;
}

export function ActionButtons({
  actions,
  executing,
  onExecute,
}: ActionButtonsProps) {
  if (actions.length === 0) {
    return (
      <div className="px-3 py-3 text-xs text-zinc-500 text-center">
        No actions suggested yet
      </div>
    );
  }

  return (
    <div className="px-3 py-2 space-y-1.5">
      <div className="text-[10px] uppercase tracking-wider text-zinc-500 mb-1">
        Suggested Actions
      </div>
      {actions.map((action, i) => {
        const isExecuting = executing === action.label;
        return (
          <button
            key={i}
            onClick={() => onExecute(action)}
            disabled={isExecuting}
            className="w-full flex items-center gap-2 px-3 py-2 text-sm text-left
              bg-zinc-800 hover:bg-zinc-700 rounded-lg transition-colors
              disabled:opacity-50 disabled:cursor-not-allowed
              border border-zinc-700 hover:border-zinc-600"
          >
            <span className="text-base">
              {ACTION_ICONS[action.type] || "\u{26A1}"}
            </span>
            <span className="flex-1 text-zinc-200">{action.label}</span>
            {isExecuting && (
              <span className="w-3 h-3 border border-zinc-400 border-t-transparent rounded-full animate-spin" />
            )}
          </button>
        );
      })}
    </div>
  );
}
