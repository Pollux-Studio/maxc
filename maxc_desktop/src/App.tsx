import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ActivitySquare,
  Globe2,
  Minus,
  Plus,
  Square,
  Terminal as TerminalIcon,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import "./App.css";

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

const defaultWorkspace = {
  id: "workspace-1",
  name: "project-dev",
};

type TerminalSurface = {
  id: string;
  surfaceId: string;
  workspaceId: string;
  title: string;
  output: string;
  status: string;
  runtime: string;
  pid?: number;
  lastSequence: number;
};

type Readiness = {
  ready: boolean;
  terminal_runtime_ready?: boolean;
  browser_runtime_ready?: boolean;
};

function TitleBar() {
  const appWindow = useMemo(() => {
    try {
      return getCurrentWindow();
    } catch (err) {
      console.warn("tauri window api unavailable", err);
      return null;
    }
  }, []);

  const handleMin = () => {
    if (appWindow) {
      appWindow.minimize().catch(console.error);
    } else if (typeof window !== "undefined") {
      window.close();
    }
  };

  const handleMax = async () => {
    if (!appWindow) return;
    try {
      const isMax = await appWindow.isMaximized();
      if (isMax) {
        await appWindow.unmaximize();
      } else {
        await appWindow.maximize();
      }
    } catch (err) {
      console.error(err);
    }
  };

  const handleClose = () => {
    if (appWindow) {
      appWindow.close().catch(console.error);
    } else if (typeof window !== "undefined") {
      window.close();
    }
  };

  return (
    <div className="flex items-center border-b bg-card/80 px-3 py-2 text-xs text-muted-foreground backdrop-blur">
      <div className="drag-region flex items-center gap-2" data-tauri-drag-region onDoubleClick={handleMax}>
        <div className="size-2.5 rounded-full bg-primary" />
        <div className="text-sm font-semibold text-foreground" data-tauri-drag-region>
          maxc
        </div>
      </div>
      <div className="ml-auto flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={(e) => {
            e.stopPropagation();
            handleMin();
          }}
          data-tauri-drag-region="false"
          className="no-drag"
          aria-label="Minimize"
        >
          <Minus className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={(e) => {
            e.stopPropagation();
            handleMax();
          }}
          data-tauri-drag-region="false"
          className="no-drag"
          aria-label="Maximize"
        >
          <Square className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={(e) => {
            e.stopPropagation();
            handleClose();
          }}
          data-tauri-drag-region="false"
          className="no-drag"
          aria-label="Close"
        >
          <X className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

function App() {
  const [token, setToken] = useState<string>("");
  const [workspaces, setWorkspaces] = useState([defaultWorkspace]);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState(defaultWorkspace.id);
  const [terminals, setTerminals] = useState<TerminalSurface[]>([]);
  const [backendStatus, setBackendStatus] = useState("Connecting...");
  const [readiness, setReadiness] = useState<Readiness | null>(null);
  const [browserSessionId, setBrowserSessionId] = useState<string>("");

  const terminalsRef = useRef<TerminalSurface[]>([]);
  const pollInFlightRef = useRef(false);
  const spawnInFlightRef = useRef(false);

  const readyForActions = Boolean(token) && (readiness?.ready ?? false);

  const selectedWorkspace = useMemo(
    () => workspaces.find((w) => w.id === selectedWorkspaceId) ?? workspaces[0],
    [workspaces, selectedWorkspaceId],
  );

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
      } catch (error) {
        console.error(error);
        setBackendStatus((error as Error).message);
      }
    })();
  }, []);

  useEffect(() => {
    terminalsRef.current = terminals;
  }, [terminals]);

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
            let output = t.output;
            if (Array.isArray(result.events)) {
              for (const ev of result.events) {
                if (ev.type === "terminal.output" && ev.output) {
                  output += ev.output as string;
                }
                lastSeq = Math.max(lastSeq, ev.sequence ?? lastSeq);
              }
            }
            return {
              ...t,
              output,
              status: result.status ?? t.status,
              runtime: result.runtime ?? t.runtime,
              pid: result.pid ?? t.pid,
              lastSequence: lastSeq,
            } as TerminalSurface;
          }),
        );
        terminalsRef.current = updated;
        setTerminals(updated);
      } catch (err) {
        console.error(err);
        setBackendStatus((err as Error).message);
      } finally {
        pollInFlightRef.current = false;
      }
    };

    const id = setInterval(poll, 1500);
    return () => clearInterval(id);
  }, [token]);

  async function addWorkspace() {
    const index = workspaces.length + 1;
    const newWs = { id: "workspace-" + index, name: "workspace-" + index };
    setWorkspaces((w) => [...w, newWs]);
    setSelectedWorkspaceId(newWs.id);
  }

  async function addTerminal() {
    if (!readyForActions || !selectedWorkspace) {
      setBackendStatus("Backend not ready for terminal.spawn yet");
      return;
    }
    if (spawnInFlightRef.current) {
      setBackendStatus("terminal.spawn in progress…");
      return;
    }
    const surfaceId = "surface-" + crypto.randomUUID();
    try {
      spawnInFlightRef.current = true;
      setBackendStatus("Spawning terminal…");
      const result = await rpc<any>("terminal.spawn", {
        command_id: "ui-term-spawn-" + crypto.randomUUID(),
        workspace_id: selectedWorkspace.id,
        surface_id: surfaceId,
        cols: 120,
        rows: 34,
        auth: { token },
      });

      let initialOutput = "";
      if (result.terminal_session_id) {
        const history = await rpc<any>("terminal.history", {
          workspace_id: selectedWorkspace.id,
          surface_id: surfaceId,
          terminal_session_id: result.terminal_session_id,
          from_sequence: 0,
          auth: { token },
        });
        if (Array.isArray(history.events)) {
          for (const ev of history.events) {
            if (ev.type === "terminal.output" && ev.output) {
              initialOutput += ev.output as string;
            }
          }
        }
      }

      const next = [
        ...terminalsRef.current,
        {
          id: result.terminal_session_id,
          surfaceId,
          workspaceId: selectedWorkspace.id,
          title: "Terminal " + (terminalsRef.current.length + 1),
          output: initialOutput,
          status: result.status ?? "running",
          runtime: result.runtime ?? "unknown",
          pid: result.pid,
          lastSequence: result.last_sequence ?? 0,
        } as TerminalSurface,
      ];
      terminalsRef.current = next;
      setTerminals(next);
      setBackendStatus("Terminal spawned");
    } catch (error) {
      console.error(error);
      setBackendStatus((error as Error).message);
    } finally {
      spawnInFlightRef.current = false;
    }
  }

  async function openBrowser() {
    if (!readyForActions || !selectedWorkspace) {
      setBackendStatus("Backend not ready for browser.create yet");
      return;
    }
    try {
      const result = await rpc<any>("browser.create", {
        command_id: "ui-browser-" + crypto.randomUUID(),
        workspace_id: selectedWorkspace.id,
        surface_id: "surface-browser-" + crypto.randomUUID(),
        auth: { token },
      });
      setBrowserSessionId(result.browser_session_id);
      setBackendStatus("Browser ready (" + (result.runtime ?? "runtime unknown") + ")");
    } catch (error) {
      console.error(error);
      setBackendStatus((error as Error).message);
    }
  }

  function renderWorkspaceSidebar() {
    return (
      <aside className="h-[calc(100vh-40px)] w-56 border-r bg-sidebar p-3 flex flex-col gap-2.5 text-[12px]">
        <div className="flex items-center justify-between">
          <div className="text-xs font-semibold uppercase tracking-wide">Workspaces</div>
          <Button size="icon-sm" variant="ghost" onClick={addWorkspace}>
            <Plus className="size-4" />
          </Button>
        </div>
        <div className="space-y-2">
          {workspaces.map((ws) => (
            <button
              key={ws.id}
              onClick={() => setSelectedWorkspaceId(ws.id)}
              className={cn(
                "w-full rounded-md border px-3 py-1.5 text-left text-xs transition",
                selectedWorkspaceId === ws.id
                  ? "border-primary/50 bg-primary/5"
                  : "border-border hover:border-primary/40 hover:bg-primary/5",
              )}
            >
              <div className="font-medium">{ws.name}</div>
              <div className="text-[11px] text-muted-foreground">{ws.id}</div>
            </button>
          ))}
        </div>
        <div className="mt-auto text-xs text-muted-foreground">
          {readiness ? "Ready: " + readiness.ready : backendStatus}
        </div>
      </aside>
    );
  }

  function renderToolbar() {
    return (
      <div className="flex items-center gap-2 border-b bg-card px-3 py-2.5 text-[12px]">
        <Button variant="secondary" size="sm" onClick={addTerminal} disabled={!readyForActions} className="h-8 px-2.5">
          <TerminalIcon className="size-3.5" />
          <span>New Terminal</span>
        </Button>
        <Button variant="ghost" size="sm" onClick={openBrowser} disabled={!readyForActions} className="h-8 px-2.5">
          <Globe2 className="size-3.5" />
          <span>{browserSessionId ? "Browser Ready" : "Open Browser"}</span>
        </Button>
        <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          <ActivitySquare className="size-4" />
          <span>{backendStatus}</span>
        </div>
      </div>
    );
  }

  function renderTerminals() {
    if (!terminals.length) {
      return (
        <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
          No terminals yet. Use "New Terminal" to spawn.
        </div>
      );
    }
    return (
      <div className="grid grid-cols-1 gap-3 p-3">
        {terminals.map((t) => (
          <div key={t.id} className="rounded-lg border bg-card p-3 shadow-sm">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-semibold">
                <TerminalIcon className="size-4" />
                {t.title}
              </div>
              <div className="text-xs text-muted-foreground">
                {t.status} · {t.runtime} · pid {t.pid ?? "-"}
              </div>
            </div>
            <pre className="mt-2 max-h-72 overflow-auto rounded-md bg-muted p-2 text-xs">
{t.output || ""}
            </pre>
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-background text-foreground overflow-hidden text-[13px]">
      <TitleBar />
      <div className="flex flex-1 min-h-0">
        {renderWorkspaceSidebar()}
        <main className="flex flex-1 min-h-0 flex-col overflow-hidden">
          {renderToolbar()}
          <div className="flex-1 overflow-auto">{renderTerminals()}</div>
        </main>
      </div>
    </div>
  );
}

export default App;
