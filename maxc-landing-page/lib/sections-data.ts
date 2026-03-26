export interface TechSection {
  id: string
  number: string
  title: string
  subtitle: string
  description: string
  ascii: string
  specs: { label: string; value: string }[]
  commands: string[]
}

export const techSections: TechSection[] = [
  {
    id: "terminal-engine",
    number: "01",
    title: "Terminal Engine",
    subtitle: "Shell multiplexing & process control",
    description:
      "The foundation of maxc's interactive layer. ConPTY on Windows with process-stdio fallback provides native terminal sessions for PowerShell, Bash, Node, and Python. Full resize, history, and real-time output subscriptions.",
    ascii: `
    +---------------------------+
    |  TERMINAL ENGINE          |
    |                           |
    |  +--------+ +--------+    |
    |  | ConPTY | | Stdio  |    |
    |  | Win32  | | Fallbk |    |
    |  +---+----+ +---+----+    |
    |      +-----+----+         |
    |  +---------+----------+   |
    |  | SHELL MULTIPLEXER  |   |
    |  +-+----+----+-----+--+   |
    |    |    |    |     |      |
    |   PS  Bash  Node   Py     |
    +---------------------------+`,
    specs: [
      { label: "Max Sessions", value: "32 concurrent" },
      { label: "Backend", value: "ConPTY / stdio" },
      { label: "Shells", value: "PS, Bash, Node, Python" },
      { label: "Features", value: "Resize, History, Subscribe" },
    ],
    commands: [
      "$ maxc terminal spawn --shell powershell",
      "Terminal t_01 spawned [PowerShell 7.4]",
      "$ maxc terminal list",
      "t_01  PowerShell  RUNNING  32x120",
      "$ maxc terminal input t_01 'Get-Process'",
      "Output subscribed: t_01 [streaming]",
    ],
  },
  {
    id: "rpc-api",
    number: "02",
    title: "RPC & API",
    subtitle: "JSON-RPC interface layer",
    description:
      "52 methods spanning terminal, browser, agent, workspace, and system domains. JSON-RPC 2.0 over WebSocket and IPC channels with built-in rate limiting, circuit breaker, and request validation.",
    ascii: `
       [Client]------[WebSocket]
       /|\\               |\\
      / | \\              | \\
  [CLI] | [Agent]------[IPC] [HTTP]
     \\  | /              |  /
      \\ |/               | /
       [RPC Router]----[Handler]
        |                 |
       [Rate Limiter]--[Validator]`,
    specs: [
      { label: "Methods", value: "52 registered" },
      { label: "Protocol", value: "JSON-RPC 2.0" },
      { label: "Transport", value: "WebSocket + IPC" },
      { label: "Rate Limit", value: "100 req/s (200 burst)" },
    ],
    commands: [
      "$ maxc health",
      "Status: HEALTHY | Methods: 52/52",
      "$ maxc readiness",
      "Terminal: OK | Browser: OK | Agent: OK",
      "$ maxc diagnostics",
      "Uptime: 4h 32m | Connections: 12",
    ],
  },
  {
    id: "browser-automation",
    number: "03",
    title: "Browser Automation",
    subtitle: "CDP-powered browser control",
    description:
      "Chromium DevTools Protocol integration with WebView2 fallback. 27 dedicated browser methods for navigation, DOM manipulation, cookie management, local storage, screenshots, downloads, and raw CDP pass-through.",
    ascii: `
    Session #1          Session #2
    +----------+      +----------+
    | CDP:Activ|----->| CDP:Activ|
    | Tabs: 3  |      | Tabs: 1  |
    | Nav: OK  |      | Nav: OK  |
    | DOM:Ready|      | DOM:Load |
    +----------+      +----------+
         |                  |
    +----+----+        +----+----+
    | Chromium|       |WebView2 |
    | Engine  |       |Fallback |
    +---------+        +---------+`,
    specs: [
      { label: "Methods", value: "27 browser-specific" },
      { label: "Max Sessions", value: "8 concurrent" },
      { label: "Protocol", value: "Chromium CDP" },
      { label: "Capabilities", value: "DOM, Storage, Screenshots" },
    ],
    commands: [
      "$ maxc browser create --url https://example.com",
      "Browser b_01 created | Tab: 1 active",
      "$ maxc browser screenshot b_01",
      "Screenshot saved: ./capture_001.png",
      "$ maxc browser list",
      "Active Sessions: 2/8",
    ],
  },
  {
    id: "cli-commands",
    number: "04",
    title: "CLI & Commands",
    subtitle: "Command pipeline interface",
    description:
      "40+ commands organized by domain: terminal, browser, agent, workspace, and system. The maxc CLI pipes through the RPC layer, turning every API method into a composable shell command.",
    ascii: `
    User Input
        |
    +---v---+
    | PARSE | --> Command + Args
    +---+---+
    +---v----+
    | ROUTE  | --> Domain
    +---+----+
    +---v-----------+
    | RPC           |
    | SERIALIZE     | --> JSON-RPC
    +---+-----------+
    +---v-----------+
    | EXECUTE       | --> Result
    +---------------+`,
    specs: [
      { label: "Commands", value: "40+ registered" },
      { label: "Domains", value: "5 (term/browser/agent/ws/sys)" },
      { label: "Transport", value: "Named pipe RPC" },
      { label: "Output", value: "JSON / Table / Raw" },
    ],
    commands: [
      "$ maxc --help",
      "Commands: terminal, browser, agent, workspace, system",
      "$ maxc terminal spawn --shell bash",
      "Terminal t_02 spawned [Bash 5.2]",
      "$ maxc health",
      "All systems operational [52/52 methods]",
    ],
  },
  {
    id: "workspace-architecture",
    number: "05",
    title: "Workspace Architecture",
    subtitle: "Composable workspace hierarchy",
    description:
      "A five-level hierarchy from Window to Panel. Workspaces contain Panes, which hold Surfaces, which render Panels. Each Panel hosts either a Terminal or Browser surface with full lifecycle control.",
    ascii: `
    Window --> Workspace Manager
                     |
              +------+------+
              |  Workspace  |
              |  +--+--+--+ |
              |  |P1|P2|P3| |
              |  +--+--+--+ |
              |  |S1|S2|S3| |
              |  +--+--+--+ |
              +-------------+
    P=Pane  S=Surface  [Term|Browser]`,
    specs: [
      { label: "Hierarchy", value: "5 levels deep" },
      { label: "Layout", value: "Split / Tab / Stack" },
      { label: "Config", value: "MAXC_WORKSPACE_ID" },
      { label: "Surfaces", value: "Terminal or Browser" },
    ],
    commands: [
      "$ maxc workspace list",
      "ws_01: 3 panes | 4 surfaces active",
      "$ maxc workspace create --layout split-h",
      "Workspace ws_02 created [split-h]",
      "$ maxc surface attach ws_02 --type terminal",
      "Surface s_05 attached to ws_02",
    ],
  },
  {
    id: "agent-system",
    number: "06",
    title: "Agent System",
    subtitle: "Autonomous task orchestration",
    description:
      "Worker-based agent architecture for autonomous task execution. Create workers, assign prompt-based tasks, attach terminals and browsers. Agents can read terminal output, drive browser sessions, and report results.",
    ascii: `
        T --+
            +--[ATTACH]--+
        B --+            |
                         +--[WORKER]-- Task
        T --+            |
            +--[ATTACH]--+
        B --+

    Task Queue:
    Prompt  Worker | Status
    "build" W-01   | RUNNING
    "test"  W-02   | QUEUED
    "lint"  W-01   | DONE
    "deploy"W-03   | PENDING`,
    specs: [
      { label: "Max Workers", value: "8 concurrent" },
      { label: "Attach", value: "Terminal + Browser" },
      { label: "Tasks", value: "Prompt-based execution" },
      { label: "Status", value: "Running / Queued / Done" },
    ],
    commands: [
      "$ maxc agent worker create",
      "Worker w_01 created [IDLE]",
      "$ maxc agent task start w_01 --prompt 'run tests'",
      "Task t_01 assigned to w_01 [RUNNING]",
      "$ maxc agent worker list",
      "Workers: 3/8 | Tasks: 5 active",
    ],
  },
  {
    id: "storage-recovery",
    number: "07",
    title: "Storage & Recovery",
    subtitle: "Event sourcing & snapshots",
    description:
      "Persistent event store for workspace state. Every terminal command, browser navigation, and agent action is recorded. Snapshot-based recovery enables full workspace replay and restoration after crashes.",
    ascii: `
    Event 1 --+         +-- Replay
              |         |
    Event 2 --+--[LOG]--+-- Restore
              |    |    |
    Event 3 --+    |    +-- Audit
                   |
             +-----+-----+
             |  Event     |
             |  Store     |
             |  [|||||||] |
             |  MAXC_EVENT|
             |  _DIR      |
             +-----------+`,
    specs: [
      { label: "Storage", value: "Append-only event log" },
      { label: "Snapshots", value: "Periodic + on-demand" },
      { label: "Recovery", value: "Full workspace replay" },
      { label: "Config", value: "MAXC_EVENT_DIR" },
    ],
    commands: [
      "$ maxc events list --last 10",
      "Events: 1,204 total | 10 shown",
      "$ maxc snapshot create",
      "Snapshot snap_042 saved [3.2MB]",
      "$ maxc snapshot restore snap_042",
      "Workspace restored from snap_042 [OK]",
    ],
  },
  {
    id: "security-diagnostics",
    number: "08",
    title: "Security & Diagnostics",
    subtitle: "Auth, monitoring & health",
    description:
      "Token-based authentication, per-method rate limiting, circuit breaker protection, and structured diagnostics. Built-in health endpoints report system state and resource utilization across all workspace components.",
    ascii: `
    +-----------------------------+
    |     CLIENT REQUEST          |
    +-----------------------------+
    |     AUTH / TOKEN CHECK      |
    +-----------------------------+
    |     RATE LIMITER            |
    |  +-------+  +-------+       |
    |  | Allow |  | Deny  |       |
    |  +---+---+  +---+---+       |
    +------+----------+-----------+
    |      |  METRICS  |          |
    |      +-----+-----+          |
    |          [LOG]              |
    +-----------------------------+`,
    specs: [
      { label: "Auth", value: "Token-based sessions" },
      { label: "Rate Limit", value: "Per-method configurable" },
      { label: "Health", value: "/health + /readiness" },
      { label: "Metrics", value: "Structured JSON logs" },
    ],
    commands: [
      "$ maxc health",
      "Status: HEALTHY | Uptime: 4h 32m",
      "$ maxc readiness",
      "RPC: OK | Terminal: OK | Browser: OK",
      "$ maxc diagnostics",
      "Memory: 142MB | Connections: 12 | CPU: 3%",
    ],
  },
]

export const navLinks = techSections.map((s) => ({
  id: s.id,
  number: s.number,
  title: s.title,
}))
