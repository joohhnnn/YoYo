import { invoke } from "@tauri-apps/api/core";
import type { SuggestedAction } from "../types";

function assertTauri(): void {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error("Not running in Tauri — please use `npm run tauri dev`");
  }
}

export async function executeAction(action: SuggestedAction): Promise<void> {
  assertTauri();
  await invoke("execute_action", {
    actionType: action.type,
    params: action.params,
  });
}
