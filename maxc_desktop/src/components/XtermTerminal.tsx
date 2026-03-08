import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

export type XtermHandle = {
  /** Write raw output (including ANSI sequences) to the terminal */
  write(data: string): void;
  /** Clear the terminal screen */
  clear(): void;
  /** Focus the terminal */
  focus(): void;
  /** Fit the terminal to its container */
  fit(): void;
};

export type XtermTerminalProps = {
  /** Called when user types in the terminal - raw data string */
  onData: (data: string) => void;
  /** Called when the terminal resizes - new cols/rows */
  onResize?: (cols: number, rows: number) => void;
  /** Whether this terminal pane is currently focused / visible */
  focused?: boolean;
};

const THEME = {
  background: "#0c0c0c",
  foreground: "#cccccc",
  cursor: "#cccccc",
  selectionBackground: "#264f78",
  black: "#0c0c0c",
  red: "#c50f1f",
  green: "#13a10e",
  yellow: "#c19c00",
  blue: "#0037da",
  magenta: "#881798",
  cyan: "#3a96dd",
  white: "#cccccc",
  brightBlack: "#767676",
  brightRed: "#e74856",
  brightGreen: "#16c60c",
  brightYellow: "#f9f1a5",
  brightBlue: "#3b78ff",
  brightMagenta: "#b4009e",
  brightCyan: "#61d6d6",
  brightWhite: "#f2f2f2",
};

/**
 * xterm.js terminal component.
 *
 * The parent pushes output via the imperative ref handle:
 *   `ref.current.write(data)`
 *
 * Keyboard input flows out via the `onData` callback.
 */
export const XtermTerminal = forwardRef<XtermHandle, XtermTerminalProps>(
  function XtermTerminal({ onData, onResize, focused }, ref) {
    const containerRef = useRef<HTMLDivElement>(null);
    const termRef = useRef<Terminal | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);

    // Expose imperative handle to parent
    useImperativeHandle(ref, () => ({
      write(data: string) {
        termRef.current?.write(data);
      },
      clear() {
        termRef.current?.clear();
      },
      focus() {
        termRef.current?.focus();
      },
      fit() {
        try { fitAddonRef.current?.fit(); } catch { /* ignore */ }
      },
    }));

    useEffect(() => {
      const el = containerRef.current;
      if (!el) return;

      const term = new Terminal({
        fontFamily: "'JetBrains Mono Variable', 'Cascadia Code', 'Consolas', monospace",
        fontSize: 13,
        lineHeight: 1.25,
        cursorBlink: true,
        cursorStyle: "bar",
        allowProposedApi: true,
        scrollback: 10_000,
        theme: THEME,
      });

      const fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.loadAddon(new WebLinksAddon());

      term.open(el);

      // Initial fit after layout settles
      requestAnimationFrame(() => {
        try { fitAddon.fit(); } catch { /* container not sized yet */ }
      });

      term.onData((data) => onData(data));
      term.onResize(({ cols, rows }) => onResize?.(cols, rows));

      termRef.current = term;
      fitAddonRef.current = fitAddon;

      // Re-fit on container resize
      const ro = new ResizeObserver(() => {
        requestAnimationFrame(() => {
          try { fitAddon.fit(); } catch { /* ignore */ }
        });
      });
      ro.observe(el);

      return () => {
        ro.disconnect();
        term.dispose();
        termRef.current = null;
        fitAddonRef.current = null;
      };
      // onData/onResize are stable callbacks from the parent; we only
      // mount/unmount when the component mounts/unmounts (keyed by terminal id
      // in the parent).
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);

    // Auto-focus when the pane becomes focused
    useEffect(() => {
      if (focused) termRef.current?.focus();
    }, [focused]);

    return (
      <div
        ref={containerRef}
        className="h-full w-full min-h-0"
        style={{ minHeight: 0 }}
      />
    );
  },
);
