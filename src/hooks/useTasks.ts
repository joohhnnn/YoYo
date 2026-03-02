import { useState, useEffect, useCallback } from "react";
import { getTasks, saveTasks } from "../services/storage";
import type { TaskItem } from "../types";

export function useTasks() {
  const [tasks, setTasks] = useState<TaskItem[]>([]);

  useEffect(() => {
    getTasks().then(setTasks).catch(() => {});
  }, []);

  const addTask = useCallback(
    (text: string) => {
      const newTask: TaskItem = {
        id: crypto.randomUUID(),
        text,
        done: false,
      };
      const updated = [...tasks, newTask];
      setTasks(updated);
      saveTasks(updated).catch(() => {});
    },
    [tasks]
  );

  const toggleTask = useCallback(
    (id: string) => {
      const updated = tasks.map((t) =>
        t.id === id ? { ...t, done: !t.done } : t
      );
      setTasks(updated);
      saveTasks(updated).catch(() => {});
    },
    [tasks]
  );

  const removeTask = useCallback(
    (id: string) => {
      const updated = tasks.filter((t) => t.id !== id);
      setTasks(updated);
      saveTasks(updated).catch(() => {});
    },
    [tasks]
  );

  return { tasks, addTask, toggleTask, removeTask };
}
