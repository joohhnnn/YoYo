import { useState, useEffect, useCallback } from "react";
import { emit } from "@tauri-apps/api/event";
import { getTasks, saveTasks } from "../services/storage";
import type { TaskItem } from "../types";

export function useTasks() {
  const [tasks, setTasks] = useState<TaskItem[]>([]);

  useEffect(() => {
    getTasks().then(setTasks).catch(() => {});
  }, []);

  const persist = useCallback((updated: TaskItem[]) => {
    setTasks(updated);
    saveTasks(updated).catch(() => {});
    emit("tasks-changed").catch(() => {});
  }, []);

  const addTask = useCallback(
    (text: string, questType: "main" | "side" = "side", target?: number) => {
      // If adding a main quest, demote existing main quests to side
      let current = tasks;
      if (questType === "main") {
        current = tasks.map((t) =>
          t.quest_type === "main" ? { ...t, quest_type: "side" as const } : t
        );
      }
      const newTask: TaskItem = {
        id: crypto.randomUUID(),
        text,
        done: false,
        quest_type: questType,
        progress: target !== undefined ? 0 : undefined,
        target,
      };
      const updated = [...current, newTask];
      persist(updated);
    },
    [tasks, persist]
  );

  const updateProgress = useCallback(
    (id: string, progress: number) => {
      const updated = tasks.map((t) => {
        if (t.id !== id) return t;
        const done = t.target !== undefined && progress >= t.target;
        return { ...t, progress, done };
      });
      persist(updated);
    },
    [tasks, persist]
  );

  const toggleTask = useCallback(
    (id: string) => {
      const updated = tasks.map((t) =>
        t.id === id ? { ...t, done: !t.done } : t
      );
      persist(updated);
    },
    [tasks, persist]
  );

  const removeTask = useCallback(
    (id: string) => {
      const updated = tasks.filter((t) => t.id !== id);
      persist(updated);
    },
    [tasks, persist]
  );

  return { tasks, addTask, toggleTask, removeTask, updateProgress };
}
