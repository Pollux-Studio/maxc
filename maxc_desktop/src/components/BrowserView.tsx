import { useCallback, useEffect, useRef, useState } from "react";
import { Webview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalPosition, LogicalSize } from "@tauri-apps/api/dpi";
import { ArrowLeft, ArrowRight, Globe2, RotateCw, ExternalLink } from "lucide-react";
import { Shimmer } from "@/components/ai-elements/shimmer";

export type BrowserViewProps = {
  surfaceId: string;
  browserSessionId: string;
  workspaceId: string;
  tabId: string | null;
  focused: boolean;
  /** Whether the webview should be visible (false when dialogs/drawers cover it) */
  visible: boolean;
  onNavigate: (surfaceId: string, url: string) => void;
  onReload: (surfaceId: string) => void;
  onBack: (surfaceId: string) => void;
  onForward: (surfaceId: string) => void;
  onScreenshot: (surfaceId: string) => void;
  currentUrl: string;
  screenshotData: string | null;
  screenshotLoading: boolean;
  sessionLoading: boolean;
};

export function BrowserView({
  surfaceId,
  onNavigate,
  onReload,
  onBack,
  onForward,
  currentUrl,
  sessionLoading,
  focused,
  visible,
}: BrowserViewProps) {
  const [urlInput, setUrlInput] = useState(currentUrl);
  const containerRef = useRef<HTMLDivElement>(null);
  const webviewRef = useRef<Webview | null>(null);
  const labelRef = useRef(`browser-${surfaceId.replace(/[^a-zA-Z0-9\-_/:]/g, "-")}`);
  const mountedUrlRef = useRef("");
  const [webviewReady, setWebviewReady] = useState(false);
  const [webviewError, setWebviewError] = useState("");

  useEffect(() => {
    setUrlInput(currentUrl);
  }, [currentUrl]);

  // --- Create / update the native webview ---
  const createWebview = useCallback(async (url: string) => {
    const el = containerRef.current;
    if (!el || !url) return;

    // Clean up previous webview
    if (webviewRef.current) {
      try {
        await webviewRef.current.close();
      } catch { /* already closed */ }
      webviewRef.current = null;
    }

    setWebviewReady(false);
    setWebviewError("");
    mountedUrlRef.current = url;

    try {
      const rect = el.getBoundingClientRect();

      const webview = new Webview(getCurrentWindow(), labelRef.current, {
        url,
        x: Math.round(rect.left),
        y: Math.round(rect.top),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
        transparent: false,
      });

      webview.once("tauri://created", () => {
        setWebviewReady(true);
      });

      webview.once("tauri://error", (e) => {
        console.error("Webview creation error:", e);
        setWebviewError(typeof e.payload === "string" ? e.payload : "Failed to create webview");
      });

      webviewRef.current = webview;
    } catch (err) {
      console.error("Failed to create webview:", err);
      setWebviewError((err as Error).message || "Failed to create webview");
    }
  }, []);

  // Create webview when URL changes
  useEffect(() => {
    if (currentUrl && currentUrl !== mountedUrlRef.current) {
      createWebview(currentUrl);
    }
  }, [currentUrl, createWebview]);

  // Sync position/size with container on resize
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const sync = () => {
      const wv = webviewRef.current;
      if (!wv) return;
      const rect = el.getBoundingClientRect();
      try {
        wv.setPosition(new LogicalPosition(rect.left, rect.top));
        wv.setSize(new LogicalSize(rect.width, rect.height));
      } catch { /* ignore position errors during transitions */ }
    };

    const ro = new ResizeObserver(() => requestAnimationFrame(sync));
    ro.observe(el);
    window.addEventListener("resize", sync);

    return () => {
      ro.disconnect();
      window.removeEventListener("resize", sync);
    };
  }, [webviewReady]);

  // Hide/show webview when switching tabs or when dialogs cover it.
  // Uses setPosition + setSize instead of hide/show for reliability.
  useEffect(() => {
    const wv = webviewRef.current;
    if (!wv) return;
    if (visible) {
      // Restore to correct position and size
      const el = containerRef.current;
      if (el) {
        const rect = el.getBoundingClientRect();
        wv.setPosition(new LogicalPosition(rect.left, rect.top)).catch(() => {});
        wv.setSize(new LogicalSize(Math.max(rect.width, 1), Math.max(rect.height, 1))).catch(() => {});
      }
    } else {
      // Collapse to zero size and move off-screen
      wv.setSize(new LogicalSize(0, 0)).catch(() => {});
      wv.setPosition(new LogicalPosition(-9999, -9999)).catch(() => {});
    }
  }, [visible]);

  // Focus webview when pane is focused
  useEffect(() => {
    if (focused && visible && webviewRef.current) {
      try { webviewRef.current.setFocus(); } catch { /* ignore */ }
    }
  }, [focused, visible]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (webviewRef.current) {
        webviewRef.current.close().catch(() => {});
        webviewRef.current = null;
      }
    };
  }, []);

  function handleNavigate() {
    let url = urlInput.trim();
    if (!url) return;
    if (!/^https?:\/\//i.test(url) && !url.startsWith("about:")) {
      url = "https://" + url;
    }
    setUrlInput(url);
    onNavigate(surfaceId, url);
  }

  return (
    <div className="flex h-full flex-col bg-background">
      {/* URL bar */}
      <div className="flex items-center gap-2 border-b border-border bg-card px-2 py-1">
        <button
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
          onClick={() => onBack(surfaceId)}
          title="Back"
        >
          <ArrowLeft className="size-3.5" />
        </button>
        <button
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
          onClick={() => onForward(surfaceId)}
          title="Forward"
        >
          <ArrowRight className="size-3.5" />
        </button>
        <button
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
          onClick={() => {
            // Recreate webview to reload
            if (currentUrl) createWebview(currentUrl);
            onReload(surfaceId);
          }}
          title="Reload"
        >
          <RotateCw className="size-3.5" />
        </button>
        <div className="flex-1 flex items-center gap-2 rounded-md border border-border bg-muted/40 px-2 py-1">
          <Globe2 className="size-3 text-muted-foreground" />
          <input
            value={urlInput}
            onChange={(e) => setUrlInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleNavigate();
            }}
            className="w-full bg-transparent text-[11px] text-foreground outline-none"
            placeholder="Enter URL"
          />
        </div>
        {currentUrl && (
          <button
            className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
            onClick={() => {
              window.open(currentUrl, "_blank");
            }}
            title="Open in external browser"
          >
            <ExternalLink className="size-3.5" />
          </button>
        )}
      </div>

      {/* Webview container */}
      <div ref={containerRef} className="relative flex-1 min-h-0 bg-background">
        {!currentUrl && !sessionLoading && (
          <div className="flex h-full items-center justify-center">
            <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-border/60 bg-card/60 px-6 py-5 text-center">
              <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground">
                <Globe2 className="size-4" />
              </div>
              <div className="space-y-1">
                <div className="text-[12px] font-medium text-foreground">No page loaded</div>
                <div className="text-[10px] text-muted-foreground">
                  Enter a URL to open the website.
                </div>
              </div>
            </div>
          </div>
        )}
        {webviewError && (
          <div className="absolute inset-0 flex items-center justify-center bg-background z-10">
            <div className="flex flex-col items-center gap-2 text-center px-6">
              <Globe2 className="size-6 text-destructive/60" />
              <div className="text-[11px] text-destructive">{webviewError}</div>
              <button
                onClick={() => currentUrl && createWebview(currentUrl)}
                className="text-[10px] text-foreground/60 hover:text-foreground underline"
              >
                Retry
              </button>
            </div>
          </div>
        )}
        {(sessionLoading || (currentUrl && !webviewReady && !webviewError)) && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-background/60 z-10">
            <div className="flex flex-col items-center gap-2 text-[11px] text-foreground/80">
              <div className="flex items-center gap-2">
                <span className="h-3 w-3 animate-spin rounded-full border border-muted-foreground/40 border-t-foreground/80" />
                <Shimmer className="text-[11px] text-foreground/80">
                  {sessionLoading ? "Initializing browser..." : "Loading page..."}
                </Shimmer>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default BrowserView;
