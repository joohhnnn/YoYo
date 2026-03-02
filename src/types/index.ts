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
}

export interface TaskItem {
  id: string;
  text: string;
  done: boolean;
}

export interface Settings {
  ai_mode: string;
  api_key: string;
  shortcut_toggle: string;
  shortcut_analyze: string;
  analysis_cooldown_secs: number;
}

export interface AppSwitchEvent {
  app_name: string;
  bundle_id: string;
}
