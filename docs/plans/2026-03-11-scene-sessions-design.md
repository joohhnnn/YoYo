# Scene Sessions & Observation Mode

## Context

YoYo currently has scene declaration (`current_scene` in settings) but no history tracking. Auto-analysis always shows the bubble regardless of whether a scene is set. Users want:
1. **Observation mode**: When no scene is declared, YoYo analyzes silently (no bubble popup)
2. **Scene timeline**: Record when each scene starts/ends for learning assessment
3. **Startup prompt**: Ask "what are you doing?" when YoYo starts, skip = observation mode

## Design: Scene Session System

### Core Rule

- `current_scene != null` → **Normal mode**: analyze + show bubble + suggestions
- `current_scene == null` → **Observation mode**: analyze + record activity, but no bubble popup

### Scene Sessions Table

```sql
CREATE TABLE scene_sessions (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    scene_name  TEXT,              -- NULL = observation mode
    started_at  TEXT DEFAULT (datetime('now','localtime')),
    ended_at    TEXT                -- NULL = currently active
);
```

Timeline example:
```
id | scene_name      | started_at       | ended_at
1  | Code Review     | 09:00            | 12:15
2  | NULL            | 12:15            | 13:00    ← observation
3  | 学习 reth 源码   | 13:00            | NULL     ← active
```

### Behavior Changes

**set_scene() modification**: When scene changes, close the previous session (set `ended_at`) and open a new one. This happens for ALL transitions including to/from NULL.

**Auto-analysis (lib.rs)**: After `do_analyze()` succeeds, check `current_scene`:
- Scene set → emit "analysis-complete", show bubble, play sound (current behavior)
- No scene → skip emit, skip bubble, skip sound. Still record activity + trigger summarization.

**Startup flow (BubbleApp.tsx)**: On first mount, if `current_scene == null`, auto-expand the scene picker with a "Skip (observe)" option. User can pick a scene or skip into observation mode.

### Changes

1. **user_data.rs**: Migration 4 for `scene_sessions` table + CRUD functions
2. **commands/settings.rs**: `set_scene()` now also creates/closes scene sessions
3. **lib.rs**: Gate bubble show/emit behind `current_scene.is_some()`
4. **BubbleApp.tsx**: Auto-show scene picker on startup when no scene set, add "Skip" option

### Verification

1. `cargo check`, `cargo fmt --check`
2. `npx tsc --noEmit`, `npx vitest run`
3. Manual: set scene → verify session recorded, clear → verify ended_at set, startup without scene → verify picker shows
