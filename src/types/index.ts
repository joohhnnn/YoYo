export interface ActionParams {
  url?: string;
  app?: string;
  text?: string;
  command?: string;
  message?: string;
}

export interface SuggestedAction {
  type: string;
  label: string;
  params: ActionParams;
}

export interface AnalysisResult {
  context: string;
  actions: SuggestedAction[];
  suggested_quest?: string;
  key_concepts?: string[];
  need_full_context?: boolean;
  on_track?: boolean;
  drift_message?: string;
}

export interface TaskItem {
  id: string;
  text: string;
  done: boolean;
  quest_type: "main" | "side";
  progress?: number;
  target?: number;
}

export interface Settings {
  ai_mode: string;
  api_key: string;
  model: string;
  shortcut_toggle: string;
  shortcut_analyze: string;
  analysis_cooldown_secs: number;
  bubble_opacity: number;
  language: string;
  auto_analyze: boolean;
  analysis_depth: "casual" | "normal" | "deep";
  scene_mode: "general" | "learning" | "working";
  obsidian_enabled: boolean;
  obsidian_vault_path: string;
}

export interface AppSwitchEvent {
  app_name: string;
  bundle_id: string;
}

export interface ChatMessage {
  role: "user" | "assistant";
  content: string;
}

export interface ActivityRecord {
  id: number;
  app_name: string;
  bundle_id: string;
  context: string;
  created_at: string;
  updated_at: string;
}

// Session types
export interface Session {
  id: string;
  goal: string;
  started_at: string;
  ended_at?: string;
  summary?: string;
  status: string;
}

export interface TimelineEntry {
  id: number;
  session_id: string;
  timestamp: string;
  context: string;
  app_name: string;
}

export interface SessionSummary {
  session: Session;
  timeline: TimelineEntry[];
}

export interface SpeechBubbleEvent {
  text: string;
  auto_dismiss_secs: number;
}

export interface SessionDriftEvent {
  message: string;
}
