# YoYo — Project Guide

macOS menu bar AI assistant. Captures screen, analyzes with Claude, suggests contextual next actions.

**Tech stack**: Tauri v2 (Rust) + React + TypeScript + Tailwind CSS + Claude CLI/API

## File Structure

```
src-tauri/src/
  main.rs            # Entry point → lib.rs::run()
  lib.rs             # Tauri setup, tray icon, panel toggle
  commands.rs        # All Tauri commands + Settings/Tasks persistence
  ai_engine.rs       # Claude CLI + API dual mode, prompt definition
  screenshot.rs      # macOS screencapture wrapper
  window_monitor.rs  # NSWorkspace app-switch listener
  user_data.rs       # ~/.yoyo/ profile, context, SQLite management

src/
  App.tsx            # Global shortcuts + app-switch event listener
  main.tsx           # React entry
  types/index.ts     # All TypeScript interfaces
  components/        # TaskBar, ContextDisplay, ActionButtons, TaskList, StatusIndicator
  hooks/             # useScreenContext, useActions, useTasks
  services/          # ai.ts, actions.ts, storage.ts, userdata.ts (Tauri invoke wrappers)
```

## Storage Architecture (3 layers)

| Layer | Format | Location | Purpose |
|-------|--------|----------|---------|
| User profile | `profile.md` | `~/.yoyo/profile.md` | Who the user is, preferences, tools |
| Dynamic context | `context.md` | `~/.yoyo/context.md` | Current projects, today's plan |
| Structured data | SQLite | `~/.yoyo/yoyo.db` | Vocab, learning progress, metrics |
| App settings | JSON | `~/Library/Application Support/com.yoyo.app/yoyo_data.json` | Settings + task list |

## How to Add a New Feature

### Step 1: Rust command

In `commands.rs`, add a function with `#[tauri::command]`:
```rust
#[tauri::command]
pub async fn my_new_command(app: AppHandle, param: String) -> Result<MyType, String> {
    // Return Result<T, String> — errors serialize to frontend
}
```

### Step 2: Register command

In `lib.rs`, add to `invoke_handler`:
```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    commands::my_new_command,
])
```

### Step 3: Frontend service

In `src/services/`, add or extend a file:
```typescript
import { invoke } from "@tauri-apps/api/core";

export async function myNewCommand(param: string): Promise<MyType> {
  return await invoke<MyType>("my_new_command", { param });
}
```

### Step 4: Hook (if state management needed)

In `src/hooks/`, create `useMyFeature.ts`:
```typescript
export function useMyFeature() {
  const [data, setData] = useState<MyType | null>(null);
  // Load, mutate, persist pattern (see useTasks.ts for reference)
  return { data, ... };
}
```

### Step 5: Component

In `src/components/`, add to the TaskBar scrollable content area.

## How to Add a New Action Type

1. **Prompt**: Update `ANALYSIS_PROMPT` in `ai_engine.rs` — add the new type to the list
2. **Rust**: Add a match arm in `commands.rs::execute_action`
3. **Frontend**: Add icon mapping in `ActionButtons.tsx::ACTION_ICONS`
4. **Types**: Update `ActionParams` in both `ai_engine.rs` and `src/types/index.ts`

## How to Add a New SQLite Table

1. Add `CREATE TABLE IF NOT EXISTS` to `user_data.rs::init_db()`
2. Add Rust query/insert functions in `user_data.rs`
3. Wrap in Tauri commands in `commands.rs`
4. Register in `lib.rs`
5. Add frontend service in `src/services/userdata.ts`

## Prompt Engineering

The analysis prompt is `ANALYSIS_PROMPT` in `ai_engine.rs`. Both CLI and API modes share it.

User context is injected before the prompt:
```
[User Profile from ~/.yoyo/profile.md]
[Dynamic Context from ~/.yoyo/context.md]
[ANALYSIS_PROMPT]
```

When modifying the prompt:
- Keep JSON output format strict — frontend parses it directly
- Action types must match the 5 (or more) types in `execute_action`
- Context should be 1 sentence, actions 2-4 items

## Code Conventions

- **Rust**: snake_case, `Result<T, String>` for all commands, `#[tauri::command]` attribute
- **TypeScript**: camelCase, services wrap `invoke()` with `assertTauri()` guard
- **Errors**: Rust formats to String, frontend catches and displays in StatusIndicator
- **Persistence**: optimistic update (set state first, then async save)
- **Events**: Rust emits Tauri events → App.tsx converts to CustomEvent → hooks listen

## Security Rules

- `open_app`: sanitize app name (alphanumeric + space/dot/hyphen only)
- `run_command`: blacklist dangerous patterns (rm -rf, sudo, mkfs, etc.)
- `notify`: strip quotes and backslashes from message (AppleScript injection)
- Never expose API keys in frontend — stored in Rust-side settings only
- All new action types must validate parameters before execution

## Key Constants

- Panel size: 320 x 560px
- Shortcuts: Cmd+Shift+Y (toggle), Cmd+Shift+R (analyze)
- App switch debounce: 2s delay + 10s cooldown
- Tray events: handle MouseButtonState::Down only (prevent double-fire)
- Keyboard shortcuts: handle "Pressed" state only (same reason)
