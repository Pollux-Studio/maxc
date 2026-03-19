# maxc Implementation Plan

## Target Hierarchy

```
Window
  └── Workspace (sidebar entry)
        └── Pane (split region)
              └── Surface (tab within pane)
                    └── Panel (terminal or browser content)
```

Visual target:

```
┌──────────────────────────────────────────────────────────────┐
│ ┌──────────┐ ┌────────────────────────────────────────────┐  │
│ │ Sidebar  │ │ Workspace "dev"                            │  │
│ │          │ │                                            │  │
│ │          │ │ ┌──────────────────┬─────────────────────┐ │  │
│ │ > dev    │ │ │ Pane 1           │ Pane 2              │ │  │
│ │   server │ │ │ [S1] [S2] [+]   │ [S1] [+]            │ │  │
│ │   logs   │ │ │                  │                     │ │  │
│ │          │ │ │  Terminal        │  Browser             │ │  │
│ │          │ │ │  (xterm.js)      │  (webview)           │ │  │
│ │          │ │ │                  │                     │ │  │
│ │          │ │ └──────────────────┴─────────────────────┘ │  │
│ └──────────┘ └────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

---

## Current State Summary

| Level     | Backend             | Frontend              | Overall |
|-----------|---------------------|-----------------------|---------|
| Window    | Nothing             | Single hardcoded      | 0%      |
| Workspace | Full CRUD + layout RPC | Sidebar rename/delete | 95%    |
| Pane      | 5 RPC methods, durable | PaneContainer + splits | 90%   |
| Surface   | 4 RPC methods, durable | SurfaceTabBar + tabs  | 85%    |
| Panel     | Terminal + Browser  | Terminal + Browser (screenshot) | 80% |

---

## Phase 1: Pane System (Backend + Frontend) -- DONE

**Goal**: Panes become real entities. Users can split workspaces into resizable regions.

> **Completed**: 5 RPC methods (pane.create/split/list/close/resize), PaneProjection + 4 event types, pane_max_per_workspace config, 5 CLI commands, PaneContainer.tsx with react-resizable-panels v4, keyboard shortcuts (Ctrl+D, Ctrl+Shift+D), workspace.create auto-creates root pane.

### 1.1 Backend: Pane RPC Methods

Add to `server.rs` a new `pane_dispatch` handler with these methods:

| Method         | Type     | Description                              |
|----------------|----------|------------------------------------------|
| `pane.create`  | Mutating | Create a pane in a workspace             |
| `pane.split`   | Mutating | Split an existing pane (right or down)   |
| `pane.close`   | Mutating | Close a pane and redistribute space      |
| `pane.list`    | Read     | List panes in a workspace                |
| `pane.resize`  | Mutating | Set pane size ratio                      |

Data model in `storage/src/lib.rs`:

```rust
pub struct PaneProjection {
    pub pane_id: String,
    pub workspace_id: String,
    pub parent_pane_id: Option<String>,  // for nested splits
    pub split_direction: Option<String>, // "horizontal" | "vertical"
    pub split_ratio: f64,                // 0.0 - 1.0
    pub order: u32,                      // sibling order
    pub created_at_ms: u64,
}
```

Event types to add: `PaneCreated`, `PaneSplit`, `PaneClosed`, `PaneResized`.

Config to add: `MAXC_PANE_MAX_PER_WORKSPACE` (default: 16).

### 1.2 Backend: Wire Pane into Terminal/Browser Spawn

Modify `terminal.spawn` and `browser.create` to accept an optional `pane_id` parameter. If provided, associate the session with that pane. If omitted, use a default pane (auto-created with the workspace).

### 1.3 Frontend: Pane Layout Engine

Replace the current CSS flex toggle with a real split-pane system.

Options (in order of preference):
1. **Custom recursive split component** — lightweight, no dependency, fits the hierarchy exactly
2. **react-resizable-panels** — popular, lightweight, supports nested splits

Build a `PaneContainer` component:

```
PaneContainer
  ├── if leaf: render SurfaceTabBar + active Panel
  └── if split: render two PaneContainers with a drag handle between them
```

Data model (frontend state):

```typescript
type PaneNode =
  | { type: "leaf"; paneId: string; surfaces: SurfaceState[] }
  | { type: "split"; paneId: string; direction: "horizontal" | "vertical";
      ratio: number; children: [PaneNode, PaneNode] };
```

### 1.4 Frontend: Pane Keyboard Shortcuts

| Shortcut           | Action                        |
|--------------------|-------------------------------|
| `Ctrl+D`           | Split pane right (vertical)   |
| `Ctrl+Shift+D`     | Split pane down (horizontal)  |
| `Ctrl+Alt+Arrow`   | Navigate between panes        |
| `Ctrl+Shift+W`     | Close current pane            |

### 1.5 CLI: Pane Commands

```
maxc pane create   --workspace-id <WS>
maxc pane split    --pane-id <P> --direction right|down
maxc pane list     --workspace-id <WS>
maxc pane close    --pane-id <P>
maxc pane resize   --pane-id <P> --ratio 0.6
```

### 1.6 Tests

- Backend: pane CRUD lifecycle, split/close redistribution, max pane limit
- CLI: parse and build pane commands, smoke test against in-process server
- Frontend: PaneContainer renders splits, resize handles work, keyboard nav

### 1.7 Files to Modify

| File | Change |
|------|--------|
| `backend/storage/src/lib.rs` | Add `PaneProjection`, `PaneCreated`/`PaneSplit`/`PaneClosed`/`PaneResized` events, projection apply |
| `backend/core/src/lib.rs` | Add `pane_max_per_workspace` config, `MAXC_PANE_MAX_PER_WORKSPACE` env var |
| `backend/automation/src/server.rs` | Add `pane_dispatch`, `pane_create`, `pane_split`, `pane_close`, `pane_list`, `pane_resize` methods |
| `backend/cli/src/main.rs` | Add `Pane*` command variants, `parse_pane`, `build_request` arms |
| `maxc_desktop/src/components/PaneContainer.tsx` | New file: recursive split-pane layout component |
| `maxc_desktop/src/App.tsx` | Replace flat terminal list with `PaneContainer` tree |
| `maxc_desktop/package.json` | Add `react-resizable-panels` (if chosen over custom) |

---

## Phase 2: Surface System (Backend + Frontend) -- DONE

**Goal**: Surfaces become real tab-within-pane entities. Each pane has a tab bar with multiple surfaces.

> **Completed**: 4 RPC methods (surface.create/list/close/focus), SurfaceProjection + 3 event types, surface_max_per_pane config, 4 CLI commands, SurfaceTabBar.tsx with tab switching/close/create, MAXC_WORKSPACE_ID env var fallback, App.tsx refactored from flat terminal list to recursive pane tree.

### 2.1 Backend: Surface RPC Methods

Add to `server.rs` a new `surface_dispatch` handler:

| Method            | Type     | Description                           |
|-------------------|----------|---------------------------------------|
| `surface.create`  | Mutating | Create a surface (tab) in a pane      |
| `surface.close`   | Mutating | Close a surface                       |
| `surface.list`    | Read     | List surfaces in a pane               |
| `surface.focus`   | Mutating | Set the active surface in a pane      |

Data model in `storage/src/lib.rs`:

```rust
pub struct SurfaceProjection {
    pub surface_id: String,
    pub pane_id: String,
    pub workspace_id: String,
    pub title: String,
    pub panel_type: String,            // "terminal" | "browser" | "agent"
    pub panel_session_id: Option<String>, // terminal_session_id or browser_session_id
    pub order: u32,
    pub focused: bool,
    pub created_at_ms: u64,
}
```

Event types: `SurfaceCreated`, `SurfaceClosed`, `SurfaceFocused`.

Config: `MAXC_SURFACE_MAX_PER_PANE` (default: 16).

### 2.2 Backend: Link Panel Creation to Surface

Modify the terminal/browser spawn flow:
1. `surface.create` creates the tab (returns `surface_id`)
2. `terminal.spawn` or `browser.create` uses that `surface_id`
3. Surface projection gets updated with `panel_session_id`

Or provide a convenience method:
- `surface.create` with `panel_type: "terminal"` auto-spawns the terminal and returns both `surface_id` and `terminal_session_id`

### 2.3 Frontend: Surface Tab Bar

Build a `SurfaceTabBar` component rendered inside each leaf pane:

```
┌─────────────────────────────┐
│ [Terminal 1] [Browser] [+]  │  ← SurfaceTabBar
├─────────────────────────────┤
│                             │
│  (active panel content)     │  ← XtermTerminal or BrowserView
│                             │
└─────────────────────────────┘
```

Features:
- Tab labels with panel type icon (terminal/browser/agent)
- Click to switch active surface
- Close button on each tab
- `[+]` button opens a dropdown: "New Terminal" / "New Browser"
- Drag to reorder tabs

### 2.4 Frontend: Surface Keyboard Shortcuts

| Shortcut        | Action                     |
|-----------------|----------------------------|
| `Ctrl+T`        | New terminal surface       |
| `Ctrl+W`        | Close current surface      |
| `Ctrl+Tab`      | Next surface tab           |
| `Ctrl+Shift+Tab`| Previous surface tab       |
| `Ctrl+1-9`      | Jump to surface tab N      |

Note: `Ctrl+1-9` currently switches workspaces. Remap:
- `Ctrl+1-9` → surface tabs (most common action)
- `Alt+1-9` → workspace switching

### 2.5 CLI: Surface Commands

```
maxc surface create --pane-id <P> --type terminal
maxc surface create --pane-id <P> --type browser
maxc surface list   --pane-id <P>
maxc surface close  --surface-id <S>
maxc surface focus  --surface-id <S>
```

### 2.6 Backend: Inject Environment Variables into Shells

When spawning a terminal, inject these env vars into the child process:

| Variable           | Value                    |
|--------------------|--------------------------|
| `MAXC_WORKSPACE_ID`| The workspace_id         |
| `MAXC_PANE_ID`     | The pane_id              |
| `MAXC_SURFACE_ID`  | The surface_id           |
| `MAXC_SOCKET_PATH` | The server pipe path     |
| `MAXC_TOKEN`       | The session token (if applicable) |

This enables any agent running inside a maxc terminal to auto-discover the maxc API without configuration.

### 2.7 Tests

- Backend: surface CRUD, surface-to-pane ownership, focus switching, max limit
- Backend: env var injection in terminal spawn
- CLI: parse surface commands
- Frontend: tab bar renders, switching works, close removes tab

### 2.8 Files to Modify

| File | Change |
|------|--------|
| `backend/storage/src/lib.rs` | Add `SurfaceProjection`, surface events, projection apply |
| `backend/core/src/lib.rs` | Add `surface_max_per_pane` config |
| `backend/automation/src/server.rs` | Add `surface_dispatch` methods, modify `terminal_spawn` to inject env vars |
| `backend/cli/src/main.rs` | Add surface CLI commands |
| `maxc_desktop/src/components/SurfaceTabBar.tsx` | New file: tab bar component |
| `maxc_desktop/src/components/PaneContainer.tsx` | Integrate SurfaceTabBar into leaf panes |

---

## Phase 3: Workspace Completion -- DONE

**Goal**: Workspaces are fully managed with layout persistence and lifecycle.

> **Completed**: 3 RPC methods (workspace.update/delete/layout), WorkspaceUpdated + WorkspaceDeleted event types, `deleted` field on WorkspaceProjection, workspace.layout returns full recursive pane/surface tree, 3 CLI commands (workspace update/delete/layout), MAXC_WORKSPACE_ID env var fallback, frontend inline rename (double-click) + delete (trash icon) in sidebar, loadPaneTree replaced with workspace.layout single-call, workspace.list filters deleted workspaces. Auto-create root pane was done in Phase 1.

### 3.1 Backend: Missing Workspace RPC Methods

| Method             | Type     | Description                    |
|--------------------|----------|--------------------------------|
| `workspace.update` | Mutating | Rename, change folder, env_vars|
| `workspace.delete` | Mutating | Delete workspace + all children|
| `workspace.layout` | Read     | Get full pane/surface tree     |

### 3.2 Backend: Layout Persistence

Add a `WorkspaceLayoutProjection` that stores the full pane tree:

```rust
pub struct WorkspaceLayoutProjection {
    pub workspace_id: String,
    pub root_pane: PaneTreeNode,
}

pub enum PaneTreeNode {
    Leaf {
        pane_id: String,
        surfaces: Vec<SurfaceSnapshot>,
    },
    Split {
        pane_id: String,
        direction: String,
        ratio: f64,
        children: Vec<PaneTreeNode>,
    },
}
```

This enables workspace layout to survive server restarts.

### 3.3 Backend: Default Pane on Workspace Create

When `workspace.create` is called, auto-create a default root pane so the workspace is immediately usable without a separate `pane.create` call.

### 3.4 Frontend: Workspace Lifecycle

- Add "Rename workspace" (inline edit in sidebar)
- Add "Delete workspace" (with confirmation)
- Add workspace reordering (drag in sidebar)
- Persist selected workspace across app restarts

### 3.5 CLI: Workspace Updates

```
maxc workspace update --workspace-id <WS> --name <new-name>
maxc workspace delete --workspace-id <WS>
maxc workspace layout --workspace-id <WS>
```

### 3.6 Files to Modify

| File | Change |
|------|--------|
| `backend/storage/src/lib.rs` | Add `WorkspaceLayoutProjection`, `PaneTreeNode`, workspace update/delete events |
| `backend/automation/src/server.rs` | Add `workspace.update`, `workspace.delete`, `workspace.layout`, auto-create root pane |
| `backend/cli/src/main.rs` | Add workspace update/delete/layout CLI commands |
| `maxc_desktop/src/App.tsx` | Add rename, delete, reorder to sidebar |

---

## Phase 4: Browser Panel (Frontend) -- DONE

**Goal**: Browser sessions render in the UI as embedded webviews within surface tabs.

> **Completed**: BrowserView.tsx component with URL bar (back/forward/reload/navigate) and screenshot-based rendering (file path via `convertFileSrc`). SurfaceTabBar provides terminal + browser [+] buttons. PaneContainer renders BrowserView for browser surfaces. App.tsx handles `browser.create` + `browser.tab.open` on surface creation, browser navigation/reload/back/forward/screenshot RPC handlers, and stores per-surface browser state in `browserStates`. CLI now includes the missing browser commands (tab list/focus/close, navigation, click/type/key/wait, screenshot/evaluate, cookie/storage, upload/download, trace, intercept).

### 4.1 Frontend: BrowserView Component

Build a `BrowserView` component that renders an embedded browser:

Options:
1. **Tauri WebView** — native, fast, but limited control
2. **iframe pointing to localhost proxy** — simpler, works with any URL

The component needs:
- URL bar with back/forward/reload buttons
- Tab bar synced with `browser.tab.list` RPC
- Screenshot display as fallback (if webview embedding is not feasible)

### 4.2 Frontend: Browser Surface Integration

When user clicks `[+] > New Browser` in a pane's tab bar:
1. Call `surface.create` with `panel_type: "browser"`
2. Call `browser.create` with the returned `surface_id`
3. Render `BrowserView` in that surface tab

### 4.3 CLI: Missing Browser Commands

Add CLI verbs for the 22 browser RPC methods that currently lack CLI commands:

```
maxc browser click        --browser-session-id <BS> --selector <SEL>
maxc browser type         --browser-session-id <BS> --selector <SEL> --text <TEXT>
maxc browser screenshot   --browser-session-id <BS>
maxc browser evaluate     --browser-session-id <BS> --expression <EXPR>
maxc browser tab-list     --browser-session-id <BS>
maxc browser tab-focus    --browser-session-id <BS> --tab-id <TAB>
maxc browser tab-close    --browser-session-id <BS> --tab-id <TAB>
maxc browser reload       --browser-session-id <BS> --tab-id <TAB>
maxc browser back         --browser-session-id <BS> --tab-id <TAB>
maxc browser forward      --browser-session-id <BS> --tab-id <TAB>
maxc browser key          --browser-session-id <BS> --key <KEY>
maxc browser wait         --browser-session-id <BS> --selector <SEL>
maxc browser cookie-get   --browser-session-id <BS>
maxc browser cookie-set   --browser-session-id <BS> --name <N> --value <V>
maxc browser storage-get  --browser-session-id <BS> --key <K>
maxc browser storage-set  --browser-session-id <BS> --key <K> --value <V>
maxc browser upload       --browser-session-id <BS> --selector <SEL> --file <PATH>
maxc browser download     --browser-session-id <BS> --url <URL>
maxc browser trace-start  --browser-session-id <BS>
maxc browser trace-stop   --browser-session-id <BS>
maxc browser evaluate     --browser-session-id <BS> --expression <EXPR>
maxc browser intercept    --browser-session-id <BS> --url-pattern <PAT>
```

### 4.4 Files to Modify

| File | Change |
|------|--------|
| `maxc_desktop/src/components/BrowserView.tsx` | New file: embedded browser component |
| `maxc_desktop/src/components/SurfaceTabBar.tsx` | Handle browser panel type |
| `maxc_desktop/src-tauri/src/lib.rs` | Add Tauri webview commands if using native approach |
| `maxc_desktop/src-tauri/capabilities/default.json` | Add webview permissions |
| `backend/cli/src/main.rs` | Add 22 missing browser CLI commands |

---

## Phase 5: Notification System -- DONE

**Goal**: Agents and users can send notifications that appear in the UI.

> **Completed**: Full stack already implemented — NotificationSent/NotificationCleared event types, NotificationProjection in storage, notification_dispatch with send/list/clear RPC methods, CLI commands (notify + notification list/clear), NotificationPanel.tsx slide-out panel with level-colored indicators and clear buttons, notification polling in App.tsx with toast overlay, badge count in sidebar, tauri-plugin-notification dependency.

> **Completed**: Added notification RPCs (send/list/clear) + projections/events, CLI commands (notify/notification), NotificationPanel + toast UI in the frontend, and OS notifications via tauri-plugin-notification.

### 5.1 Backend: Notification RPC

| Method               | Type     | Description                          |
|----------------------|----------|--------------------------------------|
| `notification.send`  | Mutating | Send a notification to the UI        |
| `notification.list`  | Read     | List recent notifications            |
| `notification.clear` | Mutating | Clear notifications                  |

Data model:

```rust
pub struct NotificationProjection {
    pub notification_id: String,
    pub workspace_id: Option<String>,
    pub title: String,
    pub body: String,
    pub level: String, // "info" | "success" | "warning" | "error"
    pub source: String, // "agent" | "terminal" | "browser" | "user"
    pub created_at_ms: u64,
    pub read: bool,
}
```

### 5.2 CLI: Notify Command

```
maxc notify --title "Build finished" --body "All tests passed" --level success
maxc notify --title "Error" --body "Server crashed" --level error
maxc notification list
maxc notification clear
```

### 5.3 Frontend: Notification UI

- Toast/snackbar overlay for new notifications
- Notification panel (slide-out or dropdown)
- Badge count in sidebar footer
- Desktop OS notifications via `tauri-plugin-notification`

### 5.4 Files to Modify

| File | Change |
|------|--------|
| `backend/storage/src/lib.rs` | Add `NotificationProjection`, events |
| `backend/automation/src/server.rs` | Add `notification_dispatch` |
| `backend/cli/src/main.rs` | Add notify/notification CLI commands |
| `maxc_desktop/src/components/NotificationPanel.tsx` | New file |
| `maxc_desktop/src/App.tsx` | Integrate notification toast + panel |
| `maxc_desktop/src-tauri/Cargo.toml` | Add `tauri-plugin-notification` |

---

## Phase 6: Agent Panel (Frontend) -- DONE

**Goal**: Agent workers and tasks are visible and manageable in the UI.

> **Completed**: AgentPanel.tsx fully implemented with auto-create worker on mount, worker status polling (1s), terminal output polling (500ms), prompt input (Ctrl+Enter to run), task cancel button, output display. SurfaceTabBar has Bot icon [+] button for agent surfaces. PaneContainer renders AgentPanel for agent panel type. App.tsx handleCreateSurface handles agent path (creates surface, AgentPanel handles worker internally).

### 6.1 Frontend: AgentPanel Component

Build an `AgentPanel` component that renders inside a surface tab:

- Worker status display (ready / running / closed)
- Task prompt input
- Live output from attached terminal (read via `terminal.history`)
- Task cancel button
- Task history list

### 6.2 Frontend: Agent Surface Integration

When user clicks `[+] > New Agent` in a pane's tab bar:
1. Call `surface.create` with `panel_type: "agent"`
2. Call `agent.worker.create` with the returned `surface_id`
3. Render `AgentPanel` in that surface tab

### 6.3 Files to Modify

| File | Change |
|------|--------|
| `maxc_desktop/src/components/AgentPanel.tsx` | New file: agent worker/task UI |
| `maxc_desktop/src/components/SurfaceTabBar.tsx` | Handle agent panel type |

---

## Phase 7: Multi-Window Support -- DONE

**Goal**: Users can open multiple maxc windows, each with independent workspaces.

> **Completed**: `create_window` Tauri command already existed with decorations:false + centered 1280x832. Ctrl+Shift+N keyboard shortcut already wired. Capabilities updated from `windows: ["main"]` to `windows: ["*"]` with `core:window:allow-create` and `core:webview:allow-create-webview-window` permissions so new windows get full access. Each window shares the same RpcServer backend (singleton via OnceLock).

> **Completed**: Added `create_window` Tauri command, Ctrl+Shift+N shortcut to open new windows, and multi-window configuration handled in Rust via WindowBuilder.

### 7.1 Frontend: Tauri Multi-Window

- Add `create_window` Tauri command
- Each window gets its own React root with independent state
- Share the same backend RPC connection (same named pipe)

### 7.2 Keyboard Shortcut

| Shortcut          | Action          |
|-------------------|-----------------|
| `Ctrl+Shift+N`    | New window      |

### 7.3 Files to Modify

| File | Change |
|------|--------|
| `maxc_desktop/src-tauri/src/lib.rs` | Add `create_window` command |
| `maxc_desktop/src-tauri/tauri.conf.json` | Allow multiple windows |
| `maxc_desktop/src/App.tsx` | Ensure state is per-window |

---

## Phase 8: Agent Ergonomics -- DONE

**Goal**: Reduce friction for external agents connecting to maxc.

> **Completed**: All 5 env var fallbacks implemented (MAXC_TOKEN, MAXC_SOCKET_PATH, MAXC_WORKSPACE_ID, MAXC_SURFACE_ID, MAXC_PANE_ID) — every CLI command that takes these flags now falls back to env vars. `maxc run "npm test"` convenience command chains workspace.list → pane.list → surface.create → terminal.spawn → terminal.input automatically. `maxc open https://localhost:3000` chains workspace.list → pane.list → surface.create → browser.create → browser.tab.open → browser.goto. All resolve_* helpers have tests. 117 backend tests pass.

### 8.1 CLI: Default IDs from Environment

| Env Var              | Fallback for             |
|----------------------|--------------------------|
| `MAXC_TOKEN`         | `--token`                |
| `MAXC_SOCKET_PATH`   | pipe path                |
| `MAXC_WORKSPACE_ID`  | `--workspace-id`         |
| `MAXC_SURFACE_ID`    | `--surface-id`           |
| `MAXC_PANE_ID`       | `--pane-id`              |

Status: `MAXC_TOKEN` and `MAXC_SOCKET_PATH` are done. The rest need to be added.

### 8.2 CLI: Convenience Wrappers

Add high-level commands that combine multiple RPCs:

```
maxc run "npm test"
  → workspace.list (pick first or use MAXC_WORKSPACE_ID)
  → pane.list (pick first or use MAXC_PANE_ID)
  → surface.create --type terminal
  → terminal.spawn
  → terminal.input "npm test\n"
  → print terminal_session_id

maxc open https://localhost:3000
  → surface.create --type browser
  → browser.create
  → browser.tab.open --url https://localhost:3000
  → print browser_session_id
```

### 8.3 Agent Protocol Documentation

Write a document (`docs/agent-protocol.md`) covering:
- Discovery: how to find the pipe path
- Authentication: session.create flow
- Environment variables: MAXC_* auto-injection
- CLI reference: every command with examples
- RPC reference: every method with request/response schemas
- Example workflows: terminal, browser, multi-agent

### 8.4 Files to Modify

| File | Change |
|------|--------|
| `backend/cli/src/main.rs` | Add MAXC_WORKSPACE_ID/SURFACE_ID/PANE_ID env var support, add `run` and `open` commands |
| `docs/agent-protocol.md` | New file: agent integration documentation |

---

## Implementation Order

```
Phase 1: Pane System           ← DONE
Phase 2: Surface System        ← DONE
Phase 3: Workspace Completion  ← DONE
Phase 4: Browser Panel         ← DONE
Phase 5: Notification System   ← DONE
Phase 6: Agent Panel           ← DONE
Phase 7: Multi-Window          ← DONE
Phase 8: Agent Ergonomics      ← DONE
```

Recommended parallel tracks:

```
Track A (structural):  Phase 1 (DONE) → Phase 2 (DONE) → Phase 3 (DONE) → Phase 7
Track B (panels):      Phase 4 (browser) + Phase 6 (agent)
Track C (integration): Phase 5 (notifications) + Phase 8 (ergonomics)
```

---

## File Impact Summary

| File | Phases |
|------|--------|
| `backend/storage/src/lib.rs` | 1, 2, 3, 5 |
| `backend/core/src/lib.rs` | 1, 2 |
| `backend/automation/src/server.rs` | 1, 2, 3, 5 |
| `backend/cli/src/main.rs` | 1, 2, 3, 4, 5, 8 |
| `maxc_desktop/src/App.tsx` | 1, 2, 3, 5, 7 |
| `maxc_desktop/src/components/PaneContainer.tsx` | 1 (new) |
| `maxc_desktop/src/components/SurfaceTabBar.tsx` | 2 (new) |
| `maxc_desktop/src/components/BrowserView.tsx` | 4 (new) |
| `maxc_desktop/src/components/NotificationPanel.tsx` | 5 (new) |
| `maxc_desktop/src/components/AgentPanel.tsx` | 6 (new) |
| `maxc_desktop/src-tauri/src/lib.rs` | 4, 7 |

---

## Estimated Scope

| Phase | New RPC Methods | New CLI Commands | New Components | Backend Tests |
|-------|-----------------|------------------|----------------|---------------|
| 1     | 5               | 5                | 1              | ~8            |
| 2     | 4               | 4                | 1              | ~6            |
| 3     | 3               | 3                | 0              | ~4            |
| 4     | 0               | 22               | 1              | ~4            |
| 5     | 3               | 4                | 1              | ~4            |
| 6     | 0               | 0                | 1              | 0             |
| 7     | 0               | 0                | 0              | 0             |
| 8     | 0               | 2                | 0              | ~4            |
| **Total** | **15**      | **40**           | **5**          | **~30**       |

---

## Feature: Native Browser Integration (v0.2.0)

### Problem

The current BrowserView uses an `<iframe>` that loads URLs in the Tauri webview sandbox. The backend launches a separate headless Chrome/Edge via CDP. These are completely disconnected.

### Solution

Replace the iframe with a **native embedded Tauri Webview** (`@tauri-apps/api/webview`) rendered directly inside the browser pane. Uses the system's native browser engine (WebView2 on Windows, WebKit on macOS/Linux). Everything stays in one maxc window.

### Architecture

```
maxc window (single native window)
  ├── React UI (main webview)
  │   ├── Sidebar
  │   ├── Terminal panes (xterm.js)
  │   ├── Agent panels
  │   └── Browser pane: URL bar (React) + container div
  │
  └── Native Webview (positioned over browser container div)
      └── Real WebView2/WebKit rendering the web page
```

The React `BrowserView` component measures its container div position/size and creates a native `Webview` at those coordinates. A `ResizeObserver` keeps the webview synced on pane resize.

### Phase NB-1: Backend — Browser Detection + Config (DONE)

- Added `browser_headless: bool` config field + `MAXC_BROWSER_HEADLESS` env var
- Added `system.browsers` RPC method returning detected browsers with names, paths, runtimes
- Files: `backend/core/src/lib.rs`, `backend/automation/src/server.rs`

### Phase NB-2: Tauri — Webview Permissions (DONE)

- Enabled `unstable` feature on `tauri` crate for multiwebview support
- Added 7 webview permissions: `allow-create-webview`, `allow-set-webview-position`, `allow-set-webview-size`, `allow-set-webview-focus`, `allow-webview-close`, `allow-webview-show`, `allow-webview-hide`
- Files: `src-tauri/Cargo.toml`, `src-tauri/capabilities/default.json`

### Phase NB-3: BrowserView — Native Webview Embedding (DONE)

- Replaced `<iframe>` with `new Webview(getCurrentWindow(), label, { url, x, y, width, height })`
- `ResizeObserver` + window resize listener syncs webview position/size with container
- Webview created on URL navigate, closed on unmount
- URL bar + nav buttons remain as React elements
- Error handling with retry button
- Files: `src/components/BrowserView.tsx` (full rewrite)

### Phase NB-4: SettingsDialog — Integrations Tab (DONE)

- Added "Integrations" tab with Puzzle icon between Agent and Shortcuts
- Auto-detects browsers via props from App.tsx
- Selectable browser cards showing name, path, runtime type with check indicator
- Empty state when no browsers detected
- "How it works" explanation section
- Files: `src/components/SettingsDialog.tsx` (+3 props, +1 tab trigger, +1 tab content ~65 lines)

### Phase NB-5: App.tsx — Browser Config Wiring (DONE)

- Added `detectedBrowsers` and `selectedBrowserRuntime` state with localStorage persistence
- Browser detection on startup via `rpc("system.browsers")` (non-fatal)
- `handleBrowserRuntimeChange` persists to localStorage
- All 3 new props passed to SettingsDialog
- Files: `src/App.tsx`

### Phase NB-6: CLI — Browser Detection Commands (DONE)

- Added `maxc browser detect` — calls `system.browsers` RPC, lists available browsers
- CLI test added and passing
- Files: `backend/cli/src/main.rs`
