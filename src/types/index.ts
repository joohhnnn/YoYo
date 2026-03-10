export type BubbleState = "ambient" | "active" | "working" | "done";

export interface ActionParams {
  url?: string;
  app?: string;
  text?: string;
  command?: string;
  message?: string;
  prompt?: string;
  directory?: string;
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
  need_full_context?: boolean;
}

export interface PlanStep {
  action_type: string;
  label: string;
  params: ActionParams;
}

export interface IntentResult {
  understanding: string;
  plan: PlanStep[];
  needs_confirmation: boolean;
  workflow_id?: number;
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
  app_blacklist: string[];
  onboarding_completed: boolean;
  preferred_mic_device: string;
  sound_enabled: boolean;
  bubble_x: number | null;
  bubble_y: number | null;
  current_scene: string | null;
}

export interface AudioDevice {
  name: string;
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

export interface KnowledgeRecord {
  id: number;
  kind: "vocab" | "reading" | "concept";
  content: string;
  source: string;
  metadata: string;
  created_at: string;
}

export interface KnowledgeMetadata {
  definition?: string;
  review_count: number;
  interval_level: number;
  next_review: string | null;
  last_reviewed: string | null;
}

export interface KnowledgeStats {
  total: number;
  due: number;
}

export interface EditTrackingResult {
  found: boolean;
  reverted: boolean;
  reason?: string;
}

export interface WorkflowRecord {
  id: number;
  name: string;
  trigger_context: string;
  steps_json: string;
  success_count: number;
  fail_count: number;
  created_at: string;
  updated_at: string;
}

export interface ExecutionRecord {
  id: number;
  workflow_id: number | null;
  input_text: string | null;
  plan_json: string | null;
  result_json: string | null;
  status: string;
  user_feedback: string | null;
  created_at: string;
  completed_at: string | null;
}

