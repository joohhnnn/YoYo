import { invoke } from "@tauri-apps/api/core";
import type { Settings, TaskItem } from "../types";

function assertTauri(): void {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error("Not running in Tauri — please use `npm run tauri dev`");
  }
}

export async function getSettings(): Promise<Settings> {
  assertTauri();
  return await invoke<Settings>("get_settings");
}

export async function saveSettings(settings: Settings): Promise<void> {
  assertTauri();
  await invoke("save_settings", { settings });
}

export async function getTasks(): Promise<TaskItem[]> {
  assertTauri();
  return await invoke<TaskItem[]>("get_tasks");
}

export async function saveTasks(tasks: TaskItem[]): Promise<void> {
  assertTauri();
  await invoke("save_tasks", { tasks });
}
