import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ActivityRecord,
  WorkflowRecord,
  KnowledgeRecord,
  KnowledgeMetadata,
  KnowledgeStats,
} from "../types";

type Section = "activities" | "workflows" | "knowledge" | null;

export function HistoryPanel() {
  const [activities, setActivities] = useState<ActivityRecord[]>([]);
  const [workflows, setWorkflows] = useState<WorkflowRecord[]>([]);
  const [knowledge, setKnowledge] = useState<KnowledgeRecord[]>([]);
  const [stats, setStats] = useState<KnowledgeStats>({ total: 0, due: 0 });
  const [open, setOpen] = useState<Section>("activities");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = async () => {
      try {
        const [acts, wfs, vocab, ks] = await Promise.all([
          invoke<ActivityRecord[]>("get_recent_activities", { limit: 20 }),
          invoke<WorkflowRecord[]>("get_workflows"),
          invoke<KnowledgeRecord[]>("get_knowledge_by_kind", { kind: "vocab", limit: 50 }),
          invoke<KnowledgeStats>("get_knowledge_stats"),
        ]);
        setActivities(acts);
        setWorkflows(wfs);
        setKnowledge(vocab);
        setStats(ks);
      } catch (e) {
        console.error("Failed to load history:", e);
      } finally {
        setLoading(false);
      }
    };
    load();
  }, []);

  const toggle = (s: Section) => setOpen(open === s ? null : s);

  const handleDeleteWorkflow = async (id: number) => {
    setWorkflows((prev) => prev.filter((w) => w.id !== id));
    await invoke("delete_workflow", { id }).catch(() => {});
  };

  const handleDeleteKnowledge = async (id: number) => {
    setKnowledge((prev) => prev.filter((k) => k.id !== id));
    setStats((prev) => ({ ...prev, total: Math.max(0, prev.total - 1) }));
    await invoke("delete_knowledge", { id }).catch(() => {});
  };

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center text-zinc-500 text-[11px]">
        Loading...
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-3 py-2 space-y-1">
      {/* Activities */}
      <AccordionHeader
        title="Recent Activities"
        count={activities.length}
        isOpen={open === "activities"}
        onClick={() => toggle("activities")}
      />
      {open === "activities" && (
        <div className="space-y-0.5 max-h-[260px] overflow-y-auto">
          {activities.length === 0 ? (
            <EmptyState text="No activities yet" />
          ) : (
            activities.map((a) => (
              <div key={a.id} className="px-2 py-1.5 bg-zinc-800/50 rounded text-[10px]">
                <div className="flex items-center justify-between">
                  <span className="text-zinc-300 font-medium truncate max-w-[180px]">
                    {a.app_name || "Unknown"}
                  </span>
                  <span className="text-zinc-600 text-[9px]">{timeAgo(a.updated_at)}</span>
                </div>
                <p className="text-zinc-500 truncate mt-0.5">{a.context}</p>
              </div>
            ))
          )}
        </div>
      )}

      {/* Workflows */}
      <AccordionHeader
        title="Workflows"
        count={workflows.length}
        isOpen={open === "workflows"}
        onClick={() => toggle("workflows")}
      />
      {open === "workflows" && (
        <div className="space-y-0.5 max-h-[260px] overflow-y-auto">
          {workflows.length === 0 ? (
            <EmptyState text="No saved workflows yet" />
          ) : (
            workflows.map((w) => (
              <div
                key={w.id}
                className="px-2 py-1.5 bg-zinc-800/50 rounded text-[10px]
                  flex items-center justify-between group"
              >
                <div className="flex-1 min-w-0">
                  <span className="text-zinc-300 font-medium">{w.name}</span>
                  <span className="text-zinc-600 ml-1.5 text-[9px]">
                    {w.success_count}ok {w.fail_count}fail
                  </span>
                </div>
                <button
                  onClick={() => handleDeleteWorkflow(w.id)}
                  className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100
                    transition-opacity text-[12px] flex-shrink-0 ml-1"
                >
                  ×
                </button>
              </div>
            ))
          )}
        </div>
      )}

      {/* Knowledge */}
      <AccordionHeader
        title={`Knowledge${stats.due > 0 ? ` (${stats.due} due)` : ""}`}
        count={stats.total}
        isOpen={open === "knowledge"}
        onClick={() => toggle("knowledge")}
      />
      {open === "knowledge" && (
        <div className="space-y-0.5 max-h-[260px] overflow-y-auto">
          {knowledge.length === 0 ? (
            <EmptyState text="No knowledge items yet" />
          ) : (
            knowledge.map((k) => {
              const meta: KnowledgeMetadata = JSON.parse(k.metadata || "{}");
              return (
                <div
                  key={k.id}
                  className="px-2 py-1.5 bg-zinc-800/50 rounded text-[10px]
                    flex items-center justify-between group"
                >
                  <div className="flex-1 min-w-0">
                    <span className="text-zinc-300">{k.content}</span>
                    {meta.definition && (
                      <p className="text-zinc-600 truncate">{meta.definition}</p>
                    )}
                  </div>
                  <button
                    onClick={() => handleDeleteKnowledge(k.id)}
                    className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100
                      transition-opacity text-[12px] flex-shrink-0 ml-1"
                  >
                    ×
                  </button>
                </div>
              );
            })
          )}
        </div>
      )}
    </div>
  );
}

function AccordionHeader({
  title,
  count,
  isOpen,
  onClick,
}: {
  title: string;
  count: number;
  isOpen: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="w-full flex items-center justify-between px-2 py-1.5 rounded
        bg-zinc-800 hover:bg-zinc-700/80 transition-colors"
    >
      <span className="text-[10px] text-zinc-400 uppercase tracking-wider">
        {isOpen ? "\u25BE" : "\u25B8"} {title}
      </span>
      <span className="text-[9px] text-zinc-600">{count}</span>
    </button>
  );
}

function EmptyState({ text }: { text: string }) {
  return (
    <p className="text-[10px] text-zinc-600 py-3 text-center italic">{text}</p>
  );
}

function timeAgo(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr.replace(" ", "T")).getTime();
  const mins = Math.floor((now - then) / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return `${Math.floor(days / 7)}w ago`;
}
