import { invoke } from "@tauri-apps/api/core";
import type { Session, SessionSummary, TimelineEntry } from "../types";

export async function startSession(goal: string): Promise<Session> {
  return await invoke<Session>("start_session", { goal });
}

export async function endSession(): Promise<SessionSummary> {
  return await invoke<SessionSummary>("end_session");
}

export async function getActiveSession(): Promise<Session | null> {
  return await invoke<Session | null>("get_active_session");
}

export async function getSessionHistory(
  limit: number = 10
): Promise<Session[]> {
  return await invoke<Session[]>("get_session_history", { limit });
}

export async function getSessionTimeline(
  sessionId: string
): Promise<TimelineEntry[]> {
  return await invoke<TimelineEntry[]>("get_session_timeline", { sessionId });
}

export async function sendSessionMessage(message: string): Promise<string> {
  return await invoke<string>("send_session_message", { message });
}
