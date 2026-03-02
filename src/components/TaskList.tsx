import { useState } from "react";
import type { TaskItem } from "../types";

interface TaskListProps {
  tasks: TaskItem[];
  onToggle: (id: string) => void;
  onAdd: (text: string) => void;
  onRemove: (id: string) => void;
}

export function TaskList({ tasks, onToggle, onAdd, onRemove }: TaskListProps) {
  const [input, setInput] = useState("");

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = input.trim();
    if (trimmed) {
      onAdd(trimmed);
      setInput("");
    }
  };

  return (
    <div className="px-3 py-2">
      <div className="text-[10px] uppercase tracking-wider text-zinc-500 mb-1.5">
        Tasks
      </div>
      <div className="space-y-1 max-h-40 overflow-y-auto">
        {tasks.map((task) => (
          <div
            key={task.id}
            className="flex items-center gap-2 group text-sm"
          >
            <button
              onClick={() => onToggle(task.id)}
              className={`w-4 h-4 rounded border flex-shrink-0 flex items-center justify-center
                ${
                  task.done
                    ? "bg-green-600 border-green-500"
                    : "border-zinc-600 hover:border-zinc-400"
                }`}
            >
              {task.done && (
                <svg
                  className="w-3 h-3 text-white"
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
                task.done ? "line-through text-zinc-500" : "text-zinc-300"
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
      <form onSubmit={handleSubmit} className="mt-2">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          placeholder="Add a task..."
          className="w-full px-2 py-1.5 text-sm bg-zinc-800 border border-zinc-700 rounded
            text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-500"
        />
      </form>
    </div>
  );
}
