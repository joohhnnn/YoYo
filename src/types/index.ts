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
  analysis_depth: string; // "casual" | "normal" | "deep"
  scene_mode: string; // "general" | "learning" | "working"
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
