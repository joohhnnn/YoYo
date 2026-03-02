import { invoke } from "@tauri-apps/api/core";
import type { AnalysisResult } from "../types";

function assertTauri(): void {
  if (!window.__TAURI_INTERNALS__) {
    throw new Error("Not running in Tauri — please use `npm run tauri dev`");
  }
}

export async function analyzeScreen(): Promise<AnalysisResult> {
  assertTauri();
  return await invoke<AnalysisResult>("analyze_screen");
}

export async function takeScreenshot(): Promise<string> {
  assertTauri();
  return await invoke<string>("take_screenshot");
}
