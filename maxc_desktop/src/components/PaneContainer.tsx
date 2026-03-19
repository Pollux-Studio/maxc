import { Group, Panel, Separator } from "react-resizable-panels";
import {
  Bot,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  Globe2,
  Terminal as TerminalIcon,
} from "lucide-react";
import { Shimmer } from "@/components/ai-elements/shimmer";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuLabel,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { SurfaceTabBar, type SurfaceState } from "./SurfaceTabBar";
import { XtermTerminal, type XtermHandle } from "./XtermTerminal";
import { BrowserView } from "./BrowserView";
import AgentPanel from "./AgentPanel";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------
// Pane tree data model
// ---------------------------------------------------------------------------

export type PaneNode =
  | {
      type: "leaf";
      paneId: string;
      surfaces: SurfaceState[];
      activeSurfaceId: string | null;
    }
  | {
      type: "split";
      paneId: string;
      direction: "horizontal" | "vertical";
      children: [PaneNode, PaneNode];
    };

export type BrowserState = {
  currentUrl: string;
  screenshotData: string | null;
  screenshotLoading: boolean;
  sessionLoading?: boolean;
  tabId: string | null;
  lastCaptureMs?: number;
};

export type PaneContainerProps = {
  node: PaneNode;
  focusedPaneId: string;
  token: string;
  paneCount: number;
  onFocusPane: (paneId: string) => void;
  onSplitPane: (paneId: string, direction: "horizontal" | "vertical") => void;
  onClosePane: (paneId: string) => void;
  onCreateSurface: (paneId: string, panelType: string) => void;
  onCloseSurface: (surfaceId: string) => void;
  onFocusSurface: (surfaceId: string) => void;
  onTerminalData: (surfaceId: string, data: string) => void;
  onTerminalResize: (surfaceId: string, cols: number, rows: number) => void;
  registerXtermHandle: (surfaceId: string, handle: XtermHandle | null) => void;
  onAttachTerminal: (surfaceId: string) => void;
  onAttachBrowser: (surfaceId: string) => void;
  // Browser callbacks
  onBrowserNavigate: (surfaceId: string, url: string) => void;
  onBrowserReload: (surfaceId: string) => void;
  onBrowserBack: (surfaceId: string) => void;
  onBrowserForward: (surfaceId: string) => void;
  onBrowserScreenshot: (surfaceId: string) => void;
  browserStates: Map<string, BrowserState>;
  workspaceFolder?: string;
  canCreateSurface?: boolean;
  /** Whether browser webviews should be visible (false when dialogs cover them) */
  browserWebviewsVisible: boolean;
};

// ---------------------------------------------------------------------------
// Recursive pane container
// ---------------------------------------------------------------------------

export function PaneContainer(props: PaneContainerProps) {
  const { node } = props;

  if (node.type === "split") {
    const isHorizontalDivider = node.direction === "horizontal";
    const groupOrientation = isHorizontalDivider ? "vertical" : "horizontal";
    return (
      <Group
        orientation={groupOrientation}
        className="h-full w-full"
        id={`pane-${node.paneId}`}
      >
        <Panel minSize={10} defaultSize={50}>
          <PaneContainer {...props} node={node.children[0]} />
        </Panel>
        <Separator
          className={cn(
            "group relative flex items-center justify-center bg-transparent",
            isHorizontalDivider
              ? "h-2 -my-1 w-full cursor-row-resize"
              : "w-2 -mx-1 h-full cursor-col-resize",
          )}
        >
          <div
            className={cn(
              "rounded-full bg-border transition-colors group-hover:bg-primary group-active:bg-primary",
              isHorizontalDivider
                ? "absolute top-1/2 left-0 right-0 h-px -translate-y-1/2"
                : "absolute left-1/2 top-0 bottom-0 w-px -translate-x-1/2",
            )}
          />
          <div className="pointer-events-none absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-card/90 px-1.5 py-0.5 text-muted-foreground shadow-sm opacity-70 group-hover:opacity-100">
            {isHorizontalDivider ? (
              <div className="flex items-center gap-0.5">
                <ChevronUp className="size-3" />
                <ChevronDown className="size-3" />
              </div>
            ) : (
              <div className="flex items-center gap-0.5">
                <ChevronLeft className="size-3" />
                <ChevronRight className="size-3" />
              </div>
            )}
          </div>
        </Separator>
        <Panel minSize={10} defaultSize={50}>
          <PaneContainer {...props} node={node.children[1]} />
        </Panel>
      </Group>
    );
  }

  // Leaf pane: show SurfaceTabBar + active panel
  const isFocused = props.focusedPaneId === node.paneId;
  const activeSurface = node.surfaces.find(
    (s) => s.surfaceId === node.activeSurfaceId,
  );

  return (
    <ContextMenu modal={false}>
      <ContextMenuTrigger asChild>
        <div
          className={cn(
            "flex h-full w-full flex-col min-h-0 min-w-0",
            isFocused && "ring-1 ring-primary/30 ring-inset",
          )}
          onClick={() => props.onFocusPane(node.paneId)}
          onContextMenu={() => props.onFocusPane(node.paneId)}
        >
          <SurfaceTabBar
            surfaces={node.surfaces}
            activeSurfaceId={node.activeSurfaceId}
            paneId={node.paneId}
            onFocusSurface={props.onFocusSurface}
            onCloseSurface={props.onCloseSurface}
            onCreateSurface={props.onCreateSurface}
            onSplitPane={props.onSplitPane}
            onClosePane={props.onClosePane}
            canClosePane={props.paneCount > 1}
          />
          <div className="flex-1 min-h-0 bg-background relative">
            {/*
              Render ALL browser surfaces in this pane persistently.
              Native webviews are OS-level windows — they must be hidden/shown
              via Tauri API, not mounted/unmounted via React.
              Only the active browser surface with no dialogs open is visible.
            */}
            {node.surfaces
              .filter((s) => s.panelType === "browser" && s.browserSessionId)
              .map((s) => {
                const bs = props.browserStates.get(s.surfaceId);
                const isActiveBrowser =
                  node.activeSurfaceId === s.surfaceId;
                return (
                  <div
                    key={`browser-${s.surfaceId}`}
                    className={cn("absolute inset-0", !isActiveBrowser && "pointer-events-none")}
                    style={{ zIndex: isActiveBrowser ? 1 : -1 }}
                  >
                    <BrowserView
                      surfaceId={s.surfaceId}
                      browserSessionId={s.browserSessionId!}
                      workspaceId={s.workspaceId}
                      tabId={bs?.tabId ?? null}
                      focused={isFocused && isActiveBrowser}
                      visible={isActiveBrowser && props.browserWebviewsVisible}
                      onNavigate={props.onBrowserNavigate}
                      onReload={props.onBrowserReload}
                      onBack={props.onBrowserBack}
                      onForward={props.onBrowserForward}
                      onScreenshot={props.onBrowserScreenshot}
                      currentUrl={bs?.currentUrl ?? ""}
                      screenshotData={bs?.screenshotData ?? null}
                      screenshotLoading={bs?.screenshotLoading ?? false}
                      sessionLoading={bs?.sessionLoading ?? false}
                    />
                  </div>
                );
              })}

            {/* Active non-browser panel content */}
            {activeSurface?.panelType === "terminal" &&
            activeSurface.terminalSessionId ? (
              <XtermTerminal
                key={activeSurface.surfaceId}
                ref={(handle) =>
                  props.registerXtermHandle(activeSurface.surfaceId, handle)
                }
                focused={isFocused}
                onData={(data) =>
                  props.onTerminalData(activeSurface.surfaceId, data)
                }
                onResize={(cols, rows) =>
                  props.onTerminalResize(activeSurface.surfaceId, cols, rows)
                }
              />
            ) : activeSurface?.panelType === "browser" &&
              activeSurface.browserSessionId ? (
              // Active browser — already rendered above persistently, nothing to render here
              null
            ) : activeSurface?.panelType === "browser" ? (
              (() => {
                const bs = props.browserStates.get(activeSurface.surfaceId);
                return (
                  <div className="flex h-full items-center justify-center">
                    <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-border/60 bg-card/60 px-6 py-5 text-center">
                      <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground">
                        <Globe2 className="size-4" />
                      </div>
                      <div className="space-y-1">
                        <div className="text-[12px] font-medium text-foreground">Browser session</div>
                        <div className="text-[10px] text-muted-foreground">
                          {bs?.sessionLoading ? "Launching renderer…" : "Attach a live browser to this surface."}
                        </div>
                      </div>
                      {bs?.sessionLoading ? (
                        <div className="w-full max-w-[200px] text-left">
                          <Shimmer className="text-[11px] text-muted-foreground">
                            Warming GPU context… routing automation…
                          </Shimmer>
                        </div>
                      ) : (
                        <button
                          className="rounded-md border border-border/70 bg-muted/40 px-2.5 py-1 text-[11px] text-foreground/80 hover:bg-muted/70"
                          onClick={() => props.onAttachBrowser(activeSurface.surfaceId)}
                        >
                          Start browser
                        </button>
                      )}
                    </div>
                  </div>
                );
              })()
            ) : activeSurface?.panelType === "terminal" ? (
              <div className="flex h-full items-center justify-center">
                <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-border/60 bg-card/60 px-6 py-5 text-center">
                  <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground">
                    <TerminalIcon className="size-4" />
                  </div>
                  <div className="space-y-1">
                    <div className="text-[12px] font-medium text-foreground">Terminal session</div>
                    <div className="text-[10px] text-muted-foreground">Spawn a terminal in this workspace.</div>
                  </div>
                  <button
                    className="rounded-md border border-border/70 bg-muted/40 px-2.5 py-1 text-[11px] text-foreground/80 hover:bg-muted/70"
                    onClick={() => props.onAttachTerminal(activeSurface.surfaceId)}
                  >
                    Start terminal
                  </button>
                </div>
              </div>
            ) : activeSurface?.panelType === "agent" ? (
              <AgentPanel
                token={props.token}
                workspaceId={activeSurface.workspaceId}
                surfaceId={activeSurface.surfaceId}
                workspaceFolder={props.workspaceFolder}
              />
            ) : activeSurface ? (
              <div className="flex h-full items-center justify-center text-xs text-[#767676]">
                {activeSurface.panelType === "browser"
                  ? "Browser session not ready"
                  : "Empty surface"}
              </div>
            ) : (
              <div className="flex h-full items-center justify-center">
                <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-border/60 bg-card/60 px-6 py-5 text-center">
                  <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground">
                    <Bot className="size-4" />
                  </div>
                  <div className="space-y-1">
                    <div className="text-[12px] font-medium text-foreground">No surfaces yet</div>
                    <div className="text-[10px] text-muted-foreground">Create a terminal, browser, or agent tab.</div>
                  </div>
                  <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                    <kbd className="rounded border border-border bg-muted/40 px-1.5 py-0.5">Ctrl+T</kbd>
                    <kbd className="rounded border border-border bg-muted/40 px-1.5 py-0.5">Ctrl+B</kbd>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuLabel>New Surface</ContextMenuLabel>
        <ContextMenuSeparator />
        <ContextMenuItem
          disabled={!props.canCreateSurface}
          onSelect={() => props.onCreateSurface(node.paneId, "terminal")}
        >
          Create Terminal
        </ContextMenuItem>
        <ContextMenuItem
          disabled={!props.canCreateSurface}
          onSelect={() => props.onCreateSurface(node.paneId, "browser")}
        >
          Create Browser
        </ContextMenuItem>
        <ContextMenuItem
          disabled={!props.canCreateSurface}
          onSelect={() => props.onCreateSurface(node.paneId, "agent")}
        >
          Create Agent
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
