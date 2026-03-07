# Session-Driven YoYo Redesign

## Overview

Transform YoYo from passive screen monitoring to an interactive, session-based workflow assistant. Users declare goals, YoYo monitors with context, provides relevant suggestions, and generates session summaries on completion.

## Core Concept

YoYo has two states: **Idle** and **In Session**.

- **Idle**: Shows session history, AI proactively suggests next session based on patterns, input box to start new session
- **In Session**: Dashboard with timer, goal, timeline of activities, input box for interaction, AI speech bubbles for proactive messages

## 1. Idle State UI

```
┌──────────────────────┐
│ YoYo            [⚙]  │
├──────────────────────┤
│                      │
│ 💬 "要不要继续昨天的  │  ← AI recommendation (clickable to start)
│    login 功能？"      │
│                      │
│ Recent Sessions:     │
│ · login功能    1.5h  │  ← Click to restart similar session
│ · 学Rust异步   2h    │
│ · 写文档       45m   │
│                      │
├──────────────────────┤
│ [________________]   │  ← Input: type goal + Enter to start
│  输入目标开始...      │
└──────────────────────┘
```

### Idle Behavior

- Show last 5 completed sessions with duration
- AI recommendation: based on recent activity patterns + time of day, suggest a likely next session
- Click recommendation or history item → pre-fill input → user confirms with Enter
- Input box: type freely ("实现 login 功能") → Enter → session starts
- Background: lightweight app-switch logging only (record app_name to activity_log, no screenshot/analysis)

## 2. In-Session State UI

```
┌──────────────────────┐
│ ⏱ 1h23m       [End] │  ← Timer + end button
│ 实现 login 功能       │  ← Session goal
├──────────────────────┤
│ 14:00 查auth文档      │  ← Timeline entries (scrollable)
│   Safari · jwt.io    │
│ 14:30 写auth中间件    │
│   VS Code · auth.rs  │
│ 15:02 调试测试失败    │
│   Terminal · cargo    │
│                      │
├──────────────────────┤
│ [________________]   │  ← Input: talk to YoYo
│  跟 YoYo 说点什么    │
└──────────────────────┘
```

### In-Session Behavior

- Timer counts up from session start
- Each analysis result appends a timeline entry (timestamp + context + app)
- Monitoring: same as current app-switch triggered analysis, but prompt includes session goal + timeline for much better context
- User can type in input box to interact (questions, notes, manual updates)
- AI responses appear as speech bubbles (see section 3)
- End button → trigger session summary → return to idle

## 3. Speech Bubbles

When AI needs to communicate (response to input, drift detection, proactive insight):

- **Implementation**: Separate Tauri webview window, transparent background, no focus steal
- **Position**: Adjacent to BubbleApp (left side or below)
- **Style**: Rounded card with small triangle pointer toward BubbleApp
- **Behavior**: Auto-fade after 8 seconds, or click to dismiss immediately
- **Content**: 1-3 sentences max, concise and actionable

### Bubble Triggers

| Trigger | Example |
|---------|---------|
| User input response | "JWT refresh token 一般用 HttpOnly cookie 存储" |
| Drift detection | "你在刷 Twitter，要回到 login 功能吗？" |
| Stuck detection | "你在 auth middleware 上卡了 20 分钟，要换个思路吗？" |
| Session suggestion (idle) | "下午好，要继续昨天的 login 功能吗？" |

## 4. Data Model

### New SQLite Tables

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    goal TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    summary TEXT,
    status TEXT DEFAULT 'active'  -- 'active' | 'completed' | 'abandoned'
);

CREATE TABLE session_timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    context TEXT NOT NULL,
    app_name TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
```

### Rust State

```rust
// Add to AppState
pub active_session: Mutex<Option<Session>>,
```

```rust
pub struct Session {
    pub id: String,
    pub goal: String,
    pub started_at: String,
    pub timeline: Vec<TimelineEntry>,
}

pub struct TimelineEntry {
    pub timestamp: String,
    pub context: String,
    pub app_name: String,
}
```

## 5. Analysis Changes

### Prompt Injection

When a session is active, inject into the AI prompt:

```
[Active Session]
Goal: 实现 login 功能
Duration: 1h23m
Recent timeline:
- 14:00 查auth文档 (Safari)
- 14:30 写auth中间件 (VS Code)
- 15:02 调试测试失败 (Terminal)

Analyze the user's current screen in the context of this goal.
- Is the user on-track or drifting from the goal?
- What specific suggestion would help them progress?
```

### New Response Fields

```json
{
  "context": "...",
  "actions": [...],
  "on_track": true,
  "drift_message": "你好像在看无关的网页，要回到 login 吗？",
  "key_concepts": [...]
}
```

- `on_track`: boolean — is current activity related to session goal?
- `drift_message`: string — only present when on_track is false

### Drift → Speech Bubble

When `on_track` is false:
1. Emit `"session-drift"` event with `drift_message`
2. Frontend shows speech bubble with the message
3. Include options: "继续当前" / "回到目标" / "结束 Session"

## 6. Session Lifecycle

```
Idle
  ↓ User types goal + Enter (or clicks suggestion/history)
  ↓
Start Session
  → Create session record in SQLite
  → Set AppState.active_session
  → Emit "session-started" event
  → UI switches to in-session mode
  → Begin app-switch monitoring with session context
  ↓
In Session (loop)
  → App switch triggers do_analyze()
  → do_analyze() injects session goal + timeline into prompt
  → AI returns analysis with on_track field
  → Append timeline entry
  → If !on_track → show drift bubble
  → User can type in input box → AI responds via bubble
  ↓
End Session (manual click or drift timeout)
  → AI generates session summary (what was done, blockers, next steps)
  → Save summary to sessions table
  → Sync to Obsidian if enabled
  → Clear AppState.active_session
  → Emit "session-ended" event
  → UI switches to idle mode
  ↓
Idle (with updated history)
```

## 7. Idle Recommendations

When no session is active, YoYo periodically (every 30 min or on app switch) checks:
1. Recent session history (what did user work on recently?)
2. Time of day patterns (user usually codes in the morning?)
3. Incomplete sessions (abandoned sessions that could be resumed?)

Generates a suggestion like "要不要继续昨天的 login 功能？" and shows it in the idle UI. This replaces the current `suggested_quest` mechanism.

## 8. Relationship to Existing Features

| Current Feature | Change |
|----------------|--------|
| Main Quest | **Replaced by** Session Goal — one active session = one goal |
| Side Quest | **Keep** as sub-tasks within a session |
| QuestBoard | **Replace with** Session Dashboard (timer + timeline + input) |
| do_analyze() | **Enhanced** — same pipeline but with session context injection |
| app-switch trigger | **Keep** — still triggers analysis during sessions |
| Reflection | **Replace with** Session Summary (generated on session end) |
| scene_mode | **Simplify** — session goal provides better context than scene mode |
| BubbleApp | **Redesign** — becomes session dashboard + speech bubble launcher |
| TrayApp | **Redesign** — becomes idle/session toggle with input box |
| Obsidian sync | **Keep** — session summaries sync instead of reflections |
| StatusIndicator | **Keep** — shows analysis status |
| Settings | **Keep** — minimal changes |

## 9. New Tauri Commands

```rust
start_session(goal: String) -> Result<Session, String>
end_session() -> Result<SessionSummary, String>
get_active_session() -> Result<Option<Session>, String>
get_session_history(limit: u32) -> Result<Vec<SessionRecord>, String>
send_session_message(message: String) -> Result<String, String>
get_session_recommendation() -> Result<Option<String>, String>
```

## 10. New Events

| Event | Payload | When |
|-------|---------|------|
| `session-started` | Session | User starts a session |
| `session-ended` | SessionSummary | Session ends |
| `session-timeline-update` | TimelineEntry | New analysis during session |
| `session-drift` | { message: String } | AI detects user drifting |
| `speech-bubble` | { text: String, auto_dismiss_secs: u8 } | Show speech bubble |

## 11. Speech Bubble Window

A third Tauri webview window (alongside TrayApp and BubbleApp):

- **Label**: `"speech-bubble"`
- **Size**: ~280 x auto-height (max 120px)
- **Position**: Adjacent to BubbleApp (left side, vertically centered)
- **Decorations**: false (no title bar)
- **Transparent**: true
- **Always on top**: true
- **Focused**: false (no focus steal)
- **Skip taskbar**: true

Content: Simple React component with message text, fade-out animation, click-to-dismiss.

## 12. Migration Path

This is a significant UI change but the backend pipeline (screenshot → text extraction → AI analysis) stays the same. The key changes are:

1. **AppState** gains `active_session` field
2. **do_analyze()** injects session context when session is active
3. **AI prompt** gains `[Active Session]` section + `on_track` response field
4. **SQLite** gains `sessions` + `session_timeline` tables
5. **TrayApp** redesigned: idle view + session view
6. **BubbleApp** redesigned: session dashboard replaces current layout
7. **New window**: speech-bubble for AI messages
8. **Existing features removed**: QuestBoard main quests (replaced by sessions), old reflection trigger (replaced by session summary)
