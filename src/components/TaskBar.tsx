import { StatusIndicator } from "./StatusIndicator";
import { ContextDisplay } from "./ContextDisplay";
import { ActionButtons } from "./ActionButtons";
import { TaskList } from "./TaskList";
import { useScreenContext } from "../hooks/useScreenContext";
import { useActions } from "../hooks/useActions";
import { useTasks } from "../hooks/useTasks";

export function TaskBar() {
  const { result, loading, error, analyze } = useScreenContext();
  const { executing, execute } = useActions();
  const { tasks, addTask, toggleTask, removeTask } = useTasks();

  return (
    <div className="flex flex-col h-screen bg-zinc-900 text-white select-none">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
        <span className="text-sm font-semibold tracking-wide">YoYo</span>
        <button
          onClick={() => analyze(0)}
          disabled={loading}
          className="px-2.5 py-1 text-xs bg-blue-600 hover:bg-blue-500 rounded
            disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {loading ? "Analyzing..." : "Analyze"}
        </button>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        {/* Status */}
        <div className="px-3 py-1.5">
          <StatusIndicator loading={loading} error={error} />
        </div>

        {/* Context */}
        <ContextDisplay context={result?.context ?? null} />

        {/* Divider */}
        <div className="border-t border-zinc-800" />

        {/* Actions */}
        <ActionButtons
          actions={result?.actions ?? []}
          executing={executing}
          onExecute={execute}
        />

        {/* Divider */}
        <div className="border-t border-zinc-800" />

        {/* Tasks */}
        <TaskList
          tasks={tasks}
          onToggle={toggleTask}
          onAdd={addTask}
          onRemove={removeTask}
        />
      </div>
    </div>
  );
}
