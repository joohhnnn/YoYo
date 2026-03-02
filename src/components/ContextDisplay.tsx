interface ContextDisplayProps {
  context: string | null;
}

export function ContextDisplay({ context }: ContextDisplayProps) {
  return (
    <div className="px-3 py-2">
      <div className="text-[10px] uppercase tracking-wider text-zinc-500 mb-1">
        Current Context
      </div>
      <div className="text-sm text-zinc-200">
        {context || "Click analyze to understand your screen"}
      </div>
    </div>
  );
}
