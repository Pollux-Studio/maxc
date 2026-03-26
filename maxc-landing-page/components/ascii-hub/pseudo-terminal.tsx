"use client"

import { useState, useRef, useEffect, type KeyboardEvent } from "react"
import { motion } from "framer-motion"

const DOWNLOAD_URL = "https://github.com/Pollux-Studio/maxc/releases/download/stable/maxc_0.2.0_x64-setup.exe"

const COMMANDS: Record<string, string[]> = {
  help: [
    "Available commands:",
    "  help       - Show this message",
    "  sections   - List all platform modules",
    "  inspect    - Inspect workspace status",
    "  about      - About maxc",
    "  stack      - Show tech stack",
    "  download   - Download maxc for Windows",
    "  clear      - Clear terminal",
    "  ascii      - Show ASCII art",
    "  maxc       - ...",
  ],
  sections: [
    "01  Terminal Engine",
    "02  RPC & API",
    "03  Browser Automation",
    "04  CLI & Commands",
    "05  Workspace Architecture",
    "06  Agent System",
    "07  Storage & Recovery",
    "08  Security & Diagnostics",
  ],
  inspect: [
    "Workspace: maxc-dev",
    "Version: 0.2.0",
    "Modules: 8 loaded",
    "RPC Methods: 52 registered",
    "Status: OPERATIONAL",
  ],
  about: [
    "maxc v0.2.0",
    "",
    "Open-source programmable developer workspace",
    "that unifies terminals, browser automation,",
    "and agent-driven task orchestration.",
    "",
    "Built with Rust, Tauri v2, React 19,",
    "and a love for the terminal aesthetic.",
  ],
  stack: [
    "Backend:   Rust + Tokio async runtime",
    "Desktop:   Tauri v2 (WebView2)",
    "Frontend:  React 19 + TypeScript",
    "Terminal:  xterm.js + ConPTY",
    "Browser:   Chromium CDP protocol",
  ],
  ascii: [
    "",
    "  ███╗   ███╗ █████╗ ██╗  ██╗ ██████╗",
    "  ████╗ ████║██╔══██╗╚██╗██╔╝██╔════╝",
    "  ██╔████╔██║███████║ ╚███╔╝ ██║     ",
    "  ██║╚██╔╝██║██╔══██║ ██╔██╗ ██║     ",
    "  ██║ ╚═╝ ██║██║  ██║██╔╝ ██╗╚██████╗",
    "  ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝",
    "",
  ],
  maxc: [
    "",
    "  ███╗   ███╗ █████╗ ██╗  ██╗ ██████╗",
    "  ████╗ ████║██╔══██╗╚██╗██╔╝██╔════╝",
    "  ██╔████╔██║███████║ ╚███╔╝ ██║     ",
    "  ██║╚██╔╝██║██╔══██║ ██╔██╗ ██║     ",
    "  ██║ ╚═╝ ██║██║  ██║██╔╝ ██╗╚██████╗",
    "  ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝",
    "",
    "  Code. Automate. Orchestrate.",
    "",
  ],
}

interface TerminalLine {
  type: "input" | "output" | "maxc" | "download"
  content: string
}

export function PseudoTerminal() {
  const [lines, setLines] = useState<TerminalLine[]>([
    { type: "output", content: 'Welcome to maxc terminal v0.2.0' },
    { type: "output", content: 'Type "help" for available commands.' },
    { type: "output", content: "" },
    { type: "input", content: "$ download" },
    { type: "output", content: "" },
    { type: "output", content: "  ┌────────────────────────────────────────┐" },
    { type: "output", content: "  │  maxc v0.2.0 — Windows Installer       │" },
    { type: "output", content: "  │                                        │" },
    { type: "output", content: "  │  Platform:  Windows 10/11 (x86_64)     │" },
    { type: "output", content: "  │  Package:   NSIS setup (no admin)      │" },
    { type: "output", content: "  │  Size:      ~8 MB                      │" },
    { type: "output", content: "  │  License:   Apache-2.0 (open-source)   │" },
    { type: "output", content: "  │                                        │" },
    { type: "download", content: "│  > [DOWNLOAD FOR WINDOWS]              │" },
    { type: "output", content: "  │                                        │" },
    { type: "output", content: "  └────────────────────────────────────────┘" },
    { type: "output", content: "" },
  ])
  const [input, setInput] = useState("")
  const scrollRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [lines])

  const processCommand = (cmd: string) => {
    const trimmed = cmd.trim().toLowerCase()
    const baseLines: TerminalLine[] = [
      ...lines,
      { type: "input", content: `$ ${cmd}` },
    ]

    if (trimmed === "clear") {
      setLines([])
      setInput("")
      return
    }

    if (trimmed === "maxc") {
      setLines([...baseLines, { type: "output", content: "" }])
      setInput("")
      const maxcLines = COMMANDS["maxc"]
      maxcLines.forEach((line, i) => {
        setTimeout(() => {
          setLines((prev) => [...prev, { type: "maxc", content: line }])
        }, i * 80)
      })
      return
    }

    if (trimmed === "download") {
      const dlLines: TerminalLine[] = [
        ...baseLines,
        { type: "output", content: "" },
        { type: "output", content: "  ┌────────────────────────────────────────┐" },
        { type: "output", content: "  │  maxc v0.2.0 — Windows Installer       │" },
        { type: "output", content: "  │                                        │" },
        { type: "output", content: "  │  Platform:  Windows 10/11 (x86_64)     │" },
        { type: "output", content: "  │  Package:   NSIS setup (no admin)      │" },
        { type: "output", content: "  │  Size:      ~8 MB                      │" },
        { type: "output", content: "  │  License:   Apache-2.0 (open-source)   │" },
        { type: "output", content: "  │                                        │" },
        { type: "download", content: "│  > [DOWNLOAD FOR WINDOWS]              │" },
        { type: "output", content: "  │                                        │" },
        { type: "output", content: "  └────────────────────────────────────────┘" },
        { type: "output", content: "" },
      ]
      setLines(dlLines)
      setInput("")
      return
    }

    const newLines: TerminalLine[] = [...baseLines]
    const response = COMMANDS[trimmed]
    if (response) {
      response.forEach((line) => {
        newLines.push({ type: "output", content: line })
      })
    } else if (trimmed === "") {
      // do nothing
    } else {
      newLines.push({ type: "output", content: `command not found: ${trimmed}` })
      newLines.push({ type: "output", content: 'Type "help" for available commands.' })
    }

    newLines.push({ type: "output", content: "" })
    setLines(newLines)
    setInput("")
  }

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      processCommand(input)
    }
  }

  return (
    <motion.section
      initial={{ opacity: 0, y: 20 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ duration: 0.5 }}
      className="mx-auto max-w-7xl px-4 py-16 lg:px-8 lg:py-24"
    >
      <div className="mb-8 flex flex-col gap-4">
        <div className="flex items-center gap-4">
          <span className="font-mono text-sm text-muted-foreground">{">"}</span>
          <div className="h-[1px] w-12 bg-border" />
          <span className="font-mono text-xs uppercase tracking-widest text-muted-foreground">
            Interactive
          </span>
        </div>
        <h2 className="font-pixel-line text-3xl font-bold tracking-tight text-foreground md:text-5xl">
          Terminal
        </h2>
        <p className="max-w-prose font-mono text-sm leading-relaxed text-muted-foreground">
          Explore the platform. Type commands to interact with maxc.
        </p>
      </div>

      <div
        className="border border-border"
        onClick={() => inputRef.current?.focus()}
        role="application"
        aria-label="Interactive pseudo-terminal"
      >
        {/* Terminal header */}
        <div className="flex items-center gap-2 border-b border-border px-4 py-2.5">
          <div className="h-2.5 w-2.5 bg-foreground" />
          <div className="h-2.5 w-2.5 bg-muted-foreground/50" />
          <div className="h-2.5 w-2.5 bg-muted-foreground/30" />
          <span className="ml-2 font-mono text-xs text-muted-foreground">
            maxc ~ interactive
          </span>
        </div>

        {/* Terminal body */}
        <div
          ref={scrollRef}
          className="h-80 overflow-y-auto bg-secondary/20 p-4"
        >
          {lines.map((line, i) => (
            <div
              key={i}
              className={`font-mono text-xs leading-relaxed ${
                line.type === "input"
                  ? "text-foreground"
                  : line.type === "maxc"
                  ? "text-foreground brightness-125"
                  : line.type === "download"
                  ? "text-foreground"
                  : "text-muted-foreground"
              }`}
            >
              {line.type === "download" ? (
                <a
                  href={DOWNLOAD_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-block animate-pulse font-bold text-foreground underline underline-offset-2 transition-opacity hover:opacity-70"
                  onClick={(e) => e.stopPropagation()}
                >
                  {line.content}
                </a>
              ) : (
                line.content || "\u00A0"
              )}
            </div>
          ))}

          {/* Input line */}
          <div className="relative flex items-center font-mono text-xs text-foreground">
            <span className="mr-1">{"$"}</span>
            <span>{input}</span>
            <span className="animate-blink">{"█"}</span>
            <input
              ref={inputRef}
              type="text"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              className="absolute inset-0 h-full w-full cursor-default border-none bg-transparent opacity-0 outline-none"
              aria-label="Terminal input"
              autoComplete="off"
              autoCorrect="off"
              spellCheck={false}
            />
          </div>
        </div>
      </div>
    </motion.section>
  )
}
