import { useEffect, useRef, useState } from "react";
import { ArrowLeft, ArrowRight, Globe2, RotateCw } from "lucide-react";
import { Shimmer } from "@/components/ai-elements/shimmer";

export type BrowserViewProps = {
  surfaceId: string;
  browserSessionId: string;
  workspaceId: string;
  tabId: string | null;
  focused: boolean;
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
}: BrowserViewProps) {
  const [urlInput, setUrlInput] = useState(currentUrl);
  const [iframeKey, setIframeKey] = useState(0);
  const iframeRef = useRef<HTMLIFrameElement | null>(null);

  useEffect(() => {
    setUrlInput(currentUrl);
  }, [currentUrl]);

  return (
    <div className="flex h-full flex-col bg-background">
      <div className="flex items-center gap-2 border-b border-border bg-card px-2 py-1">
        <button
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
          onClick={() => {
            try {
              iframeRef.current?.contentWindow?.history.back();
            } catch { /* ignore */ }
            onBack(surfaceId);
          }}
          title="Back"
        >
          <ArrowLeft className="size-3.5" />
        </button>
        <button
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
          onClick={() => {
            try {
              iframeRef.current?.contentWindow?.history.forward();
            } catch { /* ignore */ }
            onForward(surfaceId);
          }}
          title="Forward"
        >
          <ArrowRight className="size-3.5" />
        </button>
        <button
          className="rounded p-1 text-muted-foreground hover:text-foreground hover:bg-muted/60"
          onClick={() => {
            try {
              iframeRef.current?.contentWindow?.location.reload();
            } catch {
              setIframeKey((k) => k + 1);
            }
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
              if (e.key === "Enter") onNavigate(surfaceId, urlInput);
            }}
            className="w-full bg-transparent text-[11px] text-foreground outline-none"
            placeholder="Enter URL"
          />
        </div>
      </div>

      <div className="relative flex-1 min-h-0 bg-background">
        {currentUrl ? (
          <iframe
            key={iframeKey}
            ref={iframeRef}
            src={currentUrl}
            title="Browser"
            className="h-full w-full border-0 bg-background"
          />
        ) : (
          <div className="flex h-full items-center justify-center">
            <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed border-border/60 bg-card/60 px-6 py-5 text-center">
              <div className="flex size-9 items-center justify-center rounded-md bg-muted text-muted-foreground">
                <Globe2 className="size-4" />
              </div>
              <div className="space-y-1">
                <div className="text-[12px] font-medium text-foreground">No page loaded</div>
                <div className="text-[10px] text-muted-foreground">
                  {sessionLoading ? "Starting browser runtime…" : "Enter a URL to open the website."}
                </div>
              </div>
            </div>
          </div>
        )}
        {sessionLoading && (
          <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-background/60">
            <div className="flex flex-col items-center gap-2 text-[11px] text-foreground/80">
              <div className="flex items-center gap-2">
                <span className="h-3 w-3 animate-spin rounded-full border border-muted-foreground/40 border-t-foreground/80" />
                <Shimmer className="text-[11px] text-foreground/80">
                  Initializing browser…
                </Shimmer>
              </div>
              <Shimmer className="text-[10px] text-muted-foreground">
                Negotiating session… loading automation…
              </Shimmer>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default BrowserView;
