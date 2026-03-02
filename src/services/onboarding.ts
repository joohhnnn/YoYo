import { invoke } from "@tauri-apps/api/core";
import type { ChatMessage } from "../types";

export async function checkNeedsOnboarding(): Promise<boolean> {
  return await invoke<boolean>("check_needs_onboarding");
}

export async function startOnboarding(): Promise<ChatMessage> {
  return await invoke<ChatMessage>("start_onboarding");
}

export async function sendOnboardingMessage(
  message: string
): Promise<ChatMessage> {
  return await invoke<ChatMessage>("send_onboarding_message", { message });
}

export async function finishOnboarding(): Promise<void> {
  await invoke("finish_onboarding");
}
