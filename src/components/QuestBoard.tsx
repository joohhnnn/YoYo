import { useState } from "react";
import type { TaskItem } from "../types";

interface QuestBoardProps {
  tasks: TaskItem[];
  onToggle: (id: string) => void;
  onAdd: (text: string, questType: "main" | "side", target?: number) => void;
  onRemove: (id: string) => void;
  onUpdateProgress: (id: string, progress: number) => void;
}

export function QuestBoard({
  tasks,
  onToggle,
  onAdd,
  onRemove,
  onUpdateProgress,
}: QuestBoardProps) {
  const mainQuest = tasks.find((t) => t.quest_type === "main" && !t.done);
  const sideQuests = tasks.filter(
    (t) => t.quest_type === "side" || (t.quest_type === "main" && t.done)
  );

  return (
    <div className="px-3 py-2 space-y-3">
      {/* Main Quest */}
      <MainQuestCard
        quest={mainQuest}
        onToggle={onToggle}
        onRemove={onRemove}
        onUpdateProgress={onUpdateProgress}
        onAdd={onAdd}
      />

      {/* Side Quests */}
      <SideQuestList
        quests={sideQuests}
        onToggle={onToggle}
        onRemove={onRemove}
        onAdd={onAdd}
      />
    </div>
  );
}

function MainQuestCard({
  quest,
  onToggle,
  onRemove,
  onUpdateProgress,
  onAdd,
}: {
  quest: TaskItem | undefined;
  onToggle: (id: string) => void;
  onRemove: (id: string) => void;
  onUpdateProgress: (id: string, progress: number) => void;
  onAdd: (text: string, questType: "main" | "side", target?: number) => void;
}) {
  const [adding, setAdding] = useState(false);
  const [input, setInput] = useState("");
  const [targetInput, setTargetInput] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const text = input.trim();
    if (!text) return;
    const target = targetInput.trim()
      ? parseInt(targetInput.trim(), 10)
      : undefined;
    onAdd(text, "main", target && target > 0 ? target : undefined);
    setInput("");
    setTargetInput("");
    setAdding(false);
  };

  return (
    <div>
      <div className="text-[10px] uppercase tracking-wider text-amber-500/80 mb-1.5 flex items-center gap-1">
        <svg viewBox="0 0 12 12" className="w-3 h-3" fill="currentColor">
          <path d="M6 1l1.5 3.2L11 4.7 8.5 7.1l.6 3.4L6 8.8 2.9 10.5l.6-3.4L1 4.7l3.5-.5z" />
        </svg>
        Main Quest
      </div>

      {quest ? (
        <div className="bg-amber-500/[0.08] border border-amber-500/20 rounded-lg px-3 py-2 group">
          <div className="flex items-center gap-2">
            <button
              onClick={() => onToggle(quest.id)}
              className="w-4 h-4 rounded border border-amber-500/40 flex-shrink-0 flex items-center justify-center
                hover:border-amber-400"
            />
            <span className="flex-1 text-[13px] text-zinc-200 font-medium">
              {quest.text}
            </span>
            <button
              onClick={() => onRemove(quest.id)}
              className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-red-400 text-xs"
            >
              x
            </button>
          </div>

          {/* Progress bar */}
          {quest.target !== undefined && (
            <div className="mt-2">
              <div className="flex items-center gap-2">
                <div className="flex-1 h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-amber-500 rounded-full transition-all"
                    style={{
                      width: `${Math.min(
                        100,
                        ((quest.progress ?? 0) / quest.target) * 100
                      )}%`,
                    }}
                  />
                </div>
                <span className="text-[10px] text-zinc-400 tabular-nums">
                  {quest.progress ?? 0}/{quest.target}
                </span>
              </div>
              <div className="flex gap-1 mt-1.5">
                <button
                  onClick={() =>
                    onUpdateProgress(
                      quest.id,
                      Math.max(0, (quest.progress ?? 0) - 1)
                    )
                  }
                  className="text-[10px] px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                >
                  -1
                </button>
                <button
                  onClick={() =>
                    onUpdateProgress(quest.id, (quest.progress ?? 0) + 1)
                  }
                  className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/20 text-amber-400 hover:bg-amber-500/30"
                >
                  +1
                </button>
              </div>
            </div>
          )}
        </div>
      ) : adding ? (
        <form onSubmit={handleSubmit} className="space-y-1.5">
          <input
            autoFocus
            value={input}
            onChange={(e) => setInput(e.target.value)}
            placeholder="What's today's main quest?"
            className="w-full px-2 py-1.5 text-[12px] bg-zinc-800 border border-amber-500/30 rounded
              text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-amber-500/50"
          />
          <div className="flex gap-1.5 items-center">
            <input
              value={targetInput}
              onChange={(e) => setTargetInput(e.target.value)}
              placeholder="Target (optional)"
              type="number"
              min="1"
              className="flex-1 px-2 py-1 text-[11px] bg-zinc-800 border border-zinc-700 rounded
                text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500
                [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none
                [&::-webkit-inner-spin-button]:appearance-none"
            />
            <button
              type="submit"
              className="px-2 py-1 text-[11px] bg-amber-600 hover:bg-amber-500 rounded text-white"
            >
              Set
            </button>
            <button
              type="button"
              onClick={() => setAdding(false)}
              className="px-2 py-1 text-[11px] bg-zinc-800 hover:bg-zinc-700 rounded text-zinc-400"
            >
              Cancel
            </button>
          </div>
        </form>
      ) : (
        <button
          onClick={() => setAdding(true)}
          className="w-full py-2 text-[11px] text-zinc-500 hover:text-amber-400 border border-dashed
            border-zinc-700 hover:border-amber-500/30 rounded-lg transition-colors"
        >
          + Set main quest
        </button>
      )}
    </div>
  );
}

function SideQuestList({
  quests,
  onToggle,
  onRemove,
  onAdd,
}: {
  quests: TaskItem[];
  onToggle: (id: string) => void;
  onRemove: (id: string) => void;
  onAdd: (text: string, questType: "main" | "side", target?: number) => void;
}) {
  const [input, setInput] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const text = input.trim();
    if (text) {
      onAdd(text, "side");
      setInput("");
    }
  };

  return (
    <div>
      <div className="text-[10px] uppercase tracking-wider text-zinc-500 mb-1.5">
        Side Quests
      </div>
      <div className="space-y-1 max-h-32 overflow-y-auto">
        {quests.map((task) => (
          <div
            key={task.id}
            className="flex items-center gap-2 group text-[13px]"
          >
            <button
              onClick={() => onToggle(task.id)}
              className={`w-3.5 h-3.5 rounded border flex-shrink-0 flex items-center justify-center
                ${
                  task.done
                    ? "bg-emerald-600 border-emerald-500"
                    : "border-zinc-600 hover:border-zinc-400"
                }`}
            >
              {task.done && (
                <svg
                  className="w-2.5 h-2.5 text-white"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={3}
                    d="M5 13l4 4L19 7"
                  />
                </svg>
              )}
            </button>
            <span
              className={`flex-1 ${
                task.done ? "line-through text-zinc-600" : "text-zinc-300"
              }`}
            >
              {task.text}
            </span>
            <button
              onClick={() => onRemove(task.id)}
              className="opacity-0 group-hover:opacity-100 text-zinc-500 hover:text-red-400 text-xs"
            >
              x
            </button>
          </div>
        ))}
      </div>
      <form onSubmit={handleSubmit} className="mt-1.5">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Add side quest..."
          className="w-full px-2 py-1 text-[12px] bg-zinc-800 border border-zinc-700 rounded
            text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500"
        />
      </form>
    </div>
  );
}
