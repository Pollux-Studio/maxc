import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Play, Square } from "lucide-react";
import { Button } from "@/components/ui/button";
import { XtermTerminal, type XtermHandle } from "./XtermTerminal";
import { cn } from "@/lib/utils";

type AgentWorker = {
  agent_worker_id: string;
  status: string;
  terminal_session_id: string | null;
  browser_session_id: string | null;
  current_task_id: string | null;
  current_task?: {
    agent_task_id: string;
    status: string;
    last_output_sequence?: number;
    failure_reason?: string | null;
  } | null;
};

type AgentPanelProps = {
  token: string;
  workspaceId: string;
  surfaceId: string;
  workspaceFolder?: string;
};

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

export function AgentPanel({
  token,
  workspaceId,
  surfaceId,
  workspaceFolder,
}: AgentPanelProps) {
  const [worker, setWorker] = useState<AgentWorker | null>(null);
  const [prompt, setPrompt] = useState("");
  const [error, setError] = useState("");
  const lastSeqRef = useRef(0);
  const xtermRef = useRef<XtermHandle | null>(null);

  const terminalSessionId = worker?.terminal_session_id ?? null;
  const taskStatus = worker?.current_task?.status ?? null;
  const parsedMaxc = useMemo(() => parseMaxcCommand(prompt), [prompt]);

  const createWorker = useCallback(async () => {
    const created = await rpc<any>("agent.worker.create", {
      command_id: "ui-agent-worker-" + crypto.randomUUID(),
      workspace_id: workspaceId,
      surface_id: surfaceId,
      cwd: workspaceFolder || ".",
      auth: { token },
    });
    const nextWorker: AgentWorker = {
      agent_worker_id: created.agent_worker_id,
      status: created.status ?? "ready",
      terminal_session_id: created.terminal_session_id ?? null,
      browser_session_id: created.browser_session_id ?? null,
      current_task_id: null,
      current_task: null,
    };
    setWorker(nextWorker);
    return nextWorker;
  }, [surfaceId, token, workspaceFolder, workspaceId]);

  // -- Load or create worker --
  const loadWorker = useCallback(async () => {
    try {
      const list = await rpc<{ workers: AgentWorker[] }>("agent.worker.list", {
        workspace_id: workspaceId,
        surface_id: surfaceId,
        auth: { token },
      });
      const active = list.workers.find((w: any) => !w.closed);
      if (active) {
        setWorker(active);
        return;
      }
      await createWorker();
    } catch (err) {
      setError((err as Error).message);
    }
  }, [surfaceId, token, workspaceFolder, workspaceId, createWorker]);

  useEffect(() => {
    loadWorker();
  }, [loadWorker]);

  // -- Poll worker status --
  useEffect(() => {
    if (!worker?.agent_worker_id) return;
    const id = setInterval(async () => {
      try {
        const res = await rpc<any>("agent.worker.get", {
          workspace_id: workspaceId,
          surface_id: surfaceId,
          agent_worker_id: worker.agent_worker_id,
          auth: { token },
        });
        setWorker({
          agent_worker_id: res.agent_worker_id,
          status: res.status,
          terminal_session_id: res.terminal_session_id ?? null,
          browser_session_id: res.browser_session_id ?? null,
          current_task_id: res.current_task_id ?? null,
          current_task: res.current_task ?? null,
        });
      } catch (err) {
        setError((err as Error).message);
      }
    }, 1000);
    return () => clearInterval(id);
  }, [surfaceId, token, worker?.agent_worker_id, workspaceId]);

  // -- Poll terminal output → write to xterm.js --
  useEffect(() => {
    if (!terminalSessionId) return;
    const id = setInterval(async () => {
      try {
        const history = await rpc<any>("terminal.history", {
          workspace_id: workspaceId,
          surface_id: surfaceId,
          terminal_session_id: terminalSessionId,
          from_sequence: lastSeqRef.current + 1,
          max_events: 64,
          auth: { token },
        });
        if (Array.isArray(history.events)) {
          for (const ev of history.events) {
            if (ev.type === "terminal.output" && ev.output) {
              xtermRef.current?.write(ev.output as string);
            }
            if (typeof ev.sequence === "number") {
              lastSeqRef.current = Math.max(lastSeqRef.current, ev.sequence);
            }
          }
        }
      } catch (err) {
        setError((err as Error).message);
      }
    }, 500);
    return () => clearInterval(id);
  }, [surfaceId, terminalSessionId, token, workspaceId]);

  // -- Send keyboard input to agent terminal --
  const sendInput = useCallback(
    async (data: string) => {
      if (!terminalSessionId || !token) return;
      try {
        await rpc("terminal.input", {
          command_id: "ui-agent-input-" + crypto.randomUUID(),
          workspace_id: workspaceId,
          surface_id: surfaceId,
          terminal_session_id: terminalSessionId,
          input: data,
          auth: { token },
        });
      } catch (err) {
        console.error("agent terminal.input error:", err);
      }
    },
    [surfaceId, terminalSessionId, token, workspaceId],
  );

  const startTask = useCallback(async () => {
    if (!worker?.agent_worker_id || !prompt.trim()) return;
    setError("");
    try {
      const promptTrimmed = prompt.trim();
      const enrichedPrompt = parsedMaxc
        ? `${promptTrimmed}\n\n[maxc_command]\ncommand=${parsedMaxc.command || ""}\nsubcommand=${parsedMaxc.subcommand || ""}\n${Object.entries(parsedMaxc.flags)
            .map(([key, value]) => `--${key}=${value}`)
            .join("\n")}`
        : promptTrimmed;

      const runStart = async (workerId: string) => {
        const result = await rpc<any>("agent.task.start", {
          command_id: "ui-agent-task-" + crypto.randomUUID(),
          workspace_id: workspaceId,
          surface_id: surfaceId,
          agent_worker_id: workerId,
          prompt: enrichedPrompt,
          auth: { token },
        });
        if (typeof result?.last_output_sequence === "number") {
          lastSeqRef.current = result.last_output_sequence;
        }
        if (result?.agent_task_id) {
          setWorker((prev) =>
            prev
              ? {
                  ...prev,
                  status: "running",
                  current_task_id: result.agent_task_id,
                  current_task: {
                    agent_task_id: result.agent_task_id,
                    status: result.status ?? "running",
                    last_output_sequence: result.last_output_sequence,
                    failure_reason: null,
                  },
                }
              : prev,
          );
        }
        return result;
      };

      const resolveCurrentTaskId = async () => {
        if (worker.current_task_id) return worker.current_task_id;
        try {
          const res = await rpc<any>("agent.worker.get", {
            workspace_id: workspaceId,
            surface_id: surfaceId,
            agent_worker_id: worker.agent_worker_id,
            auth: { token },
          });
          if (res?.current_task_id) {
            setWorker((prev) =>
              prev
                ? {
                    ...prev,
                    status: res.status ?? prev.status,
                    current_task_id: res.current_task_id ?? prev.current_task_id,
                    current_task: res.current_task ?? prev.current_task,
                  }
                : prev,
            );
            return res.current_task_id as string;
          }
        } catch {
          // ignore resolve failures
        }
        return null;
      };

      const tryStart = async () => {
        await runStart(worker.agent_worker_id);
        return true;
      };

      try {
        await tryStart();
      } catch (err) {
        const message = (err as Error).message || "";
        if (message.startsWith("CONFLICT")) {
          const currentTaskId = await resolveCurrentTaskId();
          if (currentTaskId) {
            try {
              await rpc("agent.task.cancel", {
                command_id: "ui-agent-cancel-" + crypto.randomUUID(),
                workspace_id: workspaceId,
                surface_id: surfaceId,
                agent_task_id: currentTaskId,
                reason: "restarted by user",
                auth: { token },
              });
            } catch {
              // ignore conflict from cancel
            }
          }
          try {
            await tryStart();
          } catch (retryErr) {
            const retryMsg = (retryErr as Error).message || "";
            if (retryMsg.startsWith("CONFLICT")) {
              // reset worker
              try {
                await rpc("agent.worker.close", {
                  command_id: "ui-agent-worker-close-" + crypto.randomUUID(),
                  workspace_id: workspaceId,
                  surface_id: surfaceId,
                  agent_worker_id: worker.agent_worker_id,
                  auth: { token },
                });
              } catch {
                // ignore
              }
              const newWorker = await createWorker();
              await runStart(newWorker.agent_worker_id);
            } else {
              throw retryErr;
            }
          }
        } else {
          throw err;
        }
      }
      setPrompt("");
    } catch (err) {
      setError((err as Error).message);
    }
  }, [prompt, parsedMaxc, surfaceId, token, worker?.agent_worker_id, worker?.current_task_id, worker?.status, workspaceId, createWorker]);

  const cancelTask = useCallback(async () => {
    if (!worker?.current_task_id) return;
    setError("");
    try {
      await rpc("agent.task.cancel", {
        command_id: "ui-agent-cancel-" + crypto.randomUUID(),
        workspace_id: workspaceId,
        surface_id: surfaceId,
        agent_task_id: worker.current_task_id,
        reason: "cancelled by user",
        auth: { token },
      });
    } catch (err) {
      setError((err as Error).message);
    }
  }, [surfaceId, token, worker?.current_task_id, workspaceId]);

  const statusLabel = useMemo(() => {
    if (!worker) return "Initializing";
    if (taskStatus === "running") return "Running";
    if (worker.status === "ready") return "Ready";
    return worker.status;
  }, [taskStatus, worker]);

  return (
    <div className="flex h-full flex-col">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-[#1f1f1f] bg-[#0f0f0f]/90 px-3 py-2">
        <div className="text-[11px] text-[#aaa]">
          Agent Worker:{" "}
          <span className="text-[#ddd]">{worker?.agent_worker_id ?? "..."}</span>
        </div>
        <span
          className={cn(
            "rounded-full px-2 py-0.5 text-[10px]",
            statusLabel === "Running"
              ? "bg-chart-1/20 text-chart-1"
              : "bg-muted text-foreground/70",
          )}
        >
          {statusLabel}
        </span>
      </div>

      {/* Error bar */}
      {error && (
        <div className="px-3 py-2 text-[11px] text-destructive bg-destructive/10 border-b border-destructive/20">
          {error}
        </div>
      )}

      {/* Prompt input */}
      <div className="flex items-center gap-2 border-b border-[#1f1f1f] bg-[#0c0c0c] px-3 py-2">
        <input
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Describe the task for the agent..."
          autoFocus
          className="flex-1 rounded-md border border-[#2a2a2a] bg-[#0a0a0a] px-2 py-1 text-[11px] text-[#ddd] outline-none"
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              startTask();
            }
          }}
        />
        <Button size="sm" variant="secondary" onClick={startTask} disabled={!prompt.trim()}>
          <Play className="size-3.5" /> Run
        </Button>
        <Button size="sm" variant="outline" onClick={cancelTask} disabled={taskStatus !== "running"}>
          <Square className="size-3.5" /> Stop
        </Button>
      </div>

      {/* Parsed maxc command */}
      {parsedMaxc && (
        <div className="border-b border-[#1f1f1f] bg-[#0b0b0b] px-3 py-2 text-[11px] text-[#bdbdbd] space-y-2">
          <div className="text-[10px] uppercase tracking-wide text-[#7a7a7a]">
            Detected maxc command
          </div>
          <div className="rounded-md border border-[#222] bg-[#0a0a0a] px-2 py-1 text-[11px] text-[#e0e0e0] break-words">
            {parsedMaxc.raw}
          </div>
          <div className="flex flex-wrap gap-2">
            <span className="rounded-full bg-[#1a1a1a] px-2 py-0.5 text-[10px]">
              cmd: {parsedMaxc.command || "unknown"}
            </span>
            {parsedMaxc.subcommand && (
              <span className="rounded-full bg-[#1a1a1a] px-2 py-0.5 text-[10px]">
                sub: {parsedMaxc.subcommand}
              </span>
            )}
          </div>
          {Object.keys(parsedMaxc.flags).length > 0 && (
            <div className="flex flex-wrap gap-2">
              {Object.entries(parsedMaxc.flags).map(([key, value]) => (
                <span key={key} className="rounded-full bg-[#141414] px-2 py-0.5 text-[10px] text-[#cfcfcf]">
                  --{key}={value}
                </span>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Terminal output via xterm.js */}
      <div className="flex-1 min-h-0 bg-[#0c0c0c]">
        <XtermTerminal
          ref={xtermRef}
          focused={false}
          onData={sendInput}
        />
      </div>
    </div>
  );
}

function stripQuotes(value: string) {
  if (
    (value.startsWith("\"") && value.endsWith("\"")) ||
    (value.startsWith("'") && value.endsWith("'"))
  ) {
    return value.slice(1, -1);
  }
  return value;
}

function parseMaxcCommand(text: string) {
  const tokens = text.match(/"[^"]*"|'[^']*'|\S+/g) ?? [];
  const idx = tokens.findIndex((token) => token.toLowerCase() === "maxc");
  if (idx === -1) return null;
  if (tokens.length < idx + 2) return null;

  const raw = tokens.slice(idx).join(" ");
  const command = tokens[idx + 1] ?? "";
  if (!command) return null;

  let subcommand = "";
  let startIdx = idx + 2;
  if (tokens[idx + 2] && !tokens[idx + 2].startsWith("--")) {
    subcommand = tokens[idx + 2];
    startIdx = idx + 3;
  }

  const flags: Record<string, string> = {};
  for (let i = startIdx; i < tokens.length; i += 1) {
    const token = tokens[i];
    if (!token.startsWith("--")) continue;
    const withoutPrefix = token.replace(/^--/, "");
    const [key, inlineValue] = withoutPrefix.split("=", 2);
    if (inlineValue !== undefined) {
      flags[key] = stripQuotes(inlineValue);
      continue;
    }
    const next = tokens[i + 1];
    if (next && !next.startsWith("--")) {
      flags[key] = stripQuotes(next);
      i += 1;
    } else {
      flags[key] = "true";
    }
  }

  return { raw, command, subcommand, flags };
}

export default AgentPanel;
