import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import {
  Bell,
  ChevronRight,
  Columns2,
  FolderOpen,
  GitBranch,
  Globe2,
  Keyboard,
  Minus,
  Plus,
  Rows2,
  Square,
  Terminal as TerminalIcon,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { XtermTerminal, type XtermHandle } from "@/components/XtermTerminal";
import { cn } from "@/lib/utils";
import "./App.css";

// ---------------------------------------------------------------------------
// RPC helper
// ---------------------------------------------------------------------------

async function rpc<T = any>(method: string, params: Record<string, unknown>): Promise<T> {
  const request = {
    id: crypto.randomUUID(),
    method,
    params,
  };
  const raw = await invoke<string>("rpc_call", { request: JSON.stringify(request) });
  const parsed = JSON.parse(raw);
  if (parsed.error) {
    const code = parsed.error.code || "ERROR";
    const message = parsed.error.message || "Unknown error";
    throw new Error(code + ": " + message);
  }
  return parsed.result as T;
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type Workspace = {
  workspace_id: string;
  name: string;
  folder: string;
  env_vars: Record<string, string>;
  created_at_ms: number;
};

type WorkspaceMeta = Workspace & {
  gitBranch: string;
  terminalCount: number;
  hasNotification: boolean;
  notificationText: string;
};

type TerminalSurface = {
  id: string;
  surfaceId: string;
  workspaceId: string;
  title: string;
  status: string;
  runtime: string;
  pid?: number;
  lastSequence: number;
  hasNewOutput: boolean;
};

type Readiness = {
  ready: boolean;
  terminal_runtime_ready?: boolean;
  browser_runtime_ready?: boolean;
};

type EnvEntry = { key: string; value: string };

// ---------------------------------------------------------------------------
// Title Bar
// ---------------------------------------------------------------------------

function TitleBar() {
  const appWindow = useMemo(() => {
    try {
      return getCurrentWindow();
    } catch {
      return null;
    }
  }, []);

  const handleMin = () => appWindow?.minimize().catch(console.error);
  const handleMax = async () => {
    if (!appWindow) return;
    const isMax = await appWindow.isMaximized();
    isMax ? await appWindow.unmaximize() : await appWindow.maximize();
  };
  const handleClose = () => appWindow?.close().catch(console.error);

  return (
    <div className="flex items-center border-b bg-card/80 px-3 py-2 text-xs text-muted-foreground backdrop-blur">
      <div className="drag-region flex items-center gap-2" data-tauri-drag-region onDoubleClick={handleMax}>
        <div className="size-2.5 rounded-full bg-primary" />
        <div className="text-sm font-semibold text-foreground" data-tauri-drag-region>maxc</div>
      </div>
      <div className="ml-auto flex items-center gap-1">
        {[
          { icon: Minus, label: "Minimize", action: handleMin },
          { icon: Square, label: "Maximize", action: handleMax },
          { icon: X, label: "Close", action: handleClose },
        ].map(({ icon: Icon, label, action }) => (
          <Button key={label} variant="ghost" size="icon-sm" onClick={(e) => { e.stopPropagation(); action(); }} className="no-drag" aria-label={label}>
            <Icon className="size-3.5" />
          </Button>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

function App() {
  // -- core state --
  const [token, setToken] = useState("");
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState("");
  const [terminals, setTerminals] = useState<TerminalSurface[]>([]);
  const [backendStatus, setBackendStatus] = useState("Connecting...");
  const [readiness, setReadiness] = useState<Readiness | null>(null);
  const [browserSessionId, setBrowserSessionId] = useState("");

  // -- drawer state --
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [wsName, setWsName] = useState("");
  const [wsFolder, setWsFolder] = useState("");
  const [wsEnvVars, setWsEnvVars] = useState<EnvEntry[]>([]);
  const [wsCreating, setWsCreating] = useState(false);
  const [wsError, setWsError] = useState("");

  // -- workspace metadata --
  const [gitBranches, setGitBranches] = useState<Record<string, string>>({});
  const [notifications, setNotifications] = useState<Record<string, string>>({});

  // -- shortcut help --
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [splitMode, setSplitMode] = useState<"vertical" | "horizontal">("vertical");

  // -- refs --
  const terminalsRef = useRef<TerminalSurface[]>([]);
  const pollInFlightRef = useRef(false);
  const spawnInFlightRef = useRef(false);
  const xtermHandles = useRef<Map<string, XtermHandle>>(new Map());
  const pendingInitialOutput = useRef<Map<string, string>>(new Map());
  const [focusedTerminalId, setFocusedTerminalId] = useState("");

  const selectedWorkspace = useMemo(
    () => workspaces.find((w) => w.workspace_id === selectedWorkspaceId) ?? workspaces[0] ?? null,
    [workspaces, selectedWorkspaceId],
  );

  const readyForActions = Boolean(token) && (readiness?.ready ?? false) && selectedWorkspace !== null;

  // -- workspace metadata enrichment --
  const workspaceMetas: WorkspaceMeta[] = useMemo(() => {
    return workspaces.map((ws) => {
      const termCount = terminals.filter((t) => t.workspaceId === ws.workspace_id).length;
      const wsTerminals = terminals.filter((t) => t.workspaceId === ws.workspace_id);
      const hasNotif = wsTerminals.some((t) => t.hasNewOutput);
      return {
        ...ws,
        gitBranch: gitBranches[ws.workspace_id] || "",
        terminalCount: termCount,
        hasNotification: hasNotif || Boolean(notifications[ws.workspace_id]),
        notificationText: notifications[ws.workspace_id] || (hasNotif ? "New output" : ""),
      };
    });
  }, [workspaces, terminals, gitBranches, notifications]);

  // ---------------------------------------------------------------------------
  // Fetch git branches for workspaces
  // ---------------------------------------------------------------------------
  const refreshGitBranches = useCallback(async () => {
    const branches: Record<string, string> = {};
    for (const ws of workspaces) {
      if (ws.folder) {
        try {
          const branch = await invoke<string>("get_git_branch", { folder: ws.folder });
          if (branch) branches[ws.workspace_id] = branch;
        } catch { /* ignore */ }
      }
    }
    setGitBranches(branches);
  }, [workspaces]);

  // ---------------------------------------------------------------------------
  // Startup
  // ---------------------------------------------------------------------------
  useEffect(() => {
    (async () => {
      try {
        const health = await rpc<{ ok: boolean }>("system.health", {});
        if (!health.ok) throw new Error("health not ok");
        const session = await rpc<{ token: string; scopes: string[] }>("session.create", {
          command_id: "ui-session-" + crypto.randomUUID(),
        });
        setToken(session.token);
        const ready = await rpc<Readiness>("system.readiness", {
          auth: { token: session.token },
        });
        setReadiness(ready);
        setBackendStatus(ready.ready ? "Backend ready" : "Backend not ready yet");

        const wsList = await rpc<{ workspaces: Workspace[] }>("workspace.list", {
          auth: { token: session.token },
        });
        if (wsList.workspaces.length > 0) {
          setWorkspaces(wsList.workspaces);
          setSelectedWorkspaceId(wsList.workspaces[0].workspace_id);
        }
      } catch (error) {
        console.error(error);
        setBackendStatus((error as Error).message);
      }
    })();
  }, []);

  // refresh git branches when workspaces change
  useEffect(() => {
    if (workspaces.length > 0) refreshGitBranches();
  }, [workspaces, refreshGitBranches]);

  // periodically refresh git branches (every 15s)
  useEffect(() => {
    if (workspaces.length === 0) return;
    const id = setInterval(refreshGitBranches, 15_000);
    return () => clearInterval(id);
  }, [workspaces, refreshGitBranches]);

  // ---------------------------------------------------------------------------
  // Terminal polling
  // ---------------------------------------------------------------------------
  useEffect(() => { terminalsRef.current = terminals; }, [terminals]);

  useEffect(() => {
    if (!token) return;
    const poll = async () => {
      if (pollInFlightRef.current) return;
      const list = terminalsRef.current;
      if (!list.length) return;
      pollInFlightRef.current = true;
      try {
        const updated = await Promise.all(
          list.map(async (t) => {
            const result = await rpc<any>("terminal.history", {
              workspace_id: t.workspaceId,
              surface_id: t.surfaceId,
              terminal_session_id: t.id,
              from_sequence: t.lastSequence + 1,
              max_events: 64,
              auth: { token },
            });
            let lastSeq = t.lastSequence;
            let newOutput = false;
            if (Array.isArray(result.events)) {
              const handle = xtermHandles.current.get(t.id);
              for (const ev of result.events) {
                if (ev.type === "terminal.output" && ev.output) {
                  handle?.write(ev.output as string);
                  newOutput = true;
                }
                lastSeq = Math.max(lastSeq, ev.sequence ?? lastSeq);
              }
            }
            return {
              ...t,
              status: result.status ?? t.status,
              runtime: result.runtime ?? t.runtime,
              pid: result.pid ?? t.pid,
              lastSequence: lastSeq,
              hasNewOutput: newOutput || t.hasNewOutput,
            } as TerminalSurface;
          }),
        );
        terminalsRef.current = updated;
        setTerminals(updated);
      } catch (err) {
        console.error(err);
      } finally {
        pollInFlightRef.current = false;
      }
    };
    const id = setInterval(poll, 500);
    return () => clearInterval(id);
  }, [token]);

  // ---------------------------------------------------------------------------
  // Keyboard shortcuts
  // ---------------------------------------------------------------------------
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;

      // Ctrl+N -> new workspace drawer
      if (ctrl && e.key === "n") { e.preventDefault(); openDrawer(); return; }

      // Ctrl+T -> new terminal
      if (ctrl && e.key === "t") { e.preventDefault(); addTerminal(); return; }

      // Ctrl+B -> open browser
      if (ctrl && e.key === "b") { e.preventDefault(); openBrowser(); return; }

      // Ctrl+? -> toggle shortcuts panel
      if (ctrl && e.key === "/") { e.preventDefault(); setShowShortcuts((v) => !v); return; }

      // Ctrl+1-9 -> switch workspace
      if (ctrl && e.key >= "1" && e.key <= "9") {
        e.preventDefault();
        const idx = parseInt(e.key) - 1;
        if (idx < workspaces.length) {
          setSelectedWorkspaceId(workspaces[idx].workspace_id);
        }
        return;
      }

      // Escape -> close drawer/shortcuts
      if (e.key === "Escape") {
        if (drawerOpen) closeDrawer();
        if (showShortcuts) setShowShortcuts(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });

  // ---------------------------------------------------------------------------
  // Drawer actions
  // ---------------------------------------------------------------------------
  function openDrawer() {
    setWsName(""); setWsFolder(""); setWsEnvVars([]); setWsError("");
    setDrawerOpen(true);
  }
  function closeDrawer() {
    setDrawerOpen(false); setWsCreating(false); setWsError("");
  }
  async function pickFolder() {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select workspace folder" });
      if (selected && typeof selected === "string") setWsFolder(selected);
    } catch { /* cancelled */ }
  }
  function addEnvEntry() { setWsEnvVars((p) => [...p, { key: "", value: "" }]); }
  function updateEnvEntry(i: number, field: "key" | "value", val: string) {
    setWsEnvVars((p) => p.map((e, idx) => (idx === i ? { ...e, [field]: val } : e)));
  }
  function removeEnvEntry(i: number) { setWsEnvVars((p) => p.filter((_, idx) => idx !== i)); }

  async function createWorkspace() {
    if (!token) { setWsError("No session token"); return; }
    const name = wsName.trim();
    if (!name) { setWsError("Workspace name is required"); return; }

    const envObj: Record<string, string> = {};
    for (const e of wsEnvVars) { const k = e.key.trim(); if (k) envObj[k] = e.value; }

    setWsCreating(true); setWsError("");
    try {
      const result = await rpc<Workspace>("workspace.create", {
        command_id: "ui-ws-create-" + crypto.randomUUID(),
        name, folder: wsFolder, env_vars: envObj,
        auth: { token },
      });
      const newWs: Workspace = {
        workspace_id: result.workspace_id, name: result.name,
        folder: result.folder, env_vars: result.env_vars, created_at_ms: result.created_at_ms,
      };
      setWorkspaces((w) => [...w, newWs]);
      setSelectedWorkspaceId(newWs.workspace_id);
      setBackendStatus("Workspace created");
      closeDrawer();
    } catch (error) {
      setWsError((error as Error).message);
    } finally { setWsCreating(false); }
  }

  // ---------------------------------------------------------------------------
  // Terminal / Browser actions
  // ---------------------------------------------------------------------------
  async function addTerminal() {
    if (!readyForActions || !selectedWorkspace) {
      setBackendStatus("Create a workspace first, then spawn a terminal");
      return;
    }
    if (spawnInFlightRef.current) { setBackendStatus("terminal.spawn in progress..."); return; }
    const surfaceId = "surface-" + crypto.randomUUID();
    try {
      spawnInFlightRef.current = true;
      setBackendStatus("Spawning terminal...");
      const result = await rpc<any>("terminal.spawn", {
        command_id: "ui-term-spawn-" + crypto.randomUUID(),
        workspace_id: selectedWorkspace.workspace_id,
        surface_id: surfaceId,
        cols: 120, rows: 34,
        auth: { token },
      });

      const termId = result.terminal_session_id as string;

      // Collect any initial output that arrived between spawn and now.
      // We'll write it into the xterm once the component mounts.
      let initialOutput = "";
      let initialLastSeq = 0;
      if (termId) {
        try {
          const history = await rpc<any>("terminal.history", {
            workspace_id: selectedWorkspace.workspace_id,
            surface_id: surfaceId,
            terminal_session_id: termId,
            from_sequence: 0,
            auth: { token },
          });
          if (Array.isArray(history.events)) {
            for (const ev of history.events) {
              if (ev.type === "terminal.output" && ev.output) initialOutput += ev.output as string;
              initialLastSeq = Math.max(initialLastSeq, ev.sequence ?? initialLastSeq);
            }
          }
        } catch { /* initial history fetch failed, non-fatal */ }
      }

      // Buffer initial output so we can write it once the xterm handle registers
      if (initialOutput) {
        pendingInitialOutput.current.set(termId, initialOutput);
      }

      const next: TerminalSurface[] = [
        ...terminalsRef.current,
        {
          id: termId, surfaceId,
          workspaceId: selectedWorkspace.workspace_id,
          title: "Terminal " + (terminalsRef.current.filter((t) => t.workspaceId === selectedWorkspace.workspace_id).length + 1),
          status: result.status ?? "running", runtime: result.runtime ?? "unknown",
          pid: result.pid, lastSequence: initialLastSeq, hasNewOutput: false,
        },
      ];
      terminalsRef.current = next;
      setTerminals(next);
      setFocusedTerminalId(termId);
      setBackendStatus("Terminal spawned");
    } catch (error) {
      console.error(error);
      setBackendStatus((error as Error).message);
    } finally { spawnInFlightRef.current = false; }
  }

  async function openBrowser() {
    if (!readyForActions || !selectedWorkspace) {
      setBackendStatus("Create a workspace first"); return;
    }
    try {
      const result = await rpc<any>("browser.create", {
        command_id: "ui-browser-" + crypto.randomUUID(),
        workspace_id: selectedWorkspace.workspace_id,
        surface_id: "surface-browser-" + crypto.randomUUID(),
        auth: { token },
      });
      setBrowserSessionId(result.browser_session_id);
      setBackendStatus("Browser ready (" + (result.runtime ?? "runtime unknown") + ")");
    } catch (error) {
      setBackendStatus((error as Error).message);
    }
  }

  // -- terminal input / resize handlers --
  const sendTerminalInput = useCallback(
    async (terminalSessionId: string, workspaceId: string, surfaceId: string, data: string) => {
      if (!token) return;
      try {
        await rpc("terminal.input", {
          command_id: "ui-term-input-" + crypto.randomUUID(),
          workspace_id: workspaceId,
          surface_id: surfaceId,
          terminal_session_id: terminalSessionId,
          input: data,
          auth: { token },
        });
      } catch (err) {
        console.error("terminal.input error:", err);
      }
    },
    [token],
  );

  const sendTerminalResize = useCallback(
    async (terminalSessionId: string, workspaceId: string, surfaceId: string, cols: number, rows: number) => {
      if (!token) return;
      try {
        await rpc("terminal.resize", {
          command_id: "ui-term-resize-" + crypto.randomUUID(),
          workspace_id: workspaceId,
          surface_id: surfaceId,
          terminal_session_id: terminalSessionId,
          cols,
          rows,
          auth: { token },
        });
      } catch (err) {
        console.error("terminal.resize error:", err);
      }
    },
    [token],
  );

  /** Register an xterm handle; flush any buffered initial output */
  const registerXtermHandle = useCallback((termId: string, handle: XtermHandle | null) => {
    if (handle) {
      xtermHandles.current.set(termId, handle);
      // Flush any initial output that was buffered before the component mounted
      const pending = pendingInitialOutput.current.get(termId);
      if (pending) {
        handle.write(pending);
        pendingInitialOutput.current.delete(termId);
      }
    } else {
      xtermHandles.current.delete(termId);
    }
  }, []);

  // clear notification when selecting a workspace
  function selectWorkspace(wsId: string) {
    setSelectedWorkspaceId(wsId);
    // clear new-output flags on terminals in this workspace
    setTerminals((prev) =>
      prev.map((t) => (t.workspaceId === wsId ? { ...t, hasNewOutput: false } : t)),
    );
    setNotifications((prev) => { const n = { ...prev }; delete n[wsId]; return n; });
  }

  // ---------------------------------------------------------------------------
  // Workspace vertical tabs sidebar
  // ---------------------------------------------------------------------------
  const currentTerminals = useMemo(
    () => terminals.filter((t) => selectedWorkspace && t.workspaceId === selectedWorkspace.workspace_id),
    [terminals, selectedWorkspace],
  );

  function renderWorkspaceSidebar() {
    return (
      <aside className="h-[calc(100vh-40px)] w-56 border-r bg-sidebar/95 flex flex-col text-[12px] backdrop-blur">
        {/* header */}
        <div className="flex items-center justify-between px-3 pt-3 pb-2">
          <div className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground">Workspace</div>
          <Button size="icon-xs" variant="ghost" onClick={openDrawer} title="New Workspace (Ctrl+N)">
            <Plus className="size-3.5" />
          </Button>
        </div>

        {/* workspace tabs */}
        <div className="flex-1 overflow-auto px-2 space-y-1 pb-3">
          {workspaceMetas.length === 0 && (
            <div className="px-2 py-6 text-center text-[11px] text-muted-foreground">
              No workspaces yet.<br />Press <kbd className="rounded bg-muted px-1 py-0.5 text-[10px]">Ctrl+N</kbd> to create one.
            </div>
          )}
          {workspaceMetas.map((ws, idx) => {
            const active = selectedWorkspaceId === ws.workspace_id;
            const shortcutLabel = idx < 9 ? `Ctrl+${idx + 1}` : "";
            return (
              <button
                key={ws.workspace_id}
                onClick={() => selectWorkspace(ws.workspace_id)}
                className={cn(
                  "group relative w-full rounded-lg px-2.5 py-2 text-left transition-all shadow-sm overflow-hidden",
                  active
                    ? "bg-gradient-to-r from-primary/10 via-primary/5 to-transparent border border-primary/25"
                    : "border border-transparent hover:bg-muted/70 hover:shadow-md",
                )}
              >
                <div className="absolute left-0 top-0 h-full w-1 bg-gradient-to-b from-primary/70 via-primary/40 to-transparent opacity-80" />

                {/* notification ring */}
                {ws.hasNotification && !active && (
                  <div className="absolute -left-0.5 top-1/2 -translate-y-1/2 h-5 w-1 rounded-r-full bg-chart-1 animate-pulse" />
                )}

                {/* workspace name + chevron */}
                <div className="flex items-center gap-1.5 pl-0.5">
                  <ChevronRight className={cn("size-3 text-muted-foreground transition-transform", active && "rotate-90 text-primary")} />
                  <span className={cn("font-semibold truncate", active ? "text-foreground" : "text-muted-foreground")}>{ws.name}</span>
                  {shortcutLabel && (
                    <span className="ml-auto text-[9px] text-muted-foreground/50 opacity-0 group-hover:opacity-100 transition-opacity">
                      {shortcutLabel}
                    </span>
                  )}
                </div>

                {/* metadata row */}
                <div className="mt-1 flex flex-wrap items-center gap-1.5 text-[10px] text-muted-foreground">
                  {ws.gitBranch && (
                    <span className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-1 text-[10px]" title={"Branch: " + ws.gitBranch}>
                      <GitBranch className="size-2.5 text-primary" />
                      <span className="max-w-[90px] truncate">{ws.gitBranch}</span>
                    </span>
                  )}
                  {ws.folder && (
                    <span className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-1 text-[10px]" title={ws.folder}>
                      <FolderOpen className="size-2.5" />
                      <span className="max-w-[110px] truncate">{ws.folder.split(/[\\/]/).pop()}</span>
                    </span>
                  )}
                  <span className="inline-flex items-center gap-1 rounded-full px-2 py-1 text-[10px] bg-primary/10 text-primary">
                    <TerminalIcon className="size-2.5" />
                    {ws.terminalCount} terms
                  </span>
                  {ws.notificationText && !active && (
                    <span className="inline-flex items-center gap-1 rounded-full px-2 py-1 text-[10px] bg-chart-1/15 text-chart-1" title={ws.notificationText}>
                      <Bell className="size-2.5" />
                      <span className="truncate max-w-[80px]">{ws.notificationText}</span>
                    </span>
                  )}
                </div>

                {/* status line */}
                <div className="mt-1 flex items-center justify-between text-[10px] text-muted-foreground">
                  <span className="truncate">{ws.folder || "No directory set"}</span>
                  <span className={cn(
                    "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px]",
                    active ? "bg-primary/15 text-primary" : "bg-muted text-foreground/70"
                  )}>
                    {ws.terminalCount > 0 ? `${ws.terminalCount} running` : "Idle"}
                  </span>
                </div>
              </button>
            );
          })}
        </div>

        {/* footer status */}
        <div className="border-t px-3 py-2 flex items-center justify-between bg-sidebar">
          <span className="text-[10px] text-muted-foreground truncate">
            {readiness ? (readiness.ready ? "Ready" : "Not ready") : backendStatus}
          </span>
          <Button size="icon-xs" variant="ghost" onClick={() => setShowShortcuts((v) => !v)} title="Keyboard Shortcuts (Ctrl+/)">
            <Keyboard className="size-3" />
          </Button>
        </div>
      </aside>
    );
  }

  // ---------------------------------------------------------------------------
  // Toolbar
  // ---------------------------------------------------------------------------
  function renderToolbar() {
    return (
      <div className="flex items-center justify-end gap-2 border-b bg-card/95 px-3 py-2 text-[12px] backdrop-blur">
        <Button
          variant={splitMode === "vertical" ? "secondary" : "ghost"}
          size="icon-sm"
          onClick={() => setSplitMode("vertical")}
          title="Vertical split (Ctrl+Shift+S)"
          className="h-7 w-7"
        >
          <Columns2 className="size-3.5" />
        </Button>
        <Button
          variant={splitMode === "horizontal" ? "secondary" : "ghost"}
          size="icon-sm"
          onClick={() => setSplitMode("horizontal")}
          title="Horizontal split (Ctrl+Shift+S)"
          className="h-7 w-7"
        >
          <Rows2 className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={openBrowser}
          disabled={!readyForActions}
          title={browserSessionId ? "Browser Ready" : "Open Browser"}
          className="h-7 w-7"
        >
          <Globe2 className="size-3.5" />
        </Button>
        <Button
          variant="secondary"
          size="icon-sm"
          onClick={addTerminal}
          disabled={!readyForActions}
          title="New Terminal"
          className="h-7 w-7"
        >
          <TerminalIcon className="size-3.5" />
        </Button>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Terminal panes with notification rings
  // ---------------------------------------------------------------------------
  function renderTerminals() {
    if (!selectedWorkspace) {
      return (
        <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
          <span>No workspace selected.</span>
          <Button variant="outline" size="sm" onClick={openDrawer}>
            <Plus className="size-3.5" /> Create Workspace
          </Button>
        </div>
      );
    }
    if (!currentTerminals.length) {
      return (
        <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
          <span>No terminals in this workspace.</span>
          <Button variant="outline" size="sm" onClick={addTerminal} disabled={!readyForActions}>
            <TerminalIcon className="size-3.5" /> New Terminal
          </Button>
        </div>
      );
    }

    // Single terminal: fill the entire pane
    if (currentTerminals.length === 1) {
      const t = currentTerminals[0];
      return (
        <div className="flex h-full flex-col rounded-xl border bg-[#0a0a0a] shadow-inner overflow-hidden">
          <div className="flex items-center justify-between border-b border-[#1f1f1f] px-3 py-1.5 bg-[#0f0f0f]/90">
            <div className="flex items-center gap-2 text-[11px] font-semibold text-[#e8e8e8]">
              <TerminalIcon className="size-3.5 text-primary" />
              {t.title}
              {t.hasNewOutput && <span className="size-1.5 rounded-full bg-chart-1 animate-pulse" />}
            </div>
            <div className="text-[10px] text-[#8c8c8c]">
              {t.status} · pid {t.pid ?? "-"}
            </div>
          </div>
          <div className="flex-1 min-h-0 bg-[#0c0c0c]">
            <XtermTerminal
              key={t.id}
              ref={(handle) => registerXtermHandle(t.id, handle)}
              focused={focusedTerminalId === t.id}
              onData={(data) => sendTerminalInput(t.id, t.workspaceId, t.surfaceId, data)}
              onResize={(cols, rows) => sendTerminalResize(t.id, t.workspaceId, t.surfaceId, cols, rows)}
            />
          </div>
        </div>
      );
    }

    // Multiple terminals: bento grid layout
    const isVertical = splitMode === "vertical";
    return (
      <div
        className={cn("h-full gap-3", isVertical ? "flex flex-row" : "flex flex-col")}
      >
        {currentTerminals.map((t) => (
          <div
            key={t.id}
            className={cn(
              "flex flex-col min-h-0 min-w-0 rounded-xl border border-border bg-[#0a0a0a] shadow-inner overflow-hidden",
              isVertical ? "flex-1" : "flex-1",
              focusedTerminalId === t.id && "ring-1 ring-primary/30 ring-inset",
            )}
            onClick={() => {
              setFocusedTerminalId(t.id);
              setTerminals((prev) => prev.map((x) => (x.id === t.id ? { ...x, hasNewOutput: false } : x)));
            }}
          >
            {/* Terminal tab bar */}
            <div className="flex items-center justify-between px-3 py-1.5 bg-[#0f0f0f]/90 border-b border-[#1f1f1f]">
              <div className="flex items-center gap-1.5 text-[11px] font-semibold text-[#e8e8e8]">
                <TerminalIcon className="size-3 text-primary" />
                {t.title}
                {t.hasNewOutput && <span className="size-1.5 rounded-full bg-chart-1 animate-pulse" />}
              </div>
              <div className="text-[10px] text-[#8c8c8c]">
                {t.status} · pid {t.pid ?? "-"}
              </div>
            </div>
            <div className="flex-1 min-h-0 bg-[#0c0c0c]">
              <XtermTerminal
                key={t.id}
                ref={(handle) => registerXtermHandle(t.id, handle)}
                focused={focusedTerminalId === t.id}
                onData={(data) => sendTerminalInput(t.id, t.workspaceId, t.surfaceId, data)}
                onResize={(cols, rows) => sendTerminalResize(t.id, t.workspaceId, t.surfaceId, cols, rows)}
              />
            </div>
          </div>
        ))}
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // New Workspace Drawer
  // ---------------------------------------------------------------------------
  function renderDrawer() {
    return (
      <>
        <div
          className={cn(
            "fixed inset-0 z-40 bg-black/40 transition-opacity duration-200",
            drawerOpen ? "opacity-100" : "pointer-events-none opacity-0",
          )}
          onClick={closeDrawer}
        />
        <div
          className={cn(
            "fixed right-0 top-0 z-50 flex h-full w-[380px] flex-col border-l bg-card shadow-xl transition-transform duration-200 ease-out",
            drawerOpen ? "translate-x-0" : "translate-x-full",
          )}
        >
          <div className="flex items-center justify-between border-b px-5 py-4">
            <h2 className="text-sm font-semibold">New Workspace</h2>
            <Button variant="ghost" size="icon-sm" onClick={closeDrawer}><X className="size-4" /></Button>
          </div>
          <div className="flex-1 overflow-auto px-5 py-4 space-y-5">
            {/* workspace id */}
            <div className="space-y-1.5">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Workspace ID</label>
              <div className="rounded-md border bg-muted px-3 py-2 text-xs text-muted-foreground">Auto-generated on creation</div>
            </div>
            {/* name */}
            <div className="space-y-1.5">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Name <span className="text-destructive">*</span></label>
              <input type="text" value={wsName} onChange={(e) => setWsName(e.target.value)} placeholder="e.g. my-project"
                className="w-full rounded-md border bg-background px-3 py-2 text-xs outline-none transition focus:border-primary/50 focus:ring-1 focus:ring-primary/30"
                autoFocus
              />
            </div>
            {/* folder */}
            <div className="space-y-1.5">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Working Directory</label>
              <div className="flex gap-2">
                <div className="flex-1 truncate rounded-md border bg-background px-3 py-2 text-xs text-muted-foreground">
                  {wsFolder || "No folder selected"}
                </div>
                <Button variant="outline" size="sm" onClick={pickFolder} className="h-[34px] px-2.5 shrink-0">
                  <FolderOpen className="size-3.5" /><span>Browse</span>
                </Button>
              </div>
            </div>
            {/* env vars */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">Environment Variables</label>
                <Button variant="ghost" size="icon-xs" onClick={addEnvEntry}><Plus className="size-3.5" /></Button>
              </div>
              <p className="text-[11px] text-muted-foreground">Variables available to agent workers in this workspace.</p>
              {wsEnvVars.length === 0 && (
                <div className="rounded-md border border-dashed px-3 py-3 text-center text-[11px] text-muted-foreground">
                  No environment variables. Click + to add.
                </div>
              )}
              <div className="space-y-2">
                {wsEnvVars.map((entry, i) => (
                  <div key={i} className="flex items-center gap-1.5">
                    <input type="text" value={entry.key} onChange={(e) => updateEnvEntry(i, "key", e.target.value)} placeholder="KEY"
                      className="w-[120px] shrink-0 rounded-md border bg-background px-2 py-1.5 text-xs outline-none transition focus:border-primary/50 focus:ring-1 focus:ring-primary/30" />
                    <span className="text-[11px] text-muted-foreground">=</span>
                    <input type="text" value={entry.value} onChange={(e) => updateEnvEntry(i, "value", e.target.value)} placeholder="value"
                      className="flex-1 rounded-md border bg-background px-2 py-1.5 text-xs outline-none transition focus:border-primary/50 focus:ring-1 focus:ring-primary/30" />
                    <Button variant="ghost" size="icon-xs" onClick={() => removeEnvEntry(i)}>
                      <Trash2 className="size-3 text-muted-foreground hover:text-destructive" />
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="border-t px-5 py-4 space-y-2">
            {wsError && <div className="rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">{wsError}</div>}
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={closeDrawer} className="flex-1 h-9" disabled={wsCreating}>Cancel</Button>
              <Button variant="default" size="sm" onClick={createWorkspace} className="flex-1 h-9" disabled={wsCreating || !wsName.trim()}>
                {wsCreating ? "Creating..." : "Create Workspace"}
              </Button>
            </div>
          </div>
        </div>
      </>
    );
  }

  // ---------------------------------------------------------------------------
  // Keyboard shortcuts overlay
  // ---------------------------------------------------------------------------
  function renderShortcutsPanel() {
    if (!showShortcuts) return null;
    const shortcuts = [
      { keys: "Ctrl+N", desc: "New workspace" },
      { keys: "Ctrl+T", desc: "New terminal" },
      { keys: "Ctrl+B", desc: "Open browser" },
      { keys: "Ctrl+1-9", desc: "Switch workspace" },
      { keys: "Ctrl+/", desc: "Toggle shortcuts" },
      { keys: "Escape", desc: "Close panel / drawer" },
    ];
    return (
      <>
        <div className="fixed inset-0 z-40 bg-black/30" onClick={() => setShowShortcuts(false)} />
        <div className="fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 w-[320px] rounded-xl border bg-card p-5 shadow-2xl">
          <div className="flex items-center justify-between mb-4">
            <h3 className="text-sm font-semibold flex items-center gap-2"><Keyboard className="size-4" /> Keyboard Shortcuts</h3>
            <Button variant="ghost" size="icon-xs" onClick={() => setShowShortcuts(false)}><X className="size-3.5" /></Button>
          </div>
          <div className="space-y-2">
            {shortcuts.map((s) => (
              <div key={s.keys} className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{s.desc}</span>
                <kbd className="rounded bg-muted px-2 py-0.5 text-[11px] font-mono">{s.keys}</kbd>
              </div>
            ))}
          </div>
        </div>
      </>
    );
  }

  // ---------------------------------------------------------------------------
  // Layout
  // ---------------------------------------------------------------------------
  return (
    <div className="flex h-screen flex-col bg-background text-foreground overflow-hidden text-[13px]">
      <TitleBar />
      <div className="flex flex-1 min-h-0">
        {renderWorkspaceSidebar()}
        <main className="flex flex-1 min-h-0 flex-col overflow-hidden bg-gradient-to-br from-muted/40 via-background to-background">
          {renderToolbar()}
          <div className="flex-1 overflow-auto p-3 pb-4">
            <div className="h-full rounded-2xl border bg-card/70 backdrop-blur-sm shadow-md p-2">
              {renderTerminals()}
            </div>
          </div>
        </main>
      </div>
      {renderDrawer()}
      {renderShortcutsPanel()}
    </div>
  );
}

export default App;
