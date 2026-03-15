import { useMemo } from "react";
import { Bell, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export type NotificationItem = {
  notification_id: string;
  workspace_id?: string | null;
  title: string;
  body: string;
  level: string;
  source: string;
  created_at_ms: number;
  read: boolean;
};

type NotificationPanelProps = {
  open: boolean;
  notifications: NotificationItem[];
  onClose: () => void;
  onClearAll: () => void;
  onClearNotification: (notificationId: string) => void;
};

function formatTime(ts: number) {
  try {
    return new Date(ts).toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "";
  }
}

export function NotificationPanel({
  open,
  notifications,
  onClose,
  onClearAll,
  onClearNotification,
}: NotificationPanelProps) {
  const sorted = useMemo(() => {
    return [...notifications].sort((a, b) => b.created_at_ms - a.created_at_ms);
  }, [notifications]);

  return (
    <>
      <div
        className={cn(
          "fixed inset-0 z-40 bg-black/40 transition-opacity duration-200",
          open ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        onClick={onClose}
      />
      <div
        className={cn(
          "fixed right-0 top-0 z-50 flex h-full w-[360px] flex-col border-l bg-card shadow-xl transition-transform duration-200 ease-out",
          open ? "translate-x-0" : "translate-x-full",
        )}
      >
        <div className="flex items-center justify-between border-b px-5 py-4">
          <div className="flex items-center gap-2">
            <Bell className="size-4 text-primary" />
            <h2 className="text-sm font-semibold">Notifications</h2>
          </div>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon-xs"
              onClick={onClearAll}
              title="Clear all"
            >
              <Trash2 className="size-3.5" />
            </Button>
            <Button variant="ghost" size="icon-xs" onClick={onClose}>
              <X className="size-4" />
            </Button>
          </div>
        </div>

        <div className="flex-1 overflow-auto px-5 py-4 space-y-3">
          {sorted.length === 0 && (
            <div className="rounded-md border border-dashed px-4 py-6 text-center text-[12px] text-muted-foreground">
              No notifications yet.
            </div>
          )}
          {sorted.map((n) => {
            const levelClass =
              n.level === "success"
                ? "bg-emerald-500/10 text-emerald-400"
                : n.level === "warning"
                  ? "bg-amber-500/10 text-amber-400"
                  : n.level === "error"
                    ? "bg-rose-500/10 text-rose-400"
                    : "bg-blue-500/10 text-blue-400";
            return (
              <div
                key={n.notification_id}
                className={cn(
                  "rounded-lg border px-3 py-2 text-xs shadow-sm transition-colors",
                  n.read ? "border-border/60 bg-muted/30" : "border-primary/20 bg-background",
                )}
              >
                <div className="flex items-start gap-2">
                  <span className={cn("mt-0.5 h-2 w-2 rounded-full", levelClass)} />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-semibold truncate">{n.title}</span>
                      <span className="text-[10px] text-muted-foreground">
                        {formatTime(n.created_at_ms)}
                      </span>
                    </div>
                    {n.body && (
                      <div className="mt-1 max-h-10 overflow-hidden text-[11px] text-muted-foreground">
                        {n.body}
                      </div>
                    )}
                    <div className="mt-2 flex items-center gap-2 text-[10px] text-muted-foreground">
                      {n.workspace_id && (
                        <span className="rounded-full bg-muted px-2 py-0.5">
                          {n.workspace_id}
                        </span>
                      )}
                      <span className="rounded-full bg-muted px-2 py-0.5">
                        {n.source}
                      </span>
                      <span className="rounded-full bg-muted px-2 py-0.5">
                        {n.level}
                      </span>
                    </div>
                  </div>
                  <Button
                    variant="ghost"
                    size="icon-xs"
                    onClick={() => onClearNotification(n.notification_id)}
                    title="Mark as read"
                  >
                    <X className="size-3.5" />
                  </Button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </>
  );
}
