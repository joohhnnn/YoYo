import { invoke } from "@tauri-apps/api/core";

function assertTauri(): void {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error("Not running in Tauri — please use `npm run tauri dev`");
  }
}

export async function getProfile(): Promise<string> {
  assertTauri();
  return await invoke<string>("get_profile");
}

export async function saveProfile(content: string): Promise<void> {
  assertTauri();
  await invoke("save_profile", { content });
}

export async function getContext(): Promise<string> {
  assertTauri();
  return await invoke<string>("get_context");
}

export async function saveContext(content: string): Promise<void> {
  assertTauri();
  await invoke("save_context", { content });
}
