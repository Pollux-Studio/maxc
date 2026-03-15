# maxc Frontend Implementation Plan

## 1. Objectives

The frontend will provide a **visual workspace environment** for controlling:

* terminal sessions
* browser automation
* agent workers
* diagnostics and logs

The frontend **does not execute runtime logic**. All runtime work happens in the backend control plane which exposes a JSON-RPC interface. 

The UI acts as a **state renderer and command dispatcher**.

---

# 2. Frontend Architecture

## High Level Architecture

```text
Tauri Desktop App
│
├ UI (React / Svelte)
│
├ State Manager
│
├ RPC Client
│
└ Tauri Rust Bridge
     │
     ▼
maxc backend (JSON RPC)
```

Communication happens through **Windows named pipe RPC**.

```text
\\.\pipe\maxc-rpc
```

This is defined in backend configuration. 

---

# 3. Technology Stack

### Desktop framework

Tauri

### UI framework

React (recommended)

### Styling

Tailwind CSS

### State management

Zustand or Redux Toolkit

### Terminal rendering

xterm.js

### Pane layout

Golden Layout or Flex Layout

### Event streaming

WebSocket-like polling or RPC subscription loop

### Icons

Lucide

---

# 4. Project Structure

Recommended UI repository layout.

```text
ui/
 └ maxc-desktop
     ├ src
     │
     ├ app
     │   App.tsx
     │
     ├ rpc
     │   rpcClient.ts
     │   rpcTypes.ts
     │
     ├ state
     │   sessionStore.ts
     │   terminalStore.ts
     │   browserStore.ts
     │   workspaceStore.ts
     │
     ├ components
     │   sidebar
     │   terminal
     │   browser
     │   agent
     │   workspace
     │
     ├ panes
     │   TerminalPane.tsx
     │   BrowserPane.tsx
     │   AgentPane.tsx
     │
     ├ layout
     │   PaneLayout.tsx
     │
     └ hooks
         useTerminal.ts
         useBrowser.ts
         useSession.ts
```

Tauri backend bridge:

```text
src-tauri
 └ src
     rpc_bridge.rs
```

---

# 5. Startup Flow

The frontend must follow the backend contract strictly.

### Required initialization sequence

1. `system.health`
2. `session.create`
3. `system.readiness`

Only enable UI actions when readiness is true.

Example flow:

```text
App start
   │
system.health
   │
session.create
   │
system.readiness
   │
UI enabled
```

Readiness is the **backend action gate**. 

---

# 6. RPC Client Layer

Create a generic RPC client.

```ts
export async function rpc(method, params) {
  return await invoke("rpc_call", {
    request: JSON.stringify({
      id: crypto.randomUUID(),
      method,
      params
    })
  })
}
```

Rules from backend contract:

* every request must have a unique `id`
* mutating requests must include `command_id` 

---

# 7. Session Management

Create a session store.

```ts
type SessionState = {
  token: string
  scopes: string[]
  expiresAt: number
}
```

Flow:

```text
session.create
store token
attach token to all RPC calls
```

---

# 8. Workspace Model

The UI mirrors backend structure.

Hierarchy:

```text
Workspace
   └ Pane
        └ Surface
             └ Panel
```

Example UI structure:

```text
Workspace
 ├ Terminal surface
 ├ Browser surface
 └ Agent surface
```

Each runtime method requires:

* workspace_id
* surface_id 

---

# 9. Terminal Implementation

The backend runs **real local processes** using ConPTY. 

The UI only renders the output stream.

## Terminal creation

```text
terminal.spawn
terminal.subscribe
```

UI state:

```ts
type TerminalPaneState = {
  workspace_id
  surface_id
  terminal_session_id
  cols
  rows
  last_sequence
}
```

---

## Terminal rendering

Use xterm.js.

```ts
const term = new Terminal()

term.onData((data) => {
  rpc("terminal.input", {
    command_id: uuid(),
    terminal_session_id,
    input: data
  })
})
```

---

## Terminal output

Receive `terminal.output` events.

```ts
term.write(event.data)
```

Important rule:

Terminal output is **not line-based**. 

It may contain:

* ANSI escape codes
* partial prompts
* carriage return updates

---

# 10. Browser Pane Implementation

The backend manages browser runtime.

Possible runtimes:

```
chromium-cdp
webview2
browser-simulated
```



---

## Browser creation

```text
browser.create
browser.subscribe
browser.tab.open
```

UI state:

```ts
type BrowserPaneState = {
  browser_session_id
  tab_id
  url
  title
  load_state
}
```

---

## Browser UI rendering

Use Tauri WebView panel.

Example:

```tsx
<iframe src={url} />
```

Automation still goes through backend:

```text
browser.click
browser.type
browser.wait
browser.evaluate
```

---

# 11. Agent Pane

Agent workers run on terminal runtime.

Worker lifecycle:

```text
agent.worker.create
agent.task.start
agent.task.get
agent.task.cancel
agent.worker.close
```

Worker state:

```ts
type AgentState = {
  agent_worker_id
  agent_task_id
  terminal_session_id
  browser_session_id
  status
}
```

---

# 12. Event Subscription System

Terminal and browser events use **bounded subscription streams**.

The frontend must:

* track `sequence`
* detect sequence gaps
* recover using history APIs 

---

## Reconnect strategy

When subscription breaks:

```text
call terminal.history
restore events
restart terminal.subscribe
```

---

# 13. Diagnostics UI

Create an operator panel.

Use RPC endpoints:

```
system.health
system.readiness
system.diagnostics
system.metrics
system.logs
```

Polling recommendations:

```
health → 10s
readiness → 5s
metrics → 10s
logs → 5s
```



---

# 14. Error Handling

Handle backend errors explicitly.

| Code            | Meaning         |
| --------------- | --------------- |
| INVALID_REQUEST | client bug      |
| UNAUTHORIZED    | token invalid   |
| NOT_FOUND       | stale UI state  |
| CONFLICT        | lifecycle issue |
| TIMEOUT         | retryable       |
| RATE_LIMITED    | overload        |
| INTERNAL        | backend failure |



---

# 15. UI Layout

Recommended layout:

```text
+--------------------------------+
| Sidebar                        |
|--------------------------------|
| Workspace list                 |
+--------------------------------+

+--------------------------------+
| Pane Layout                    |
|                                |
| +-----------+----------------+ |
| | Terminal  | Browser        | |
| +-----------+----------------+ |
| | Agent / Logs               | |
| +----------------------------+ |
```

Components:

```
Sidebar
WorkspaceTabs
PaneLayout
TerminalPane
BrowserPane
AgentPane
DiagnosticsPanel
```

---

# 16. Performance Considerations

Terminal events can be high frequency.

Use:

```
batch rendering
throttled updates
virtualized logs
```

Never render each event directly.

---

# 17. Security Considerations

Follow backend policy.

Important rules:

* never bypass RPC
* never run local commands directly
* always send token
* always include workspace and surface IDs

---

# 18. Milestone Roadmap

## Phase 1

RPC bridge
Session management
Health/readiness UI

---

## Phase 2

Terminal pane
xterm integration
terminal.spawn + input + history

---

## Phase 3

Browser pane
browser.create
tab management

---

## Phase 4

Workspace layout
pane splitting
surface tabs

---

## Phase 5

Agent workers
task execution UI

---

## Phase 6

Diagnostics dashboard
metrics viewer
log viewer

---

# 19. Final Architecture

```text
maxc UI
   │
   ▼
Tauri bridge
   │
   ▼
JSON RPC client
   │
   ▼
maxc backend automation
   │
   ├ terminal runtime
   ├ browser runtime
   ├ agent runtime
   └ event store
```