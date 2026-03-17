import { useEffect, useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Check,
  ArrowUpDown,
  Bot,
  ChevronDown,
  Download,
  Info,
  Keyboard,
  Plus,
  Trash2,
} from "lucide-react";
import logoWhite from "@/assets/maxc_logo_white.svg";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type EnvEntry = { key: string; value: string };

type Workspace = {
  workspace_id: string;
  name: string;
  folder: string;
  env_vars: Record<string, string>;
  created_at_ms: number;
};

type UpdateInfo = {
  available: boolean;
  version?: string;
  current_version?: string;
  date_ms?: number | null;
  body?: string | null;
};

type ShortcutItem = {
  id: string;
  label: string;
  keys: string;
  defaultKeys: string;
};

type RpcRateLimitValue = number | "unlimited";

const RPC_ENDPOINT_GROUPS: { title: string; methods: string[] }[] = [
  {
    title: "Session",
    methods: ["session.create", "session.refresh", "session.revoke"],
  },
  {
    title: "System",
    methods: ["system.health", "system.readiness", "system.diagnostics", "system.metrics", "system.logs"],
  },
  {
    title: "Workspace",
    methods: [
      "workspace.create",
      "workspace.list",
      "workspace.update",
      "workspace.delete",
      "workspace.layout",
    ],
  },
  {
    title: "Pane",
    methods: ["pane.create", "pane.split", "pane.list", "pane.close", "pane.resize"],
  },
  {
    title: "Surface",
    methods: ["surface.create", "surface.list", "surface.close", "surface.focus"],
  },
  {
    title: "Terminal (PowerShell)",
    methods: [
      "terminal.spawn",
      "terminal.input",
      "terminal.resize",
      "terminal.history",
      "terminal.subscribe",
      "terminal.kill",
    ],
  },
  {
    title: "Browser",
    methods: [
      "browser.create",
      "browser.attach",
      "browser.detach",
      "browser.close",
      "browser.tab.open",
      "browser.tab.list",
      "browser.tab.focus",
      "browser.tab.close",
      "browser.goto",
      "browser.reload",
      "browser.back",
      "browser.forward",
      "browser.click",
      "browser.type",
      "browser.key",
      "browser.wait",
      "browser.screenshot",
      "browser.evaluate",
      "browser.cookie.get",
      "browser.cookie.set",
      "browser.storage.get",
      "browser.storage.set",
      "browser.network.intercept",
      "browser.upload",
      "browser.download",
      "browser.trace.start",
      "browser.trace.stop",
      "browser.raw.command",
      "browser.history",
      "browser.subscribe",
    ],
  },
  {
    title: "Agent",
    methods: [
      "agent.worker.create",
      "agent.worker.list",
      "agent.worker.get",
      "agent.worker.close",
      "agent.task.start",
      "agent.task.list",
      "agent.task.get",
      "agent.task.cancel",
      "agent.attach.terminal",
      "agent.detach.terminal",
      "agent.attach.browser",
      "agent.detach.browser",
    ],
  },
  {
    title: "Notification",
    methods: ["notification.send", "notification.list", "notification.clear"],
  },
];

export type SettingsDialogProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Which tab to show on open */
  defaultTab?: string;
  /** Auth token */
  token: string;
  /** Current workspace being edited (null = settings mode) */
  editWorkspace: Workspace | null;
  /** App version string */
  appVersion: string;
  /** Named pipe path */
  socketPath: string;
  /** Current workspace/pane/surface IDs for env block */
  selectedWorkspaceId: string;
  focusedPaneId: string;
  focusedSurfaceId: string;
  /** Shortcuts */
  shortcuts: ShortcutItem[];
  onShortcutChange: (id: string, keys: string) => void;
  onResetShortcuts: () => void;
  /** Rate limit */
  rpcRateLimit: RpcRateLimitValue;
  onRpcRateLimitChange: (value: string) => void;
  /** Callbacks */
  onSaveWorkspaceEnv: (workspaceId: string, envVars: Record<string, string>) => Promise<void>;
  onStatusMessage: (msg: string) => void;
};

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SettingsDialog({
  open: isOpen,
  onOpenChange,
  defaultTab = "workspace",
  token,
  editWorkspace,
  appVersion,
  socketPath,
  selectedWorkspaceId,
  focusedPaneId,
  focusedSurfaceId,
  shortcuts,
  onShortcutChange,
  onResetShortcuts,
  rpcRateLimit,
  onRpcRateLimitChange,
  onSaveWorkspaceEnv,
  onStatusMessage,
}: SettingsDialogProps) {
  // -- Edit workspace state --
  const [editEnvVars, setEditEnvVars] = useState<EnvEntry[]>(() => {
    if (!editWorkspace) return [];
    return Object.entries(editWorkspace.env_vars || {})
      .filter(([k]) => !k.startsWith("MAXC_"))
      .map(([key, value]) => ({ key, value }));
  });
  const [editSaving, setEditSaving] = useState(false);
  const [editError, setEditError] = useState("");

  // -- Update tab state --
  const [updateChannel, setUpdateChannel] = useState<"stable" | "beta">(() => {
    try { return (localStorage.getItem("maxc-update-channel") as "stable" | "beta") || "stable"; }
    catch { return "stable"; }
  });
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "available" | "uptodate" | "downloading" | "ready" | "error"
  >("idle");
  const [updateError, setUpdateError] = useState("");
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);

  // -- Agent tab state --
  const [envCopied, setEnvCopied] = useState(false);
  const [activeTab, setActiveTab] = useState(defaultTab);
  const [captureShortcutId, setCaptureShortcutId] = useState<string | null>(null);

  const isEditing = editWorkspace !== null;
  const ws = editWorkspace;
  const rateLimitValue = rpcRateLimit === "unlimited" ? "unlimited" : String(rpcRateLimit);
  const showUnlimitedWarning = rpcRateLimit === "unlimited";
  const isDownloading = updateStatus === "downloading";

  useEffect(() => {
    if (!isOpen) return;
    const nextTab = !isEditing && defaultTab === "workspace" ? "updates" : defaultTab;
    setActiveTab(nextTab);
  }, [defaultTab, isEditing, isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    if (editWorkspace) {
      setEditEnvVars(
        Object.entries(editWorkspace.env_vars || {})
          .filter(([k]) => !k.startsWith("MAXC_"))
          .map(([key, value]) => ({ key, value })),
      );
    } else {
      setEditEnvVars([]);
    }
    setEditError("");
  }, [editWorkspace, isOpen]);

  useEffect(() => {
    if (!captureShortcutId) return;

    function handleKeyCapture(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();

      const targetId = captureShortcutId;
      if (!targetId) return;

      if (event.key === "Escape") {
        setCaptureShortcutId(null);
        return;
      }

      if (event.key === "Backspace" || event.key === "Delete") {
        onShortcutChange(targetId, "");
        setCaptureShortcutId(null);
        return;
      }

      const formatted = formatShortcut(event);
      if (!formatted) return;
      onShortcutChange(targetId, formatted);
      setCaptureShortcutId(null);
    }

    window.addEventListener("keydown", handleKeyCapture, true);
    return () => window.removeEventListener("keydown", handleKeyCapture, true);
  }, [captureShortcutId, onShortcutChange]);

  function formatShortcut(event: KeyboardEvent) {
    const key = event.key;
    if (key === "Control" || key === "Shift" || key === "Alt" || key === "Meta") return "";
    const parts: string[] = [];
    if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
    if (event.shiftKey) parts.push("Shift");
    if (event.altKey) parts.push("Alt");
    let mainKey = key;
    if (mainKey === " ") mainKey = "Space";
    if (mainKey.length === 1) mainKey = mainKey.toUpperCase();
    parts.push(mainKey);
    return parts.join("+");
  }

  // -- Handlers --

  async function handleSaveEnv() {
    if (!ws) return;
    const envObj: Record<string, string> = {};
    for (const e of editEnvVars) { const k = e.key.trim(); if (k) envObj[k] = e.value; }
    for (const [key, value] of Object.entries(ws.env_vars || {})) {
      if (key.startsWith("MAXC_") && !(key in envObj)) envObj[key] = value;
    }
    setEditSaving(true); setEditError("");
    try {
      await onSaveWorkspaceEnv(ws.workspace_id, envObj);
      onOpenChange(false);
    } catch (err) {
      setEditError((err as Error).message);
    } finally { setEditSaving(false); }
  }

  async function checkForUpdates() {
    setUpdateStatus("checking"); setUpdateError(""); setUpdateInfo(null);
    try {
      const result = await invoke<UpdateInfo>("update_check", { channel: updateChannel });
      if (!result.available) { setUpdateStatus("uptodate"); return; }
      setUpdateInfo(result);
      setUpdateStatus("available");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      if (/404|not found|no release/i.test(msg)) { setUpdateStatus("uptodate"); return; }
      setUpdateStatus("error"); setUpdateError(msg || "Update check failed");
    }
  }

  async function downloadAndInstall() {
    setUpdateStatus("downloading"); setUpdateError("");
    try {
      await invoke("update_download_and_install", { channel: updateChannel });
      setUpdateStatus("ready");
    } catch (err) {
      setUpdateStatus("error"); setUpdateError((err as Error).message || "Update failed");
    }
  }

  function handleChannelChange(ch: "stable" | "beta") {
    setUpdateChannel(ch);
    try { localStorage.setItem("maxc-update-channel", ch); } catch {}
  }

  const envLines = [
    `MAXC_SOCKET_PATH=${socketPath}`,
    token ? `MAXC_TOKEN=${token}` : "MAXC_TOKEN=",
    selectedWorkspaceId ? `MAXC_WORKSPACE_ID=${selectedWorkspaceId}` : "MAXC_WORKSPACE_ID=",
    focusedPaneId ? `MAXC_PANE_ID=${focusedPaneId}` : "MAXC_PANE_ID=",
    focusedSurfaceId ? `MAXC_SURFACE_ID=${focusedSurfaceId}` : "MAXC_SURFACE_ID=",
  ];
  const envBlock = envLines.join("\n");

  async function copyEnvBlock() {
    try {
      await navigator.clipboard.writeText(envBlock);
      setEnvCopied(true);
      setTimeout(() => setEnvCopied(false), 2000);
    } catch { onStatusMessage("Failed to copy"); }
  }

  // -- Env var helpers (shared between create/edit) --
  function envAdd(setter: Dispatch<SetStateAction<EnvEntry[]>>) {
    setter((p) => [...p, { key: "", value: "" }]);
  }
  function envUpdate(setter: Dispatch<SetStateAction<EnvEntry[]>>, i: number, field: "key" | "value", val: string) {
    setter((p) => p.map((e, idx) => (idx === i ? { ...e, [field]: val } : e)));
  }
  function envRemove(setter: Dispatch<SetStateAction<EnvEntry[]>>, i: number) {
    setter((p) => p.filter((_, idx) => idx !== i));
  }

  function renderEnvEditor(entries: EnvEntry[], setter: Dispatch<SetStateAction<EnvEntry[]>>) {
    return (
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
            Environment Variables
          </label>
          <Button variant="ghost" size="icon-xs" onClick={() => envAdd(setter)}>
            <Plus className="size-3.5" />
          </Button>
        </div>
        {entries.length === 0 ? (
          <div className="text-[11px] text-muted-foreground/60 rounded-md border border-dashed px-3 py-2 text-center">
            No variables. Click + to add.
          </div>
        ) : (
          <div className="space-y-1.5">
            {entries.map((entry, i) => (
              <div key={i} className="flex items-center gap-1.5">
                <input
                  type="text"
                  value={entry.key}
                  onChange={(e) => envUpdate(setter, i, "key", e.target.value)}
                  placeholder="KEY"
                  className="flex-1 rounded-md border bg-muted px-2 py-1.5 text-[11px] font-mono outline-none focus:ring-1 focus:ring-ring"
                />
                <span className="text-muted-foreground text-[11px]">=</span>
                <input
                  type="text"
                  value={entry.value}
                  onChange={(e) => envUpdate(setter, i, "value", e.target.value)}
                  placeholder="value"
                  className="flex-1 rounded-md border bg-muted px-2 py-1.5 text-[11px] font-mono outline-none focus:ring-1 focus:ring-ring"
                />
                <Button variant="ghost" size="icon-xs" onClick={() => envRemove(setter, i)}>
                  <Trash2 className="size-3 text-muted-foreground" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg h-[55vh] overflow-hidden flex flex-col" showCloseButton>
        <DialogHeader>
          <DialogTitle>{isEditing ? "Workspace Settings" : "Settings"}</DialogTitle>
          <DialogDescription>
            {isEditing
              ? `Configure ${ws?.name || "workspace"}`
              : "Manage workspaces, agent connection, and updates."}
          </DialogDescription>
        </DialogHeader>

        <Tabs value={activeTab} onValueChange={setActiveTab} className="flex-1 min-h-0">
          <TabsList variant="line" className="w-full justify-start">
            {isEditing && (
              <TabsTrigger value="workspace">
                Workspace
              </TabsTrigger>
            )}
            <TabsTrigger value="agent" className="gap-1.5">
              <Bot className="size-3.5" />
              Agent
            </TabsTrigger>
            <TabsTrigger value="shortcuts" className="gap-1.5">
              <Keyboard className="size-3.5" />
              Shortcuts
            </TabsTrigger>
            <TabsTrigger value="rate-limit" className="gap-1.5">
              <ArrowUpDown className="size-3.5" />
              Rate Limit
            </TabsTrigger>
            <TabsTrigger value="updates" className="gap-1.5">
              <Download className="size-3.5" />
              Updates
            </TabsTrigger>
            <TabsTrigger value="about" className="gap-1.5">
              <Info className="size-3.5" />
              About
            </TabsTrigger>
          </TabsList>

          {/* ============ WORKSPACE TAB ============ */}
          {isEditing && (
            <TabsContent value="workspace" className="settings-scroll overflow-auto h-[55vh] pr-1 space-y-4 mt-3">
              <div className="space-y-1.5">
                <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  Working Directory
                </label>
                <div className="rounded-md border bg-muted px-3 py-2 text-[11px] text-muted-foreground font-mono truncate">
                  {ws?.folder || "No folder set"}
                </div>
              </div>

              {renderEnvEditor(editEnvVars, setEditEnvVars)}

              <div className="space-y-1.5">
                <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  MAXC Environment (read-only)
                </label>
                <pre className="rounded-md border bg-muted px-3 py-2 text-[10px] font-mono text-muted-foreground whitespace-pre-wrap break-words">
                  {Object.entries(ws?.env_vars ?? {})
                    .filter(([k]) => k.startsWith("MAXC_"))
                    .map(([k, v]) => `${k}=${v}`)
                    .join("\n") || "Not available yet."}
                </pre>
              </div>

              {editError && (
                <div className="text-[11px] text-destructive bg-destructive/10 rounded-md px-3 py-2">{editError}</div>
              )}
              <div className="flex gap-2 pt-1">
                <Button variant="outline" size="sm" onClick={() => onOpenChange(false)} disabled={editSaving}>
                  Cancel
                </Button>
                <Button size="sm" onClick={handleSaveEnv} disabled={editSaving}>
                  {editSaving ? "Saving..." : "Save"}
                </Button>
              </div>
            </TabsContent>
          )}

          {/* ============ AGENT TAB ============ */}
          <TabsContent value="agent" className="settings-scroll overflow-auto h-[55vh] pr-1 space-y-4 mt-3">
            <div className="space-y-1.5">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Agent Connection
              </label>
              <p className="text-[11px] text-muted-foreground leading-relaxed">
                Use these environment variables to let an external agent access all maxc commands.
                Agents launched inside a maxc terminal inherit these values automatically.
              </p>
            </div>

            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  MAXC Environment
                </label>
                <Button variant="outline" size="sm" onClick={copyEnvBlock} className="h-6 text-[10px] gap-1.5">
                  {envCopied ? <><Check className="size-3" /> Copied</> : "Copy"}
                </Button>
              </div>
              <pre className="rounded-md border bg-muted px-3 py-2 text-[10px] font-mono text-muted-foreground whitespace-pre-wrap break-words">
                {envBlock}
              </pre>
            </div>

          </TabsContent>

          {/* ============ UPDATES TAB ============ */}
          <TabsContent value="updates" className="settings-scroll overflow-auto h-[55vh] pr-1 space-y-4 mt-3">
            <div className="space-y-1.5">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Current Version
              </label>
              <div className="rounded-md border bg-muted px-3 py-2 text-[11px] font-mono">
                {appVersion ? `v${appVersion}` : "Unknown"}
              </div>
            </div>

            <div className="space-y-2">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Update Channel
              </label>
              <div className="flex items-center gap-2">
                <div className="relative">
                  <select
                    value={updateChannel}
                    onChange={(e) => handleChannelChange(e.target.value as "stable" | "beta")}
                    className="h-7 appearance-none rounded-md border bg-muted pl-2 pr-7 text-[11px] outline-none focus:ring-1 focus:ring-ring cursor-pointer"
                  >
                    <option value="stable">Stable</option>
                    <option value="beta">Beta</option>
                  </select>
                  <ChevronDown className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground" />
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 text-[11px]"
                  onClick={checkForUpdates}
                  disabled={updateStatus === "checking" || isDownloading}
                >
                  {updateStatus === "checking" ? "Checking..." : "Check for Updates"}
                </Button>
              </div>
            </div>

            {updateStatus === "available" && updateInfo && (
              <div className="rounded-md border bg-muted px-3 py-3 space-y-2">
                <div className="text-[11px] font-medium">
                  Update available: v{updateInfo.version ?? "new"}
                </div>
                {updateInfo.date_ms && (
                  <div className="text-[10px] text-muted-foreground">
                    {new Date(updateInfo.date_ms).toLocaleString()}
                  </div>
                )}
                {updateInfo.body && (
                  <div className="text-[10px] text-muted-foreground whitespace-pre-wrap leading-relaxed">
                    {updateInfo.body}
                  </div>
                )}
                <Button size="sm" onClick={downloadAndInstall} disabled={isDownloading}>
                  {isDownloading ? "Downloading..." : "Download & Install"}
                </Button>
              </div>
            )}
            {updateStatus === "downloading" && (
              <div className="text-[11px] text-muted-foreground">Downloading update...</div>
            )}
            {updateStatus === "ready" && (
              <div className="text-[11px] text-muted-foreground">Update installed. Restarting...</div>
            )}
            {updateStatus === "uptodate" && (
              <div className="text-[11px] text-emerald-500">You are up to date.</div>
            )}
            {updateStatus === "error" && (
              <div className="text-[11px] text-destructive">{updateError || "Update check failed."}</div>
            )}
          </TabsContent>

          {/* ============ ABOUT TAB ============ */}
          <TabsContent value="about" className="settings-scroll overflow-auto h-[55vh] pr-1 space-y-4 mt-3">
            <div className="flex flex-col items-center gap-3 py-4">
              <img
                src={logoWhite}
                alt="maxc"
                className="h-12 w-auto select-none"
                draggable={false}
              />
              <div className="text-[11px] text-muted-foreground font-mono">
                {appVersion ? `Version ${appVersion}` : "Version unknown"}
              </div>
            </div>

            <div className="space-y-2 text-[11px] text-muted-foreground leading-relaxed">
              <p>
                A programmable developer workspace for terminals, browsers, and AI agents.
                Built with Rust, Tauri, and React.
              </p>
              <p>
                maxc is agent-agnostic. Any AI coding agent that can run shell commands
                can control terminals, browsers, and tasks through the CLI or JSON-RPC API.
              </p>
            </div>

            <div className="grid grid-cols-2 gap-2 text-[10px]">
              {[
                { label: "Backend", value: "Rust + Event Sourcing" },
                { label: "Frontend", value: "Tauri + React + xterm.js" },
                { label: "RPC Methods", value: "52 methods" },
                { label: "License", value: "Apache-2.0" },
              ].map((item) => (
                <div key={item.label} className="rounded-md border bg-muted px-3 py-2">
                  <div className="text-muted-foreground">{item.label}</div>
                  <div className="text-foreground font-medium mt-0.5">{item.value}</div>
                </div>
              ))}
            </div>

            <div className="flex gap-2 pt-1">
              <Button variant="outline" size="sm" className="text-[11px]" asChild>
                <a href="https://github.com/Pollux-Studio/maxc" target="_blank" rel="noopener noreferrer">
                  GitHub
                </a>
              </Button>
              <Button variant="outline" size="sm" className="text-[11px]" asChild>
                <a href="https://maxc.polluxstudio.in/" target="_blank" rel="noopener noreferrer">
                  Website
                </a>
              </Button>
            </div>
          </TabsContent>

          {/* ============ SHORTCUTS TAB ============ */}
          <TabsContent value="shortcuts" className="settings-scroll overflow-auto h-[55vh] pr-1 space-y-4 mt-3">
            <div className="flex items-center justify-between">
              <div>
                <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  Keyboard Shortcuts
                </div>
                <div className="text-[10px] text-muted-foreground mt-1">
                  Use a format like <span className="font-mono">Ctrl+Shift+N</span>.
                </div>
              </div>
              <Button variant="outline" size="sm" className="h-7 text-[11px]" onClick={onResetShortcuts}>
                Reset
              </Button>
            </div>
            <div className="space-y-2">
              {shortcuts.map((shortcut) => (
                <div key={shortcut.id} className="flex items-center gap-2">
                  <div className="flex-1">
                    <div className="text-[11px] font-medium text-foreground">{shortcut.label}</div>
                    <div className="text-[10px] text-muted-foreground">
                      Default: <span className="font-mono">{shortcut.defaultKeys}</span>
                    </div>
                  </div>
                  <Button
                    variant={captureShortcutId === shortcut.id ? "default" : "outline"}
                    size="sm"
                    className="h-7 px-2 text-[11px] font-mono"
                    onClick={() =>
                      setCaptureShortcutId((prev) => (prev === shortcut.id ? null : shortcut.id))
                    }
                  >
                    {captureShortcutId === shortcut.id
                      ? "Press keys..."
                      : shortcut.keys || "Unassigned"}
                  </Button>
                </div>
              ))}
            </div>
            <div className="rounded-md border bg-muted/50 px-3 py-2 text-[10px] text-muted-foreground">
              Click a shortcut to capture keys. Press Escape to cancel, Backspace/Delete to clear.
            </div>
          </TabsContent>

          {/* ============ RATE LIMIT TAB ============ */}
          <TabsContent value="rate-limit" className="settings-scroll overflow-auto h-[55vh] pr-1 space-y-4 mt-3">
            <div className="space-y-2">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                RPC Rate Limit
              </label>
              <div className="flex items-center gap-2">
                <select
                  value={rateLimitValue}
                  onChange={(e) => onRpcRateLimitChange(e.target.value)}
                  className="h-8 w-[160px] appearance-none rounded-md border bg-muted px-2 text-[11px] outline-none focus:ring-1 focus:ring-ring"
                >
                  <option value="5">5 req/s</option>
                  <option value="10">10 req/s</option>
                  <option value="20">20 req/s</option>
                  <option value="30">30 req/s</option>
                  <option value="60">60 req/s</option>
                  <option value="120">120 req/s</option>
                  <option value="unlimited">Unlimited</option>
                </select>
                <span className="text-[10px] text-muted-foreground">
                  Applies to frontend JSON-RPC requests.
                </span>
              </div>
              {showUnlimitedWarning && (
                <div className="rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
                  Unlimited rate limit can increase system resource usage.
                </div>
              )}
            </div>

            <div className="space-y-3">
              <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                RPC Endpoints
              </div>
              <div className="space-y-3">
                {RPC_ENDPOINT_GROUPS.map((group) => (
                  <div key={group.title} className="space-y-1">
                    <div className="text-[11px] font-medium text-foreground">{group.title}</div>
                    <div className="flex flex-wrap gap-1">
                      {group.methods.map((method) => (
                        <span
                          key={method}
                          className="rounded border bg-muted px-2 py-0.5 text-[10px] font-mono text-muted-foreground"
                        >
                          {method}
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
