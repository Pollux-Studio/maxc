import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { getVersion } from "@tauri-apps/api/app";
import {
  Bell,
  ChevronDown,
  ChevronRight,
  Check,
  FolderOpen,
  GitBranch,
  Keyboard,
  Minus,
  Moon,
  Plus,
  Settings,
  Square,
  Sun,
  Terminal as TerminalIcon,
  Trash2,
  X,
} from "lucide-react";
import logoWhite from "./assets/maxc_logo_white.svg";
import { Button } from "@/components/ui/button";
import { type XtermHandle } from "@/components/XtermTerminal";
import { PaneContainer, type PaneNode, type BrowserState } from "@/components/PaneContainer";
import { type SurfaceState } from "@/components/SurfaceTabBar";
import { NotificationPanel, type NotificationItem } from "@/components/NotificationPanel";
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

// (PaneRpc/SurfaceRpc removed — workspace.layout returns the full tree)

type Readiness = {
  ready: boolean;
  terminal_runtime_ready?: boolean;
  browser_runtime_ready?: boolean;
};

type EnvEntry = { key: string; value: string };
type UpdateInfo = {
  available: boolean;
  version?: string;
  current_version?: string;
  date_ms?: number | null;
  body?: string | null;
};

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
  const handleClose = () => appWindow?.close().catch(console.error);

  return (
    <div className="flex items-center border-b bg-card/80 px-3 py-1.5 text-xs text-muted-foreground backdrop-blur">
      <div className="drag-region flex items-center gap-2" data-tauri-drag-region>
        <img
          src={logoWhite}
          alt="maxc"
          className="h-4 w-auto select-none"
          draggable={false}
        />
      </div>
      <div className="ml-auto flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={(e) => { e.stopPropagation(); handleMin(); }}
          className="no-drag"
          aria-label="Minimize"
        >
          <Minus className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          disabled
          className="no-drag text-muted-foreground/40"
          aria-label="Maximize (disabled)"
          title="Maximize disabled"
        >
          <Square className="size-3.5" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={(e) => { e.stopPropagation(); handleClose(); }}
          className="no-drag hover:bg-destructive/15 hover:text-destructive"
          aria-label="Close"
        >
          <X className="size-3.5" />
        </Button>
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
  const [backendStatus, setBackendStatus] = useState("Connecting...");
  const [readiness, setReadiness] = useState<Readiness | null>(null);

  // -- pane tree state --
  const [paneTree, setPaneTree] = useState<PaneNode | null>(null);
  const [focusedPaneId, setFocusedPaneId] = useState("");
  const [surfaces, setSurfaces] = useState<SurfaceState[]>([]);
  const [browserStates, setBrowserStates] = useState<Map<string, BrowserState>>(new Map());

  // -- drawer state --
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [wsName, setWsName] = useState("");
  const [wsFolder, setWsFolder] = useState("");
  const [wsEnvVars, setWsEnvVars] = useState<EnvEntry[]>([]);
  const [wsCreating, setWsCreating] = useState(false);
  const [wsError, setWsError] = useState("");
  const [editDrawerOpen, setEditDrawerOpen] = useState(false);
  const [editWorkspaceId, setEditWorkspaceId] = useState("");
  const [editEnvVars, setEditEnvVars] = useState<EnvEntry[]>([]);
  const [editSaving, setEditSaving] = useState(false);
  const [editError, setEditError] = useState("");

  // -- workspace metadata --
  const [gitBranches, setGitBranches] = useState<Record<string, string>>({});
  const [notificationItems, setNotificationItems] = useState<NotificationItem[]>([]);
  const [notificationPanelOpen, setNotificationPanelOpen] = useState(false);
  const [toastItems, setToastItems] = useState<NotificationItem[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [theme, setTheme] = useState<"dark" | "light">(() => {
    if (typeof window === "undefined") return "dark";
    const stored = window.localStorage.getItem("maxc-theme");
    return stored === "light" ? "light" : "dark";
  });
  const [appVersion, setAppVersion] = useState("");
  const [updateChannel, setUpdateChannel] = useState<"stable" | "beta">(() => {
    if (typeof window === "undefined") return "stable";
    const stored = window.localStorage.getItem("maxc-update-channel");
    return stored === "beta" ? "beta" : "stable";
  });
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "available" | "uptodate" | "downloading" | "ready" | "error"
  >("idle");
  const [updateError, setUpdateError] = useState("");
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [updateProgress, setUpdateProgress] = useState<{ downloaded: number; total?: number }>({
    downloaded: 0,
    total: undefined,
  });
  const [envCopied, setEnvCopied] = useState(false);

  // -- shortcut help --
  const [showShortcuts, setShowShortcuts] = useState(false);
  const [renamingWorkspaceId, setRenamingWorkspaceId] = useState("");
  const [renameValue, setRenameValue] = useState("");

  // -- refs --
  const surfacesRef = useRef<SurfaceState[]>([]);
  const pollInFlightRef = useRef(false);
  const spawnInFlightRef = useRef(false);
  const xtermHandles = useRef<Map<string, XtermHandle>>(new Map());
  const pendingInitialOutput = useRef<Map<string, string>>(new Map());
  const browserStatesRef = useRef<Map<string, BrowserState>>(new Map());
  const browserCaptureRef = useRef<Map<string, number>>(new Map());
  const notificationInitialized = useRef(false);
  const notificationSeen = useRef<Set<string>>(new Set());
  const inputBufferRef = useRef<Map<string, string>>(new Map());
  const inputTimerRef = useRef<Map<string, number>>(new Map());
  const inputInFlightRef = useRef<Set<string>>(new Set());

  const selectedWorkspace = useMemo(
    () => workspaces.find((w) => w.workspace_id === selectedWorkspaceId) ?? workspaces[0] ?? null,
    [workspaces, selectedWorkspaceId],
  );

  useEffect(() => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    if (theme === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
    window.localStorage.setItem("maxc-theme", theme);
  }, [theme]);

  useEffect(() => {
    getVersion()
      .then((v) => setAppVersion(v))
      .catch(() => setAppVersion(""));
  }, []);

  useEffect(() => {
    window.localStorage.setItem("maxc-update-channel", updateChannel);
  }, [updateChannel]);

  const readyForActions = Boolean(token) && (readiness?.ready ?? false) && selectedWorkspace !== null;

  const unreadNotifications = useMemo(
    () => notificationItems.filter((n) => !n.read),
    [notificationItems],
  );
  const unreadByWorkspace = useMemo(() => {
    const map = new Map<string, NotificationItem>();
    for (const n of unreadNotifications) {
      if (!n.workspace_id) continue;
      if (!map.has(n.workspace_id)) {
        map.set(n.workspace_id, n);
      }
    }
    return map;
  }, [unreadNotifications]);
  const unreadCount = unreadNotifications.length;

  const focusedSurface = useMemo(() => {
    if (!focusedPaneId) return null;
    return (
      surfaces.find((s) => s.paneId === focusedPaneId && s.focused) ??
      surfaces.find((s) => s.paneId === focusedPaneId) ??
      null
    );
  }, [focusedPaneId, surfaces]);

  const socketPath = useMemo(() => "\\\\.\\pipe\\maxc-rpc", []);

  function normalizeUrl(raw: string) {
    const trimmed = raw.trim();
    if (!trimmed) return "";
    if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(trimmed)) return trimmed;
    return `https://${trimmed}`;
  }

  // -- workspace metadata enrichment --
  const workspaceMetas: WorkspaceMeta[] = useMemo(() => {
    return workspaces.map((ws) => {
      const wsSurfaces = surfaces.filter((s) => s.workspaceId === ws.workspace_id);
      const termCount = wsSurfaces.filter((s) => s.panelType === "terminal").length;
      const hasNotif = wsSurfaces.some((s) => s.hasNewOutput);
      const notification = unreadByWorkspace.get(ws.workspace_id);
      return {
        ...ws,
        gitBranch: gitBranches[ws.workspace_id] || "",
        terminalCount: termCount,
        hasNotification: hasNotif || Boolean(notification),
        notificationText: notification?.title || (hasNotif ? "New output" : ""),
      };
    });
  }, [workspaces, surfaces, gitBranches, unreadByWorkspace]);

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
  // Notifications (backend-driven)
  // ---------------------------------------------------------------------------

  const pushToast = useCallback((note: NotificationItem) => {
    setToastItems((prev) => [...prev, note]);
    window.setTimeout(() => {
      setToastItems((prev) => prev.filter((n) => n.notification_id !== note.notification_id));
    }, 4000);
  }, []);

  const maybeSendOsNotification = useCallback(async (note: NotificationItem) => {
    try {
      let granted = await isPermissionGranted();
      if (!granted) {
        const status = await requestPermission();
        granted = status === "granted";
      }
      if (!granted) return;
      sendNotification({
        title: note.title,
        body: note.body,
      });
    } catch {
      // best effort
    }
  }, []);

  const loadNotifications = useCallback(async () => {
    if (!token) return;
    try {
      const result = await rpc<{ notifications: NotificationItem[] }>("notification.list", {
        auth: { token },
        limit: 200,
      });
      const items = Array.isArray(result.notifications) ? result.notifications : [];
      setNotificationItems(items);

      if (!notificationInitialized.current) {
        items.forEach((n) => notificationSeen.current.add(n.notification_id));
        notificationInitialized.current = true;
        return;
      }

      const newOnes = items.filter((n) => !notificationSeen.current.has(n.notification_id));
      if (newOnes.length > 0) {
        for (const note of newOnes) {
          pushToast(note);
          void maybeSendOsNotification(note);
          notificationSeen.current.add(note.notification_id);
        }
      }
    } catch (err) {
      console.error("notification.list failed", err);
    }
  }, [token, pushToast, maybeSendOsNotification]);

  // ---------------------------------------------------------------------------
  // Pane tree builder
  // ---------------------------------------------------------------------------
  const countPanes = useCallback((node: PaneNode | null): number => {
    if (!node) return 0;
    if (node.type === "leaf") return 1;
    return countPanes(node.children[0]) + countPanes(node.children[1]);
  }, []);

  const loadPaneTree = useCallback(async (tok: string, wsId: string, allowCreate = true) => {
    try {
      const result = await rpc<{ workspace_id: string; layout: any }>("workspace.layout", {
        workspace_id: wsId,
        auth: { token: tok },
      });

      const layout = result.layout;
      if (!layout || !layout.pane_id) {
        if (allowCreate) {
          try {
            await rpc("pane.create", {
              command_id: "ui-pane-create-" + crypto.randomUUID(),
              workspace_id: wsId,
              auth: { token: tok },
            });
          } catch (err) {
            console.error("pane.create failed", err);
          }
          await loadPaneTree(tok, wsId, false);
        } else {
          setPaneTree(null);
          surfacesRef.current = [];
          setSurfaces([]);
        }
        return;
      }

      // Collect all surfaces from the layout tree and build SurfaceState list
      const collectedSurfaces: SurfaceState[] = [];
      function collectSurfaces(node: any) {
        if (node.type === "leaf" && Array.isArray(node.surfaces)) {
          for (const s of node.surfaces) {
            collectedSurfaces.push({
              surfaceId: s.surface_id,
              paneId: s.pane_id,
              workspaceId: s.workspace_id,
              title: s.title,
              panelType: s.panel_type,
              terminalSessionId: s.panel_type === "terminal" ? s.panel_session_id : null,
              browserSessionId: s.panel_type === "browser" ? s.panel_session_id : null,
              order: s.order,
              focused: s.focused,
              lastSequence: 0,
              hasNewOutput: false,
            });
          }
        } else if (node.type === "split" && Array.isArray(node.children)) {
          for (const child of node.children) collectSurfaces(child);
        }
      }
      collectSurfaces(layout);

      // Preserve lastSequence from existing surfaces
      const existing = new Map(surfacesRef.current.map((s) => [s.surfaceId, s]));
      for (const s of collectedSurfaces) {
        const prev = existing.get(s.surfaceId);
        if (prev) {
          s.lastSequence = prev.lastSequence;
          s.hasNewOutput = prev.hasNewOutput;
          if (!s.terminalSessionId && prev.terminalSessionId) {
            s.terminalSessionId = prev.terminalSessionId;
          }
          if (!s.browserSessionId && prev.browserSessionId) {
            s.browserSessionId = prev.browserSessionId;
          }
        }
      }

      surfacesRef.current = collectedSurfaces;
      setSurfaces(collectedSurfaces);

      // Convert layout JSON to PaneNode
      function toNode(raw: any): PaneNode {
        if (raw.type === "split" && Array.isArray(raw.children) && raw.children.length >= 2) {
          return {
            type: "split",
            paneId: raw.pane_id,
            direction: raw.direction as "horizontal" | "vertical",
            children: [toNode(raw.children[0]), toNode(raw.children[1])],
          };
        }
        const paneSurfs = collectedSurfaces.filter((s) => s.paneId === raw.pane_id);
        return {
          type: "leaf",
          paneId: raw.pane_id,
          surfaces: paneSurfs,
          activeSurfaceId: raw.active_surface_id ?? paneSurfs[0]?.surfaceId ?? null,
        };
      }

      const tree = toNode(layout);
      setPaneTree(tree);
      if (tree) setFocusedPaneId(tree.paneId);
    } catch (err) {
      console.error("loadPaneTree error:", err);
    }
  }, []);

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
          // Load pane tree for the first workspace
          await loadPaneTree(session.token, wsList.workspaces[0].workspace_id);
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

  // poll notifications
  useEffect(() => {
    if (!token) return;
    loadNotifications();
    const id = setInterval(loadNotifications, 3_000);
    return () => clearInterval(id);
  }, [token, loadNotifications]);

  // ---------------------------------------------------------------------------
  // Keyboard shortcuts
  // ---------------------------------------------------------------------------
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;
      const targetSurface = focusedSurface?.surfaceId;

      // Ctrl+Shift+N -> new window
      if (ctrl && e.shiftKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        invoke("create_window").catch(console.error);
        return;
      }

      // Ctrl + / Ctrl = -> increase font size
      if (ctrl && (e.key === "+" || e.key === "=")) {
        e.preventDefault();
        if (targetSurface) xtermHandles.current.get(targetSurface)?.changeFontSize(1);
        return;
      }

      // Ctrl - -> decrease font size
      if (ctrl && e.key === "-") {
        e.preventDefault();
        if (targetSurface) xtermHandles.current.get(targetSurface)?.changeFontSize(-1);
        return;
      }

      // Ctrl+N -> new workspace drawer
      if (ctrl && e.key === "n") { e.preventDefault(); openDrawer(); return; }

      // Ctrl+T -> new terminal surface in focused pane
      if (ctrl && e.key === "t") { e.preventDefault(); handleCreateSurface(focusedPaneId, "terminal"); return; }

      // Ctrl+B -> new browser surface in focused pane
      if (ctrl && e.key === "b") { e.preventDefault(); handleCreateSurface(focusedPaneId, "browser"); return; }

      // Ctrl+D -> split pane right
      if (ctrl && !e.shiftKey && e.key === "d") { e.preventDefault(); handleSplitPane(focusedPaneId, "vertical"); return; }

      // Ctrl+Shift+D -> split pane down
      if (ctrl && e.shiftKey && e.key === "D") { e.preventDefault(); handleSplitPane(focusedPaneId, "horizontal"); return; }

      // Ctrl+? -> toggle shortcuts panel
      if (ctrl && e.key === "/") { e.preventDefault(); setShowShortcuts((v) => !v); return; }

      // Alt+1-9 -> switch workspace
      if (e.altKey && e.key >= "1" && e.key <= "9") {
        e.preventDefault();
        const idx = parseInt(e.key) - 1;
        if (idx < workspaces.length) {
          selectWorkspace(workspaces[idx].workspace_id);
        }
        return;
      }

      // Escape -> close drawer/shortcuts
      if (e.key === "Escape") {
        if (drawerOpen) closeDrawer();
        if (editDrawerOpen) closeEditDrawer();
        if (showShortcuts) setShowShortcuts(false);
        if (notificationPanelOpen) setNotificationPanelOpen(false);
        if (settingsOpen) setSettingsOpen(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [focusedSurface?.surfaceId, focusedPaneId, workspaces, selectedWorkspaceId, drawerOpen, editDrawerOpen, showShortcuts, notificationPanelOpen, settingsOpen]);

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

  function addEditEnvEntry() { setEditEnvVars((p) => [...p, { key: "", value: "" }]); }
  function updateEditEnvEntry(i: number, field: "key" | "value", val: string) {
    setEditEnvVars((p) => p.map((e, idx) => (idx === i ? { ...e, [field]: val } : e)));
  }
  function removeEditEnvEntry(i: number) {
    setEditEnvVars((p) => p.filter((_, idx) => idx !== i));
  }

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

  function openEditDrawer(ws: Workspace) {
    setEditWorkspaceId(ws.workspace_id);
    const editable = Object.entries(ws.env_vars || {})
      .filter(([key]) => !key.startsWith("MAXC_"))
      .map(([key, value]) => ({ key, value }));
    setEditEnvVars(editable);
    setEditError("");
    setEditDrawerOpen(true);
  }

  function closeEditDrawer() {
    setEditDrawerOpen(false);
    setEditSaving(false);
    setEditError("");
  }

  async function saveWorkspaceEnv() {
    if (!token || !editWorkspaceId) return;
    const ws = workspaces.find((w) => w.workspace_id === editWorkspaceId);
    if (!ws) return;

    const envObj: Record<string, string> = {};
    for (const e of editEnvVars) {
      const k = e.key.trim();
      if (k) envObj[k] = e.value;
    }
    // Preserve MAXC_* and other system-provided values
    for (const [key, value] of Object.entries(ws.env_vars || {})) {
      if (key.startsWith("MAXC_") && !(key in envObj)) {
        envObj[key] = value;
      }
    }

    setEditSaving(true);
    setEditError("");
    try {
      await rpc("workspace.update", {
        command_id: "ui-ws-env-update-" + crypto.randomUUID(),
        workspace_id: ws.workspace_id,
        env_vars: envObj,
        auth: { token },
      });
      setWorkspaces((prev) =>
        prev.map((w) =>
          w.workspace_id === ws.workspace_id ? { ...w, env_vars: envObj } : w,
        ),
      );
      closeEditDrawer();
    } catch (err) {
      setEditError((err as Error).message);
    } finally {
      setEditSaving(false);
    }
  }

  // ---------------------------------------------------------------------------
  // Browser helpers (used by pane actions and views)
  // ---------------------------------------------------------------------------
  function getBrowserAudit(surfaceId: string) {
    const s = surfacesRef.current.find((sf) => sf.surfaceId === surfaceId);
    if (!s?.browserSessionId) return null;
    const bs = browserStatesRef.current.get(surfaceId);
    return {
      workspace_id: s.workspaceId,
      surface_id: surfaceId,
      browser_session_id: s.browserSessionId,
      tab_id: bs?.tabId ?? "",
    };
  }

  const requestBrowserScreenshot = useCallback(
    async (
      surfaceId: string,
      auditOverride?: {
        workspace_id: string;
        surface_id: string;
        browser_session_id: string;
        tab_id: string;
      },
    ) => {
      if (!token) return;
      const audit = auditOverride ?? getBrowserAudit(surfaceId);
      if (!audit || !audit.tab_id) return;
      setBrowserStates((prev) => {
        const next = new Map(prev);
        const bs = next.get(surfaceId) ?? {
          currentUrl: "",
          screenshotData: null,
          screenshotLoading: false,
          sessionLoading: false,
          tabId: audit.tab_id,
        };
        next.set(surfaceId, { ...bs, screenshotLoading: true, tabId: audit.tab_id });
        return next;
      });
      try {
        const result = await rpc<any>("browser.screenshot", {
          command_id: "ui-browser-screenshot-" + crypto.randomUUID(),
          ...audit,
          auth: { token },
        });
        const artifactPath = result.artifact_path ?? result.details?.artifact_path;
        const imageSrc = artifactPath ? convertFileSrc(artifactPath) : result.data ?? null;
        setBrowserStates((prev) => {
          const next = new Map(prev);
          const bs = next.get(surfaceId) ?? {
            currentUrl: "",
            screenshotData: null,
            screenshotLoading: false,
            tabId: audit.tab_id,
            sessionLoading: false,
          };
          next.set(surfaceId, {
            ...bs,
            screenshotData: imageSrc,
            screenshotLoading: false,
            sessionLoading: false,
            tabId: audit.tab_id,
          });
          return next;
        });
      } catch (err) {
        console.error("browser.screenshot error:", err);
        setBrowserStates((prev) => {
          const next = new Map(prev);
          const bs = next.get(surfaceId);
          if (bs) next.set(surfaceId, { ...bs, screenshotLoading: false });
          return next;
        });
      }
    },
    [token],
  );

  // ---------------------------------------------------------------------------
  // Pane / Surface actions
  // ---------------------------------------------------------------------------
  async function handleCreateSurface(paneId: string, panelType: string) {
    if (!readyForActions || !selectedWorkspace || !paneId) {
      setBackendStatus("Create a workspace first");
      return;
    }
    if (spawnInFlightRef.current) { setBackendStatus("Spawn in progress..."); return; }
    spawnInFlightRef.current = true;
    try {
      setBackendStatus("Creating surface...");

      // Count existing terminal surfaces in workspace for title numbering
      const termCount = surfacesRef.current.filter(
        (s) => s.workspaceId === selectedWorkspace.workspace_id && s.panelType === "terminal",
      ).length;
      const title =
        panelType === "terminal"
          ? `Terminal ${termCount + 1}`
          : panelType === "agent"
            ? "Agent"
            : "Browser";

      // Create surface
      const surfResult = await rpc<any>("surface.create", {
        command_id: "ui-surface-create-" + crypto.randomUUID(),
        pane_id: paneId,
        workspace_id: selectedWorkspace.workspace_id,
        panel_type: panelType,
        title,
        auth: { token },
      });
      const surfaceId = surfResult.surface_id as string;

      if (panelType === "browser") {
        const newSurface: SurfaceState = {
          surfaceId,
          paneId,
          workspaceId: selectedWorkspace.workspace_id,
          title,
          panelType,
          terminalSessionId: null,
          browserSessionId: null,
          order: surfResult.order ?? 0,
          focused: true,
          lastSequence: 0,
          hasNewOutput: false,
        };
        const next = [...surfacesRef.current, newSurface];
        surfacesRef.current = next;
        setSurfaces(next);
        await loadPaneTree(token, selectedWorkspace.workspace_id);
        setBackendStatus("Starting browser session...");
        void attachBrowser(surfaceId);
        return;
      }

      let termSessionId: string | null = null;
      let browserSessionId: string | null = null;
      let initialLastSeq = 0;

      if (panelType === "terminal") {
        // Spawn terminal in workspace folder
        const spawnParams: Record<string, unknown> = {
          command_id: "ui-term-spawn-" + crypto.randomUUID(),
          workspace_id: selectedWorkspace.workspace_id,
          surface_id: surfaceId,
          cols: 120, rows: 34,
          auth: { token },
        };
        if (selectedWorkspace.folder) {
          spawnParams.cwd = selectedWorkspace.folder;
        }
        const termResult = await rpc<any>("terminal.spawn", spawnParams);
        termSessionId = termResult.terminal_session_id as string;

        // Fetch initial output
        if (termSessionId) {
          try {
            const history = await rpc<any>("terminal.history", {
              workspace_id: selectedWorkspace.workspace_id,
              surface_id: surfaceId,
              terminal_session_id: termSessionId,
              from_sequence: 0,
              auth: { token },
            });
            let initialOutput = "";
            if (Array.isArray(history.events)) {
              for (const ev of history.events) {
                if (ev.type === "terminal.output" && ev.output) initialOutput += ev.output as string;
                initialLastSeq = Math.max(initialLastSeq, ev.sequence ?? initialLastSeq);
              }
            }
            if (initialOutput) {
              pendingInitialOutput.current.set(surfaceId, initialOutput);
            }
          } catch { /* non-fatal */ }
        }
      }

      // Add surface to state
      const newSurface: SurfaceState = {
        surfaceId,
        paneId,
        workspaceId: selectedWorkspace.workspace_id,
        title,
        panelType,
        terminalSessionId: termSessionId,
        browserSessionId,
        order: surfResult.order ?? 0,
        focused: true,
        lastSequence: initialLastSeq,
        hasNewOutput: false,
      };
      const next = [...surfacesRef.current, newSurface];
      surfacesRef.current = next;
      setSurfaces(next);

      // Rebuild pane tree
      await loadPaneTree(token, selectedWorkspace.workspace_id);
      setBackendStatus(panelType === "terminal" ? "Terminal spawned" : "Surface created");
    } catch (error) {
      console.error(error);
      setBackendStatus((error as Error).message);
    } finally { spawnInFlightRef.current = false; }
  }

  async function attachTerminal(surfaceId: string) {
    if (!readyForActions || !selectedWorkspace) return;
    const surface = surfacesRef.current.find((s) => s.surfaceId === surfaceId);
    if (!surface) return;
    try {
      const attachParams: Record<string, unknown> = {
        command_id: "ui-term-attach-" + crypto.randomUUID(),
        workspace_id: surface.workspaceId,
        surface_id: surfaceId,
        cols: 120,
        rows: 34,
        auth: { token },
      };
      if (selectedWorkspace.folder) {
        attachParams.cwd = selectedWorkspace.folder;
      }
      const termResult = await rpc<any>("terminal.spawn", attachParams);
      const termSessionId = termResult.terminal_session_id as string;
      const next = surfacesRef.current.map((s) =>
        s.surfaceId === surfaceId ? { ...s, terminalSessionId: termSessionId } : s,
      );
      surfacesRef.current = next;
      setSurfaces(next);
    } catch (err) {
      setBackendStatus((err as Error).message);
    }
  }

  async function attachBrowser(surfaceId: string) {
    if (!readyForActions || !selectedWorkspace) return;
    const surface = surfacesRef.current.find((s) => s.surfaceId === surfaceId);
    if (!surface) return;
    try {
      setBrowserStates((prev) => {
        const next = new Map(prev);
        const existing = next.get(surfaceId) ?? {
          currentUrl: "",
          screenshotData: null,
          screenshotLoading: false,
          sessionLoading: false,
          tabId: null,
        };
        next.set(surfaceId, { ...existing, sessionLoading: true });
        return next;
      });

      const browserResult = await rpc<any>("browser.create", {
        command_id: "ui-browser-attach-" + crypto.randomUUID(),
        workspace_id: surface.workspaceId,
        surface_id: surfaceId,
        auth: { token },
      });
      const browserSessionId = browserResult.browser_session_id as string;

      const next = surfacesRef.current.map((s) =>
        s.surfaceId === surfaceId ? { ...s, browserSessionId } : s,
      );
      surfacesRef.current = next;
      setSurfaces(next);

      const tabResult = await rpc<any>("browser.tab.open", {
        command_id: "ui-browser-tab-" + crypto.randomUUID(),
        workspace_id: surface.workspaceId,
        surface_id: surfaceId,
        browser_session_id: browserSessionId,
        url: "https://example.com",
        auth: { token },
      });
      const tabId = (tabResult.browser_tab_id ?? tabResult.tab_id ?? "") as string;
      setBrowserStates((prev) => {
        const next = new Map(prev);
        next.set(surfaceId, {
          currentUrl: "https://example.com",
          screenshotData: null,
          screenshotLoading: false,
          sessionLoading: false,
          tabId,
        });
        return next;
      });
      if (tabId) {
        setTimeout(() => {
          requestBrowserScreenshot(surfaceId, {
            workspace_id: surface.workspaceId,
            surface_id: surfaceId,
            browser_session_id: browserSessionId,
            tab_id: tabId,
          });
        }, 600);
      }
    } catch (err) {
      setBrowserStates((prev) => {
        const next = new Map(prev);
        const existing = next.get(surfaceId);
        if (existing) next.set(surfaceId, { ...existing, sessionLoading: false });
        return next;
      });
      setBackendStatus((err as Error).message);
    }
  }

  async function handleSplitPane(paneId: string, direction: "horizontal" | "vertical") {
    if (!readyForActions || !selectedWorkspace || !paneId) return;
    try {
      await rpc("pane.split", {
        command_id: "ui-pane-split-" + crypto.randomUUID(),
        pane_id: paneId,
        direction,
        ratio: 0.5,
        auth: { token },
      });
      await loadPaneTree(token, selectedWorkspace.workspace_id);
    } catch (error) {
      setBackendStatus((error as Error).message);
    }
  }

  async function handleClosePane(paneId: string) {
    if (!readyForActions || !selectedWorkspace) return;
    if (countPanes(paneTree) <= 1) {
      setBackendStatus("Pane 1 cannot be closed");
      return;
    }
    try {
      await rpc("pane.close", {
        command_id: "ui-pane-close-" + crypto.randomUUID(),
        pane_id: paneId,
        auth: { token },
      });
      await loadPaneTree(token, selectedWorkspace.workspace_id);
    } catch (error) {
      setBackendStatus((error as Error).message);
    }
  }

  async function handleCloseSurface(surfaceId: string) {
    if (!token) return;
    try {
      const target = surfacesRef.current.find((s) => s.surfaceId === surfaceId);
      if (target?.browserSessionId) {
        await rpc("browser.close", {
          command_id: "ui-browser-close-" + crypto.randomUUID(),
          workspace_id: target.workspaceId,
          surface_id: target.surfaceId,
          browser_session_id: target.browserSessionId,
          auth: { token },
        });
      }
      await rpc("surface.close", {
        command_id: "ui-surface-close-" + crypto.randomUUID(),
        surface_id: surfaceId,
        auth: { token },
      });
      // Remove from local state
      const next = surfacesRef.current.filter((s) => s.surfaceId !== surfaceId);
      surfacesRef.current = next;
      setSurfaces(next);
      xtermHandles.current.delete(surfaceId);
      if (selectedWorkspace) await loadPaneTree(token, selectedWorkspace.workspace_id);
    } catch (error) {
      setBackendStatus((error as Error).message);
    }
  }

  async function handleFocusSurface(surfaceId: string) {
    if (!token) return;
    try {
      await rpc("surface.focus", {
        command_id: "ui-surface-focus-" + crypto.randomUUID(),
        surface_id: surfaceId,
        auth: { token },
      });
      // Update local state
      const target = surfacesRef.current.find((s) => s.surfaceId === surfaceId);
      if (target) {
        const next = surfacesRef.current.map((s) => ({
          ...s,
          focused: s.paneId === target.paneId ? s.surfaceId === surfaceId : s.focused,
          hasNewOutput: s.surfaceId === surfaceId ? false : s.hasNewOutput,
        }));
        surfacesRef.current = next;
        setSurfaces(next);
      }
      // Update pane tree to reflect focus
      setPaneTree((prev) => {
        if (!prev) return prev;
        return updateTreeFocus(prev, surfaceId);
      });
    } catch (error) {
      setBackendStatus((error as Error).message);
    }
  }

  function updateTreeFocus(node: PaneNode, surfaceId: string): PaneNode {
    if (node.type === "split") {
      return {
        ...node,
        children: [
          updateTreeFocus(node.children[0], surfaceId),
          updateTreeFocus(node.children[1], surfaceId),
        ],
      };
    }
    const hasSurface = node.surfaces.some((s) => s.surfaceId === surfaceId);
    if (!hasSurface) return node;
    return { ...node, activeSurfaceId: surfaceId };
  }

  // -- terminal input / resize handlers (keyed by surfaceId) --
  const flushTerminalInput = useCallback(
    async (surfaceId: string) => {
      const buffer = inputBufferRef.current.get(surfaceId);
      if (!buffer) {
        inputBufferRef.current.delete(surfaceId);
        return;
      }
      if (inputInFlightRef.current.has(surfaceId)) {
        const retry = window.setTimeout(() => flushTerminalInput(surfaceId), 12);
        inputTimerRef.current.set(surfaceId, retry);
        return;
      }
      const surface = surfacesRef.current.find((s) => s.surfaceId === surfaceId);
      if (!surface?.terminalSessionId || !token) {
        inputBufferRef.current.delete(surfaceId);
        return;
      }
      inputInFlightRef.current.add(surfaceId);
      inputBufferRef.current.set(surfaceId, "");
      try {
        await rpc("terminal.input", {
          command_id: "ui-term-input-" + crypto.randomUUID(),
          workspace_id: surface.workspaceId,
          surface_id: surfaceId,
          terminal_session_id: surface.terminalSessionId,
          input: buffer,
          auth: { token },
        });
      } catch (err) {
        console.error("terminal.input error:", err);
      } finally {
        inputInFlightRef.current.delete(surfaceId);
        const remaining = inputBufferRef.current.get(surfaceId);
        if (remaining) {
          const retry = window.setTimeout(() => flushTerminalInput(surfaceId), 0);
          inputTimerRef.current.set(surfaceId, retry);
        }
      }
    },
    [token],
  );

  const sendTerminalInput = useCallback(
    (surfaceId: string, data: string) => {
      if (!token) return;
      const prev = inputBufferRef.current.get(surfaceId) ?? "";
      inputBufferRef.current.set(surfaceId, prev + data);
      if (!inputTimerRef.current.has(surfaceId)) {
        const timer = window.setTimeout(() => {
          inputTimerRef.current.delete(surfaceId);
          void flushTerminalInput(surfaceId);
        }, 12);
        inputTimerRef.current.set(surfaceId, timer);
      }
    },
    [flushTerminalInput, token],
  );

  const sendTerminalResize = useCallback(
    async (surfaceId: string, cols: number, rows: number) => {
      if (!token) return;
      const surface = surfacesRef.current.find((s) => s.surfaceId === surfaceId);
      if (!surface?.terminalSessionId) return;
      try {
        await rpc("terminal.resize", {
          command_id: "ui-term-resize-" + crypto.randomUUID(),
          workspace_id: surface.workspaceId,
          surface_id: surfaceId,
          terminal_session_id: surface.terminalSessionId,
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
  const registerXtermHandle = useCallback((surfaceId: string, handle: XtermHandle | null) => {
    if (handle) {
      xtermHandles.current.set(surfaceId, handle);
      const pending = pendingInitialOutput.current.get(surfaceId);
      if (pending) {
        handle.write(pending);
        pendingInitialOutput.current.delete(surfaceId);
      }
    } else {
      xtermHandles.current.delete(surfaceId);
    }
  }, []);

  // ---------------------------------------------------------------------------
  // Browser RPC handlers
  // ---------------------------------------------------------------------------

  const handleBrowserNavigate = useCallback(async (surfaceId: string, url: string) => {
    if (!token) return;
    const normalized = normalizeUrl(url);
    if (!normalized) return;

    setBrowserStates((prev) => {
      const next = new Map(prev);
      const bs = next.get(surfaceId) ?? {
        currentUrl: "",
        screenshotData: null,
        screenshotLoading: false,
        sessionLoading: false,
        tabId: null,
      };
      next.set(surfaceId, { ...bs, currentUrl: normalized });
      return next;
    });

    const audit = getBrowserAudit(surfaceId);
    if (!audit) return;
    if (!audit.tab_id) {
      try {
        const tab = await rpc<any>("browser.tab.open", {
          command_id: "ui-browser-tab-open-" + crypto.randomUUID(),
          workspace_id: audit.workspace_id,
          surface_id: surfaceId,
          browser_session_id: audit.browser_session_id,
          url,
          auth: { token },
        });
        const tabId = (tab.browser_tab_id ?? tab.tab_id ?? "") as string;
        setBrowserStates((prev) => {
          const next = new Map(prev);
          const bs = next.get(surfaceId) ?? {
            currentUrl: normalized,
            screenshotData: null,
            screenshotLoading: false,
            sessionLoading: false,
            tabId: null,
          };
          next.set(surfaceId, {
            ...bs,
            currentUrl: normalized,
            screenshotData: null,
            screenshotLoading: false,
            tabId,
          });
          return next;
        });
        if (tabId) {
          setTimeout(() => {
            requestBrowserScreenshot(surfaceId, {
              workspace_id: audit.workspace_id,
              surface_id: surfaceId,
              browser_session_id: audit.browser_session_id,
              tab_id: tabId,
            });
          }, 500);
        }
      } catch (err) {
        console.error("browser.tab.open error:", err);
      }
      return;
    }
    try {
      await rpc("browser.goto", {
        command_id: "ui-browser-goto-" + crypto.randomUUID(),
        ...audit,
        url: normalized,
        auth: { token },
      });
      setBrowserStates((prev) => {
        const next = new Map(prev);
        const bs = next.get(surfaceId) ?? {
          currentUrl: "",
          screenshotData: null,
          screenshotLoading: false,
          sessionLoading: false,
          tabId: audit.tab_id,
        };
        next.set(surfaceId, { ...bs, currentUrl: normalized });
        return next;
      });
      setTimeout(() => requestBrowserScreenshot(surfaceId), 500);
    } catch (err) { console.error("browser.goto error:", err); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, requestBrowserScreenshot]);

  const handleBrowserReload = useCallback(async (surfaceId: string) => {
    if (!token) return;
    const audit = getBrowserAudit(surfaceId);
    if (!audit || !audit.tab_id) return;
    try {
      await rpc("browser.reload", {
        command_id: "ui-browser-reload-" + crypto.randomUUID(),
        ...audit,
        auth: { token },
      });
    } catch (err) { console.error("browser.reload error:", err); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, browserStates]);

  const handleBrowserBack = useCallback(async (surfaceId: string) => {
    if (!token) return;
    const audit = getBrowserAudit(surfaceId);
    if (!audit || !audit.tab_id) return;
    try {
      await rpc("browser.back", {
        command_id: "ui-browser-back-" + crypto.randomUUID(),
        ...audit,
        auth: { token },
      });
    } catch (err) { console.error("browser.back error:", err); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, browserStates]);

  const handleBrowserForward = useCallback(async (surfaceId: string) => {
    if (!token) return;
    const audit = getBrowserAudit(surfaceId);
    if (!audit || !audit.tab_id) return;
    try {
      await rpc("browser.forward", {
        command_id: "ui-browser-forward-" + crypto.randomUUID(),
        ...audit,
        auth: { token },
      });
    } catch (err) { console.error("browser.forward error:", err); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [token, browserStates]);

  const handleBrowserScreenshot = useCallback(
    async (surfaceId: string) => {
      await requestBrowserScreenshot(surfaceId);
    },
    [requestBrowserScreenshot],
  );

  // auto-refresh browser snapshots for focused browser surfaces
  useEffect(() => {
    if (!token) return;
    const id = setInterval(() => {
      const now = Date.now();
      const focusedBrowsers = surfacesRef.current.filter(
        (s) => s.panelType === "browser" && s.browserSessionId && s.focused,
      );
      for (const surface of focusedBrowsers) {
        const bs = browserStatesRef.current.get(surface.surfaceId);
        if (bs?.screenshotLoading) continue;
        const last = browserCaptureRef.current.get(surface.surfaceId) ?? 0;
        if (now - last < 1500) continue;
        browserCaptureRef.current.set(surface.surfaceId, now);
        requestBrowserScreenshot(surface.surfaceId);
      }
    }, 1500);
    return () => clearInterval(id);
  }, [token, requestBrowserScreenshot]);

  // ---------------------------------------------------------------------------
  // Terminal polling (polls all surfaces with terminal sessions)
  // ---------------------------------------------------------------------------
  useEffect(() => { surfacesRef.current = surfaces; }, [surfaces]);
  useEffect(() => { browserStatesRef.current = browserStates; }, [browserStates]);

  useEffect(() => {
    if (!token) return;
    const poll = async () => {
      if (pollInFlightRef.current) return;
      const list = surfacesRef.current.filter((s) => s.terminalSessionId);
      if (!list.length) return;
      pollInFlightRef.current = true;
      try {
        let didChange = false;
        const updated = await Promise.all(
          list.map(async (s) => {
            const result = await rpc<any>("terminal.history", {
              workspace_id: s.workspaceId,
              surface_id: s.surfaceId,
              terminal_session_id: s.terminalSessionId,
              from_sequence: s.lastSequence + 1,
              max_events: 64,
              auth: { token },
            });
            let lastSeq = s.lastSequence;
            let newOutput = false;
            if (Array.isArray(result.events)) {
              const handle = xtermHandles.current.get(s.surfaceId);
              for (const ev of result.events) {
                if (ev.type === "terminal.output" && ev.output) {
                  handle?.write(ev.output as string);
                  newOutput = true;
                }
                lastSeq = Math.max(lastSeq, ev.sequence ?? lastSeq);
              }
            }
            const nextHasNew = newOutput || s.hasNewOutput;
            if (lastSeq !== s.lastSequence || nextHasNew !== s.hasNewOutput) {
              didChange = true;
            }
            return { ...s, lastSequence: lastSeq, hasNewOutput: nextHasNew };
          }),
        );

        // Merge updates back into full surfaces list
        if (didChange) {
          const updatedMap = new Map(updated.map((u) => [u.surfaceId, u]));
          const merged = surfacesRef.current.map((s) => updatedMap.get(s.surfaceId) ?? s);
          surfacesRef.current = merged;
          setSurfaces(merged);
        }
      } catch (err) {
        console.error(err);
      } finally {
        pollInFlightRef.current = false;
      }
    };
    const id = setInterval(poll, 500);
    return () => clearInterval(id);
  }, [token]);

  // clear notification when selecting a workspace
  function selectWorkspace(wsId: string) {
    setSelectedWorkspaceId(wsId);
    if (token) loadPaneTree(token, wsId);
    void clearWorkspaceNotifications(wsId);
  }

  async function clearWorkspaceNotifications(workspaceId: string) {
    if (!token) return;
    try {
      await rpc("notification.clear", {
        command_id: "ui-notify-clear-ws-" + crypto.randomUUID(),
        workspace_id: workspaceId,
        auth: { token },
      });
      setNotificationItems((prev) =>
        prev.map((n) =>
          n.workspace_id === workspaceId ? { ...n, read: true } : n,
        ),
      );
    } catch (err) {
      console.error("notification.clear workspace failed", err);
    }
  }

  async function clearAllNotifications() {
    if (!token) return;
    try {
      await rpc("notification.clear", {
        command_id: "ui-notify-clear-all-" + crypto.randomUUID(),
        auth: { token },
      });
      setNotificationItems((prev) => prev.map((n) => ({ ...n, read: true })));
    } catch (err) {
      console.error("notification.clear failed", err);
    }
  }

  async function clearNotification(notificationId: string) {
    if (!token) return;
    try {
      await rpc("notification.clear", {
        command_id: "ui-notify-clear-one-" + crypto.randomUUID(),
        notification_id: notificationId,
        auth: { token },
      });
      setNotificationItems((prev) =>
        prev.map((n) =>
          n.notification_id === notificationId ? { ...n, read: true } : n,
        ),
      );
    } catch (err) {
      console.error("notification.clear single failed", err);
    }
  }

  function startRename(wsId: string, currentName: string) {
    setRenamingWorkspaceId(wsId);
    setRenameValue(currentName);
  }

  async function commitRename(wsId: string) {
    if (!token || !renameValue.trim()) {
      setRenamingWorkspaceId("");
      return;
    }
    try {
      await rpc("workspace.update", {
        command_id: "ui-ws-rename-" + crypto.randomUUID(),
        workspace_id: wsId,
        name: renameValue.trim(),
        auth: { token },
      });
      setWorkspaces((prev) =>
        prev.map((ws) => (ws.workspace_id === wsId ? { ...ws, name: renameValue.trim() } : ws)),
      );
    } catch (err) {
      setBackendStatus((err as Error).message);
    }
    setRenamingWorkspaceId("");
  }

  async function deleteWorkspace(wsId: string) {
    if (!token) return;
    try {
      await rpc("workspace.delete", {
        command_id: "ui-ws-delete-" + crypto.randomUUID(),
        workspace_id: wsId,
        auth: { token },
      });
      setWorkspaces((prev) => prev.filter((ws) => ws.workspace_id !== wsId));
      if (selectedWorkspaceId === wsId) {
        const remaining = workspaces.filter((ws) => ws.workspace_id !== wsId);
        setSelectedWorkspaceId(remaining[0]?.workspace_id || "");
        setPaneTree(null);
        setSurfaces([]);
      }
    } catch (err) {
      setBackendStatus((err as Error).message);
    }
  }

  // ---------------------------------------------------------------------------
  // Workspace vertical tabs sidebar
  // ---------------------------------------------------------------------------

  function renderWorkspaceSidebar() {
    return (
      <aside className="h-[calc(100vh-40px)] w-52 border-r bg-sidebar/95 flex flex-col text-[12px] backdrop-blur">
        {/* header */}
        <div className="flex items-center justify-between px-3 pt-3 pb-2">
          <div className="text-[10px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">Workspaces</div>
          <div className="flex items-center gap-1">
            <Button
              size="icon-xs"
              variant="ghost"
              onClick={() => setTheme((prev) => (prev === "dark" ? "light" : "dark"))}
              title={theme === "dark" ? "Switch to light" : "Switch to dark"}
            >
              {theme === "dark" ? <Sun className="size-3.5" /> : <Moon className="size-3.5" />}
            </Button>
            <Button size="icon-xs" variant="ghost" onClick={openDrawer} title="New Workspace (Ctrl+N)">
              <Plus className="size-3.5" />
            </Button>
          </div>
        </div>

        {/* workspace tabs */}
        <div className="flex-1 overflow-auto px-2 space-y-1 pb-3">
          {workspaceMetas.length === 0 && (
            <div className="px-2 py-6">
              <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-border/60 bg-card/60 px-4 py-5 text-center">
                <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground">
                  <FolderOpen className="size-4" />
                </div>
                <div className="space-y-1">
                  <div className="text-[12px] font-medium text-foreground">No workspaces</div>
                  <div className="text-[10px] text-muted-foreground">Create one to start working.</div>
                </div>
                <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                  <kbd className="rounded border border-border bg-muted/40 px-1.5 py-0.5">Ctrl+N</kbd>
                  <span className="text-muted-foreground/60">New workspace</span>
                </div>
              </div>
            </div>
          )}
          {workspaceMetas.map((ws, idx) => {
            const active = selectedWorkspaceId === ws.workspace_id;
            const shortcutLabel = idx < 9 ? `Ctrl+${idx + 1}` : "";
            return (
              <button
                key={ws.workspace_id}
                onClick={() => {
                  selectWorkspace(ws.workspace_id);
                }}
                className={cn(
                  "group relative w-full rounded-md px-2 py-1.5 text-left transition-colors border",
                  active
                    ? "bg-muted/40 border-border"
                    : "border-transparent hover:bg-muted/60",
                )}
              >
                {/* workspace name + chevron */}
                <div className="flex items-center gap-1.5">
                  <span
                    className={cn(
                      "h-1.5 w-1.5 rounded-full",
                      ws.terminalCount > 0 ? "bg-emerald-400" : "bg-muted-foreground/50",
                    )}
                    aria-hidden
                  />
                  <ChevronRight className={cn("size-3 text-muted-foreground/70 transition-transform", active && "rotate-90 text-foreground")} />
                  {renamingWorkspaceId === ws.workspace_id ? (
                    <input
                      autoFocus
                      className="flex-1 bg-transparent border-b border-border text-foreground font-medium outline-none text-[12px] py-0"
                      value={renameValue}
                      onChange={(e) => setRenameValue(e.target.value)}
                      onBlur={() => commitRename(ws.workspace_id)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") commitRename(ws.workspace_id);
                        if (e.key === "Escape") setRenamingWorkspaceId("");
                      }}
                      onClick={(e) => e.stopPropagation()}
                    />
                  ) : (
                    <span
                      className={cn("font-medium truncate", active ? "text-foreground" : "text-muted-foreground")}
                      onDoubleClick={(e) => { e.stopPropagation(); startRename(ws.workspace_id, ws.name); }}
                    >
                      {ws.name}
                    </span>
                  )}
                  {renamingWorkspaceId !== ws.workspace_id && (
                    <span className="ml-auto flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
                      {shortcutLabel && (
                        <span className="text-[9px] text-muted-foreground/60">{shortcutLabel}</span>
                      )}
                      <button
                        onClick={(e) => { e.stopPropagation(); openEditDrawer(ws); }}
                        className="p-0.5 rounded hover:bg-muted/60 text-muted-foreground/70 hover:text-foreground transition-colors"
                        title="Edit workspace"
                      >
                        <Settings className="size-3" />
                      </button>
                      <button
                        onClick={(e) => { e.stopPropagation(); deleteWorkspace(ws.workspace_id); }}
                        className="p-0.5 rounded hover:bg-destructive/15 text-muted-foreground/70 hover:text-destructive transition-colors"
                        title="Delete workspace"
                      >
                        <Trash2 className="size-3" />
                      </button>
                    </span>
                  )}
                </div>

                {/* compact metadata */}
                <div className="mt-1 flex items-center gap-2 text-[10px] text-muted-foreground/90">
                  <div className="inline-flex items-center gap-1 min-w-0">
                    <FolderOpen className="size-2.5" />
                    <span className="truncate">{ws.folder ? ws.folder.split(/[\\/]/).pop() : "No folder"}</span>
                  </div>
                  {ws.gitBranch && (
                    <div className="inline-flex items-center gap-1 min-w-0">
                      <GitBranch className="size-2.5" />
                      <span className="truncate max-w-[90px]">{ws.gitBranch}</span>
                    </div>
                  )}
                  <span className="ml-auto inline-flex items-center gap-1 text-[10px] text-muted-foreground/70">
                    {ws.terminalCount > 0 ? (
                      <>
                        <TerminalIcon className="size-2.5" />
                        <span>{ws.terminalCount}</span>
                      </>
                    ) : (
                      <span>Idle</span>
                    )}
                  </span>
                </div>

                {ws.notificationText && !active && (
                  <div className="mt-1 flex items-center gap-1 text-[10px] text-chart-1">
                    <span className="h-1 w-1 rounded-full bg-chart-1" />
                    <span className="truncate">{ws.notificationText}</span>
                  </div>
                )}
              </button>
            );
          })}
        </div>

        {/* footer status */}
        <div className="border-t px-3 py-2 flex items-center justify-between bg-sidebar">
          <span className="text-[10px] text-muted-foreground truncate">
            {readiness ? (readiness.ready ? "Ready" : "Not ready") : backendStatus}
          </span>
          <div className="flex items-center gap-1">
            <Button
              size="icon-xs"
              variant="ghost"
              onClick={() => setNotificationPanelOpen(true)}
              title="Notifications"
              className="relative"
            >
              <Bell className="size-3" />
              {unreadCount > 0 && (
                <span className="absolute -right-1 -top-1 rounded-full bg-destructive px-1 text-[9px] text-white">
                  {unreadCount > 99 ? "99+" : unreadCount}
                </span>
              )}
            </Button>
            <Button
              size="icon-xs"
              variant="ghost"
              onClick={() => setSettingsOpen(true)}
              title="Settings"
            >
              <Settings className="size-3" />
            </Button>
            <Button size="icon-xs" variant="ghost" onClick={() => setShowShortcuts((v) => !v)} title="Keyboard Shortcuts (Ctrl+/)">
              <Keyboard className="size-3" />
            </Button>
          </div>
        </div>
      </aside>
    );
  }

  // ---------------------------------------------------------------------------
  // Main content area (pane tree)
  // ---------------------------------------------------------------------------
  function renderContent() {
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
    if (!paneTree) {
      return (
        <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
          <span>Loading workspace layout...</span>
        </div>
      );
    }

    // Rebuild pane tree with latest surface data
    const liveTree = injectSurfaces(paneTree, surfaces);
    const paneCount = countPanes(liveTree);

    return (
      <PaneContainer
        node={liveTree}
        focusedPaneId={focusedPaneId}
        token={token}
        paneCount={paneCount}
        onFocusPane={setFocusedPaneId}
        onSplitPane={handleSplitPane}
        onClosePane={handleClosePane}
        onCreateSurface={handleCreateSurface}
        onCloseSurface={handleCloseSurface}
        onFocusSurface={handleFocusSurface}
        onTerminalData={sendTerminalInput}
        onTerminalResize={sendTerminalResize}
        registerXtermHandle={registerXtermHandle}
        onAttachTerminal={attachTerminal}
        onAttachBrowser={attachBrowser}
        onBrowserNavigate={handleBrowserNavigate}
        onBrowserReload={handleBrowserReload}
        onBrowserBack={handleBrowserBack}
        onBrowserForward={handleBrowserForward}
        onBrowserScreenshot={handleBrowserScreenshot}
        browserStates={browserStates}
        workspaceFolder={selectedWorkspace?.folder}
      />
    );
  }

  /** Inject the latest surface state into the pane tree leaves */
  function injectSurfaces(node: PaneNode, allSurfaces: SurfaceState[]): PaneNode {
    if (node.type === "split") {
      return {
        ...node,
        children: [
          injectSurfaces(node.children[0], allSurfaces),
          injectSurfaces(node.children[1], allSurfaces),
        ],
      };
    }
    const paneSurfaces = allSurfaces
      .filter((s) => s.paneId === node.paneId)
      .sort((a, b) => a.order - b.order);
    const focused = paneSurfaces.find((s) => s.focused);
    return {
      ...node,
      surfaces: paneSurfaces,
      activeSurfaceId: node.activeSurfaceId && paneSurfaces.some((s) => s.surfaceId === node.activeSurfaceId)
        ? node.activeSurfaceId
        : focused?.surfaceId ?? paneSurfaces[0]?.surfaceId ?? null,
    };
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

  function renderEditDrawer() {
    const ws = workspaces.find((w) => w.workspace_id === editWorkspaceId);
    return (
      <>
        <div
          className={cn(
            "fixed inset-0 z-40 bg-black/40 transition-opacity duration-200",
            editDrawerOpen ? "opacity-100" : "pointer-events-none opacity-0",
          )}
          onClick={closeEditDrawer}
        />
        <div
          className={cn(
            "fixed right-0 top-0 z-50 flex h-full w-[380px] flex-col border-l bg-card shadow-xl transition-transform duration-200 ease-out",
            editDrawerOpen ? "translate-x-0" : "translate-x-full",
          )}
        >
          <div className="flex items-center justify-between border-b px-5 py-4">
            <div className="space-y-0.5">
              <h2 className="text-sm font-semibold">Workspace Settings</h2>
              <div className="text-[11px] text-muted-foreground truncate">
                {ws?.name || "Workspace"}
              </div>
            </div>
            <Button variant="ghost" size="icon-sm" onClick={closeEditDrawer}>
              <X className="size-4" />
            </Button>
          </div>
          <div className="flex-1 overflow-auto px-5 py-4 space-y-5 text-xs">
            <div className="space-y-1.5">
              <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Working Directory
              </label>
              <div className="rounded-md border bg-muted px-3 py-2 text-xs text-muted-foreground">
                {ws?.folder || "No folder set"}
              </div>
            </div>
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <label className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  Environment Variables
                </label>
                <Button variant="ghost" size="icon-xs" onClick={addEditEnvEntry}>
                  <Plus className="size-3.5" />
                </Button>
              </div>
              {editEnvVars.length === 0 && (
                <div className="rounded-md border border-dashed px-3 py-3 text-center text-[11px] text-muted-foreground">
                  No environment variables. Click + to add.
                </div>
              )}
              <div className="space-y-2">
                {editEnvVars.map((entry, i) => (
                  <div key={i} className="flex items-center gap-1.5">
                    <input
                      type="text"
                      value={entry.key}
                      onChange={(e) => updateEditEnvEntry(i, "key", e.target.value)}
                      placeholder="KEY"
                      className="w-[120px] shrink-0 rounded-md border bg-background px-2 py-1.5 text-xs outline-none transition focus:border-primary/50 focus:ring-1 focus:ring-primary/30"
                    />
                    <span className="text-[11px] text-muted-foreground">=</span>
                    <input
                      type="text"
                      value={entry.value}
                      onChange={(e) => updateEditEnvEntry(i, "value", e.target.value)}
                      placeholder="value"
                      className="flex-1 rounded-md border bg-background px-2 py-1.5 text-xs outline-none transition focus:border-primary/50 focus:ring-1 focus:ring-primary/30"
                    />
                    <Button variant="ghost" size="icon-xs" onClick={() => removeEditEnvEntry(i)}>
                      <Trash2 className="size-3 text-muted-foreground hover:text-destructive" />
                    </Button>
                  </div>
                ))}
              </div>
            </div>
            <div className="space-y-2">
              <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                MAXC Environment (read-only)
              </div>
              <div className="rounded-md border bg-muted px-3 py-2 text-[11px] text-muted-foreground whitespace-pre-wrap break-words">
                {Object.entries(ws?.env_vars ?? {})
                  .filter(([key]) => key.startsWith("MAXC_"))
                  .map(([key, value]) => `${key}=${value}`)
                  .join("\n") || "Not available yet."}
              </div>
            </div>
          </div>
          <div className="border-t px-5 py-4 space-y-2">
            {editError && (
              <div className="rounded-md bg-destructive/10 px-3 py-2 text-xs text-destructive">
                {editError}
              </div>
            )}
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={closeEditDrawer}
                className="flex-1 h-9"
                disabled={editSaving}
              >
                Cancel
              </Button>
              <Button
                variant="default"
                size="sm"
                onClick={saveWorkspaceEnv}
                className="flex-1 h-9"
                disabled={editSaving}
              >
                {editSaving ? "Saving..." : "Save"}
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
      { keys: "Ctrl+Shift+N", desc: "New window" },
      { keys: "Ctrl+N", desc: "New workspace" },
      { keys: "Ctrl+T", desc: "New terminal" },
      { keys: "Ctrl+B", desc: "New browser" },
      { keys: "Ctrl+D", desc: "Split pane right" },
      { keys: "Ctrl+Shift+D", desc: "Split pane down" },
      { keys: "Alt+1-9", desc: "Switch workspace" },
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

  function renderNotificationToasts() {
    if (toastItems.length === 0) return null;
    return (
      <div className="fixed right-4 top-12 z-50 flex w-[280px] flex-col gap-2">
        {toastItems.map((n) => (
          <div
            key={n.notification_id}
            className={cn(
              "rounded-lg border px-3 py-2 text-xs shadow-lg backdrop-blur",
              n.level === "success"
                ? "border-emerald-500/20 bg-emerald-500/10 text-emerald-100"
                : n.level === "warning"
                  ? "border-amber-500/20 bg-amber-500/10 text-amber-100"
                  : n.level === "error"
                    ? "border-rose-500/20 bg-rose-500/10 text-rose-100"
                    : "border-blue-500/20 bg-blue-500/10 text-blue-100",
            )}
          >
            <div className="font-semibold">{n.title}</div>
            {n.body && <div className="mt-1 text-[11px] text-white/70">{n.body}</div>}
          </div>
        ))}
      </div>
    );
  }

  function renderSettingsPanel() {
    const envLines = [
      `MAXC_SOCKET_PATH=${socketPath}`,
      token ? `MAXC_TOKEN=${token}` : "MAXC_TOKEN=",
      selectedWorkspaceId ? `MAXC_WORKSPACE_ID=${selectedWorkspaceId}` : "MAXC_WORKSPACE_ID=",
      focusedPaneId ? `MAXC_PANE_ID=${focusedPaneId}` : "MAXC_PANE_ID=",
      focusedSurface?.surfaceId
        ? `MAXC_SURFACE_ID=${focusedSurface.surfaceId}`
        : "MAXC_SURFACE_ID=",
    ];
    const envBlock = envLines.join("\n");
    const isUpdateDownloading = updateStatus === "downloading";

    async function copyEnvBlock() {
      try {
        await navigator.clipboard.writeText(envBlock);
        setEnvCopied(true);
        window.setTimeout(() => setEnvCopied(false), 2000);
      } catch {
        setBackendStatus("Failed to copy env");
      }
    }

    async function checkForUpdates() {
      setUpdateStatus("checking");
      setUpdateError("");
      setUpdateInfo(null);
      try {
        const result = await invoke<UpdateInfo>("update_check", { channel: updateChannel });
        if (!result.available) {
          setUpdateStatus("uptodate");
          return;
        }
        setUpdateInfo(result);
        setUpdateStatus("available");
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (/404|not found|no release/i.test(message)) {
          setUpdateStatus("uptodate");
          setUpdateError("");
          return;
        }
        setUpdateStatus("error");
        setUpdateError(message || "Update check failed");
      }
    }

    async function downloadAndInstallUpdate() {
      setUpdateStatus("downloading");
      setUpdateError("");
      setUpdateProgress({ downloaded: 0, total: undefined });
      try {
        await invoke("update_download_and_install", { channel: updateChannel });
        setUpdateStatus("ready");
      } catch (err) {
        setUpdateStatus("error");
        setUpdateError((err as Error).message || "Update download failed");
      }
    }

    return (
      <>
        <div
          className={cn(
            "fixed inset-0 z-40 bg-black/40 transition-opacity duration-200",
            settingsOpen ? "opacity-100" : "pointer-events-none opacity-0",
          )}
          onClick={() => setSettingsOpen(false)}
        />
        <div
          className={cn(
            "fixed right-0 top-0 z-50 flex h-full w-[380px] flex-col border-l bg-card shadow-xl transition-transform duration-200 ease-out",
            settingsOpen ? "translate-x-0" : "translate-x-full",
          )}
        >
          <div className="flex items-center justify-between border-b px-5 py-4">
            <h2 className="text-sm font-semibold">Settings</h2>
            <Button variant="ghost" size="icon-sm" onClick={() => setSettingsOpen(false)}>
              <X className="size-4" />
            </Button>
          </div>
          <div className="flex-1 overflow-auto overflow-x-hidden px-5 py-4 space-y-5 text-xs">
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  Updates
                </span>
                <div className="text-[10px] text-muted-foreground">
                  {appVersion ? `v${appVersion}` : "v—"}
                </div>
              </div>
              <div className="flex items-center gap-2">
                <label className="text-[10px] text-muted-foreground">Channel</label>
                <div className="relative">
                  <select
                    className="h-7 appearance-none rounded-md border border-border/70 bg-background px-2 pr-6 text-[11px] text-foreground shadow-sm outline-none transition focus:border-primary/60 focus:ring-1 focus:ring-primary/30"
                    value={updateChannel}
                    onChange={(e) => setUpdateChannel(e.target.value as "stable" | "beta")}
                  >
                    <option value="stable">Stable</option>
                    <option value="beta">Beta</option>
                  </select>
                  <ChevronDown className="pointer-events-none absolute right-1.5 top-1/2 size-3 -translate-y-1/2 text-muted-foreground" />
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 px-2"
                  onClick={checkForUpdates}
                  disabled={updateStatus === "checking" || isUpdateDownloading}
                >
                  {updateStatus === "checking" ? "Checking..." : "Check"}
                </Button>
              </div>
              {updateStatus === "available" && updateInfo && (
                <div className="rounded-md border bg-muted px-3 py-2">
                  <div className="text-[11px] text-foreground">
                    Update available: v{updateInfo.version ?? "new"}
                  </div>
                  {updateInfo.date_ms ? (
                    <div className="text-[10px] text-muted-foreground">
                      {new Date(updateInfo.date_ms).toLocaleString()}
                    </div>
                  ) : null}
                  {updateInfo.body && (
                    <div className="mt-2 text-[10px] text-muted-foreground whitespace-pre-wrap">
                      {updateInfo.body}
                    </div>
                  )}
                  <div className="mt-2">
                    <Button
                      variant="default"
                      size="sm"
                      className="h-7 px-2"
                      onClick={downloadAndInstallUpdate}
                      disabled={isUpdateDownloading}
                    >
                      {isUpdateDownloading ? "Downloading..." : "Download & Install"}
                    </Button>
                  </div>
                </div>
              )}
              {updateStatus === "downloading" && (
                <div className="text-[10px] text-muted-foreground">
                  Downloading update…{" "}
                  {updateProgress.total
                    ? `${Math.round((updateProgress.downloaded / updateProgress.total) * 100)}%`
                    : ""}
                </div>
              )}
              {updateStatus === "ready" && (
                <div className="text-[10px] text-muted-foreground">
                  Update installed. Restarting…
                </div>
              )}
              {updateStatus === "uptodate" && (
                <div className="text-[10px] text-muted-foreground">You are up to date.</div>
              )}
              {updateStatus === "error" && (
                <div className="text-[10px] text-destructive">
                  {updateError || "Updater not configured."}
                </div>
              )}
            </div>

            <div className="flex items-center gap-3 text-[10px] text-muted-foreground/80">
              <div className="h-px flex-1 bg-border/70" />
              <span className="uppercase tracking-[0.2em]">Agent</span>
              <div className="h-px flex-1 bg-border/70" />
            </div>

            <div className="space-y-2">
              <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Agent Connection
              </div>
              <p className="text-muted-foreground">
                Use these environment variables to let an external agent access all maxc
                commands. Agents launched inside a maxc terminal inherit these values
                automatically.
              </p>
            </div>

            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                  MAXC Environment
                </span>
                <Button variant="outline" size="sm" className="h-7 px-2" onClick={copyEnvBlock}>
                  {envCopied ? (
                    <span className="inline-flex items-center gap-1">
                      <Check className="size-3.5" />
                      Copied
                    </span>
                  ) : (
                    "Copy"
                  )}
                </Button>
              </div>
              <pre className="whitespace-pre-wrap break-words rounded-md border bg-muted px-3 py-2 text-[11px] text-muted-foreground">
                {envBlock}
              </pre>
            </div>

            <div className="space-y-2">
              <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
                Example CLI Usage
              </div>
              <pre className="whitespace-pre-wrap break-words rounded-md border bg-muted px-3 py-2 text-[11px] text-muted-foreground">
{`maxc terminal spawn --workspace-id ${selectedWorkspaceId || "ws-1"} --surface-id ${focusedSurface?.surfaceId || "sf-1"}
maxc terminal input --workspace-id ${selectedWorkspaceId || "ws-1"} --surface-id ${focusedSurface?.surfaceId || "sf-1"} --terminal-session-id ts-1 --input "npm test\\n"
maxc notify --title "Task complete" --body "All tests passed" --level success`}
              </pre>
            </div>
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
        <main className="flex flex-1 min-h-0 flex-col overflow-hidden">
          <div className="flex-1 min-h-0">{renderContent()}</div>
        </main>
      </div>
      {renderDrawer()}
      {renderEditDrawer()}
      {renderShortcutsPanel()}
      {renderNotificationToasts()}
      {renderSettingsPanel()}
      <NotificationPanel
        open={notificationPanelOpen}
        notifications={notificationItems}
        onClose={() => setNotificationPanelOpen(false)}
        onClearAll={clearAllNotifications}
        onClearNotification={clearNotification}
      />
    </div>
  );
}

export default App;
