import {
  Columns2,
  Globe2,
  Bot,
  Plus,
  Rows2,
  Terminal as TerminalIcon,
  X,
} from "lucide-react";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------
// Surface state
// ---------------------------------------------------------------------------

export type SurfaceState = {
  surfaceId: string;
  paneId: string;
  workspaceId: string;
  title: string;
  panelType: string; // "terminal" | "browser" | "agent"
  terminalSessionId: string | null;
  browserSessionId: string | null;
  order: number;
  focused: boolean;
  lastSequence: number;
  hasNewOutput: boolean;
};

export type SurfaceTabBarProps = {
  surfaces: SurfaceState[];
  activeSurfaceId: string | null;
  paneId: string;
  canClosePane: boolean;
  onFocusSurface: (surfaceId: string) => void;
  onCloseSurface: (surfaceId: string) => void;
  onCreateSurface: (paneId: string, panelType: string) => void;
  onSplitPane: (paneId: string, direction: "horizontal" | "vertical") => void;
  onClosePane: (paneId: string) => void;
};

function PanelIcon({ type }: { type: string }) {
  switch (type) {
    case "browser":
      return <Globe2 className="size-3" />;
    case "agent":
      return <Bot className="size-3" />;
    default:
      return <TerminalIcon className="size-3" />;
  }
}

export function SurfaceTabBar({
  surfaces,
  activeSurfaceId,
  paneId,
  onFocusSurface,
  onCloseSurface,
  onCreateSurface,
  onSplitPane,
  onClosePane,
  canClosePane,
}: SurfaceTabBarProps) {
  const sorted = [...surfaces].sort((a, b) => a.order - b.order);

  return (
    <div className="surface-tabbar flex items-center bg-[#1a1a1a] border-b border-[#333] text-[10px] h-7 shrink-0 overflow-x-auto">
      {/* Tabs */}
      {sorted.map((s) => {
        const active = s.surfaceId === activeSurfaceId;
        return (
          <button
            key={s.surfaceId}
            onClick={(e) => {
              e.stopPropagation();
              onFocusSurface(s.surfaceId);
            }}
            className={cn(
              "group flex items-center gap-1 px-2.5 h-full border-r border-[#333] transition-colors whitespace-nowrap",
              active
                ? "bg-[#0c0c0c] text-[#cccccc]"
                : "text-[#888888] hover:text-[#cccccc] hover:bg-[#222]",
            )}
          >
            <PanelIcon type={s.panelType} />
            <span className="max-w-[100px] truncate">{s.title || "Untitled"}</span>
            {s.hasNewOutput && !active && (
              <span className="size-1.5 rounded-full bg-chart-1 animate-pulse" />
            )}
            <button
              onClick={(e) => {
                e.stopPropagation();
                onCloseSurface(s.surfaceId);
              }}
              className="ml-1 rounded p-0.5 opacity-0 group-hover:opacity-100 hover:bg-[#444] transition-opacity"
              aria-label="Close tab"
            >
              <X className="size-2.5" />
            </button>
          </button>
        );
      })}

      {/* New tab button */}
      <button
        onClick={(e) => {
          e.stopPropagation();
          onCreateSurface(paneId, "terminal");
        }}
        className="flex items-center justify-center px-1.5 h-full text-[#666] hover:text-[#ccc] transition-colors"
        title="New Terminal (Ctrl+T)"
      >
        <Plus className="size-3" />
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onCreateSurface(paneId, "browser");
        }}
        className="flex items-center justify-center px-1.5 h-full text-[#666] hover:text-[#ccc] transition-colors"
        title="New Browser"
      >
        <Globe2 className="size-3" />
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation();
          onCreateSurface(paneId, "agent");
        }}
        className="flex items-center justify-center px-1.5 h-full text-[#666] hover:text-[#ccc] transition-colors"
        title="New Agent"
      >
        <Bot className="size-3" />
      </button>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Pane actions */}
      <div className="flex items-center gap-0.5 px-1">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onSplitPane(paneId, "vertical");
          }}
          className="p-0.5 text-[#555] hover:text-[#ccc] transition-colors"
          title="Split Right (Ctrl+D)"
        >
          <Columns2 className="size-3" />
        </button>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onSplitPane(paneId, "horizontal");
          }}
          className="p-0.5 text-[#555] hover:text-[#ccc] transition-colors"
          title="Split Down (Ctrl+Shift+D)"
        >
          <Rows2 className="size-3" />
        </button>
        {canClosePane && surfaces.length === 0 && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onClosePane(paneId);
            }}
            className="p-0.5 text-[#555] hover:text-[#ccc] transition-colors"
            title="Close Pane"
          >
            <X className="size-3" />
          </button>
        )}
      </div>
    </div>
  );
}
