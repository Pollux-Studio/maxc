"use client"

import { motion, AnimatePresence } from "framer-motion"
import { useState, useEffect, useRef } from "react"
import { useInView } from "framer-motion"
import type { TechSection } from "@/lib/sections-data"

const shadow = "rgba(14, 63, 126, 0.04) 0px 0px 0px 1px, rgba(42, 51, 69, 0.04) 0px 1px 1px -0.5px, rgba(42, 51, 70, 0.04) 0px 3px 3px -1.5px, rgba(42, 51, 70, 0.04) 0px 6px 6px -3px, rgba(14, 63, 126, 0.04) 0px 12px 12px -6px, rgba(14, 63, 126, 0.04) 0px 24px 24px -12px"

/*
  SECTION 03: BROWSER AUTOMATION
  Style: Live browser dashboard. Each session is a real automation scenario
  with animated action logs, status indicators, and a rich inspector.
*/

interface BrowserSession {
  id: string
  name: string
  url: string
  status: "running" | "complete" | "capturing"
  engine: "Chromium" | "WebView2"
  tabs: number
  actions: number
  method: string
  log: string[]
}

const sessions: BrowserSession[] = [
  {
    id: "bs_001",
    name: "Navigation",
    url: "https://app.example.com/dashboard",
    status: "running",
    engine: "Chromium",
    tabs: 3,
    actions: 12,
    method: "browser.goto",
    log: [
      "> browser.create --engine chromium",
      "  Session bs_001 created [CDP:active]",
      "> browser.goto https://app.example.com",
      "  Navigation: 200 OK (142ms)",
      "> browser.goto /dashboard",
      "  Navigation: 200 OK (89ms)",
      "> browser.tab-open /settings",
      "  Tab 2 opened [load: complete]",
      "> browser.tab-open /analytics",
      "  Tab 3 opened [load: complete]",
    ],
  },
  {
    id: "bs_002",
    name: "DOM Scrape",
    url: "https://docs.example.com/api",
    status: "complete",
    engine: "Chromium",
    tabs: 1,
    actions: 8,
    method: "browser.evaluate",
    log: [
      "> browser.goto https://docs.example.com/api",
      "  Navigation: 200 OK (203ms)",
      "> browser.wait --selector '.api-list'",
      "  Selector found (34ms)",
      "> browser.evaluate 'document.querySelectorAll",
      "  (\".endpoint\").length'",
      "  Result: 47",
      "> browser.evaluate 'JSON.stringify(",
      "  [...document.querySelectorAll(\".endpoint\")]",
      "  .map(e => e.textContent))'",
      "  Result: [\"GET /users\", \"POST /auth\", ...]",
    ],
  },
  {
    id: "bs_003",
    name: "Screenshot",
    url: "https://staging.example.com",
    status: "capturing",
    engine: "Chromium",
    tabs: 1,
    actions: 15,
    method: "browser.screenshot",
    log: [
      "> browser.goto https://staging.example.com",
      "  Navigation: 200 OK (312ms)",
      "> browser.wait --load-state networkidle",
      "  Network idle reached (1.2s)",
      "> browser.screenshot --full-page",
      "  Capturing viewport: 1920x1080...",
      "  Encoding PNG (2.1MB)...",
      "  Saved: ./captures/staging_001.png",
      "> browser.screenshot --selector '#hero'",
      "  Clipping to element: 1920x640",
      "  Saved: ./captures/hero_001.png",
    ],
  },
  {
    id: "bs_004",
    name: "Form Fill",
    url: "https://app.example.com/signup",
    status: "running",
    engine: "WebView2",
    tabs: 1,
    actions: 5,
    method: "browser.type",
    log: [
      "> browser.create --engine webview2",
      "  Session bs_004 [WebView2 fallback]",
      "> browser.goto https://app.example.com/signup",
      "  Navigation: 200 OK (178ms)",
      "> browser.type '#email' 'test@maxc.dev'",
      "  Typed 13 chars into #email",
      "> browser.type '#password' '••••••••'",
      "  Typed 8 chars into #password",
      "> browser.click '#submit'",
      "  Click dispatched at (640, 420)",
    ],
  },
  {
    id: "bs_005",
    name: "Cookie Mgmt",
    url: "https://app.example.com",
    status: "complete",
    engine: "Chromium",
    tabs: 2,
    actions: 22,
    method: "browser.cookies",
    log: [
      "> browser.goto https://app.example.com",
      "  Navigation: 200 OK (156ms)",
      "> browser.cookies --list",
      "  Found 8 cookies for .example.com",
      "> browser.cookies --get 'session_token'",
      "  Value: eyJhbGciOi... (exp: 3600s)",
      "> browser.storage --local --list",
      "  12 keys in localStorage",
      "> browser.cookies --clear --domain .example.com",
      "  Cleared 8 cookies [OK]",
    ],
  },
]

function SessionCard({ session, index, isSelected, onSelect }: {
  session: BrowserSession
  index: number
  isSelected: boolean
  onSelect: () => void
}) {
  const statusColor = session.status === "running"
    ? "bg-foreground"
    : session.status === "capturing"
    ? "bg-foreground"
    : "bg-muted-foreground"

  return (
    <motion.button
      initial={{ opacity: 0, y: 30 }}
      whileInView={{ opacity: 1, y: 0 }}
      viewport={{ once: true }}
      transition={{ delay: 0.15 + index * 0.1 }}
      onClick={onSelect}
      className={`group relative flex w-52 flex-shrink-0 flex-col border p-4 text-left font-mono transition-all duration-300 focus-visible:ring-2 focus-visible:ring-foreground focus-visible:outline-none ${
        isSelected
          ? "border-foreground bg-foreground text-background"
          : "border-border bg-background text-foreground hover:border-foreground"
      }`}
      style={{ boxShadow: shadow }}
    >
      {/* Status + name */}
      <div className="flex items-center gap-2">
        {session.status !== "complete" ? (
          <motion.div
            className={`h-1.5 w-1.5 ${isSelected ? "bg-background" : statusColor}`}
            animate={{ opacity: [1, 0.3, 1] }}
            transition={{ repeat: Infinity, duration: 1.2 }}
          />
        ) : (
          <div className={`h-1.5 w-1.5 ${isSelected ? "bg-background/50" : "bg-muted-foreground/50"}`} />
        )}
        <span className={`text-[10px] uppercase tracking-wider ${isSelected ? "text-background/60" : "text-muted-foreground"}`}>
          {session.status}
        </span>
      </div>

      {/* Session name */}
      <span className={`mt-2 text-sm font-bold ${isSelected ? "text-background" : "text-foreground"}`}>
        {session.name}
      </span>

      {/* URL preview */}
      <span className={`mt-1 truncate text-[10px] ${isSelected ? "text-background/50" : "text-muted-foreground"}`}>
        {session.url}
      </span>

      {/* Stats row */}
      <div className="mt-3 flex gap-4 text-[10px]">
        <div className="flex flex-col">
          <span className={isSelected ? "text-background/50" : "text-muted-foreground"}>Tabs</span>
          <span className="font-bold">{session.tabs}</span>
        </div>
        <div className="flex flex-col">
          <span className={isSelected ? "text-background/50" : "text-muted-foreground"}>Actions</span>
          <span className="font-bold">{session.actions}</span>
        </div>
        <div className="flex flex-col">
          <span className={isSelected ? "text-background/50" : "text-muted-foreground"}>Engine</span>
          <span className="font-bold">{session.engine === "Chromium" ? "CDP" : "WV2"}</span>
        </div>
      </div>

      {/* Chain connector */}
      {index < sessions.length - 1 && (
        <div className="absolute -right-6 top-1/2 hidden -translate-y-1/2 items-center md:flex" aria-hidden="true">
          <div className="h-px w-6 bg-border" />
          <div className="h-0 w-0 border-y-[3px] border-l-[5px] border-y-transparent border-l-border" />
        </div>
      )}
    </motion.button>
  )
}

function ActionLog({ session }: { session: BrowserSession }) {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: false })
  const [visibleLines, setVisibleLines] = useState(0)

  useEffect(() => {
    setVisibleLines(0)
    if (!isInView) return
    let i = 0
    const interval = setInterval(() => {
      if (i < session.log.length) {
        i++
        setVisibleLines(i)
      } else {
        clearInterval(interval)
      }
    }, 120)
    return () => clearInterval(interval)
  }, [isInView, session.id, session.log.length])

  return (
    <div ref={ref} className="flex flex-col gap-0.5">
      {session.log.slice(0, visibleLines).map((line, i) => (
        <motion.div
          key={`${session.id}-${i}`}
          initial={{ opacity: 0, x: -5 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.15 }}
          className={`font-mono text-[11px] leading-relaxed ${
            line.startsWith(">")
              ? "text-foreground font-bold"
              : "text-muted-foreground"
          }`}
        >
          {line}
        </motion.div>
      ))}
      {visibleLines < session.log.length && (
        <span className="animate-blink inline-block font-mono text-xs text-foreground">{"_"}</span>
      )}
    </div>
  )
}

export function SectionLedger({ section }: { section: TechSection }) {
  const [selected, setSelected] = useState(0)

  return (
    <div className="py-20 lg:py-32">
      {/* Full-width top bar with number + title */}
      <div className="mx-auto max-w-7xl px-4 lg:px-8">
        <motion.div
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          className="flex items-end gap-6"
        >
          <span className="font-pixel-line text-7xl font-bold leading-none text-foreground/[0.08] md:text-9xl">
            {section.number}
          </span>
          <div className="pb-2">
            <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">{section.subtitle}</span>
            <h2 className="font-pixel-line text-3xl font-bold text-foreground md:text-5xl">
              {section.title}
            </h2>
          </div>
        </motion.div>

        {/* Description in columns */}
        <motion.p
          initial={{ opacity: 0, y: 15 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ delay: 0.1 }}
          className="mt-8 max-w-4xl font-mono text-sm leading-relaxed text-muted-foreground md:columns-2 md:gap-12"
        >
          {section.description} Each browser session runs in isolation with full
          CDP control. Navigate pages, manipulate DOM, manage cookies, and
          capture screenshots programmatically.
        </motion.p>
      </div>

      {/* Horizontal scrolling session cards */}
      <div className="mt-12 overflow-x-auto">
        <div className="mx-auto flex w-max items-center gap-6 px-8 pb-4">
          {sessions.map((session, i) => (
            <SessionCard
              key={session.id}
              session={session}
              index={i}
              isSelected={selected === i}
              onSelect={() => setSelected(i)}
            />
          ))}
        </div>
      </div>

      {/* Inspector + action log */}
      <div className="mx-auto mt-8 max-w-7xl px-4 lg:px-8">
        <div className="grid gap-4 md:grid-cols-2">
          {/* Live action log */}
          <AnimatePresence mode="wait">
            <motion.div
              key={selected}
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.25 }}
              className="border border-border"
              style={{ boxShadow: shadow }}
            >
              {/* Log header */}
              <div className="flex items-center justify-between border-b border-border px-4 py-2">
                <div className="flex items-center gap-2">
                  {sessions[selected].status !== "complete" ? (
                    <motion.div
                      className="h-1.5 w-1.5 bg-foreground"
                      animate={{ opacity: [1, 0.3, 1] }}
                      transition={{ repeat: Infinity, duration: 1 }}
                    />
                  ) : (
                    <div className="h-1.5 w-1.5 bg-muted-foreground/50" />
                  )}
                  <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
                    Action Log — {sessions[selected].name}
                  </span>
                </div>
                <span className="font-mono text-[10px] text-muted-foreground">
                  {sessions[selected].id}
                </span>
              </div>

              {/* URL bar */}
              <div className="flex items-center gap-2 border-b border-border bg-secondary/30 px-4 py-1.5">
                <div className="flex gap-1">
                  <div className="h-2 w-2 border border-border bg-muted-foreground/20" />
                  <div className="h-2 w-2 border border-border bg-muted-foreground/20" />
                  <div className="h-2 w-2 border border-border bg-muted-foreground/20" />
                </div>
                <div className="flex-1 border border-border bg-background px-2 py-0.5">
                  <span className="font-mono text-[10px] text-muted-foreground">
                    {sessions[selected].url}
                  </span>
                </div>
              </div>

              {/* Log body */}
              <div className="h-56 overflow-y-auto p-4">
                <ActionLog session={sessions[selected]} />
              </div>
            </motion.div>
          </AnimatePresence>

          {/* Right: Session inspector + specs */}
          <div className="flex flex-col gap-4">
            {/* Session inspector */}
            <AnimatePresence mode="wait">
              <motion.div
                key={`inspector-${selected}`}
                initial={{ opacity: 0, x: 10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                transition={{ duration: 0.25 }}
                className="border border-border p-4"
                style={{ boxShadow: shadow }}
              >
                <div className="flex items-center gap-2">
                  <div className="h-1.5 w-1.5 bg-foreground" />
                  <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">Session Inspector</span>
                </div>
                <div className="mt-3 grid grid-cols-2 gap-x-6 gap-y-2 font-mono text-xs">
                  <div className="flex justify-between border-b border-border pb-1.5">
                    <span className="text-muted-foreground">ID</span>
                    <span className="font-bold text-foreground">{sessions[selected].id}</span>
                  </div>
                  <div className="flex justify-between border-b border-border pb-1.5">
                    <span className="text-muted-foreground">Engine</span>
                    <span className="font-bold text-foreground">{sessions[selected].engine}</span>
                  </div>
                  <div className="flex justify-between border-b border-border pb-1.5">
                    <span className="text-muted-foreground">Method</span>
                    <span className="font-bold text-foreground">{sessions[selected].method}</span>
                  </div>
                  <div className="flex justify-between border-b border-border pb-1.5">
                    <span className="text-muted-foreground">Tabs</span>
                    <span className="font-bold text-foreground">{sessions[selected].tabs}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Actions</span>
                    <span className="font-bold text-foreground">{sessions[selected].actions}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Status</span>
                    <span className="font-bold text-foreground uppercase">{sessions[selected].status}</span>
                  </div>
                </div>
              </motion.div>
            </AnimatePresence>

            {/* Specs */}
            <div className="grid grid-cols-2 gap-3">
              {section.specs.map((spec, i) => (
                <motion.div
                  key={spec.label}
                  initial={{ opacity: 0, x: 20 }}
                  whileInView={{ opacity: 1, x: 0 }}
                  viewport={{ once: true }}
                  transition={{ delay: 0.2 + i * 0.1 }}
                  className="flex flex-col items-center justify-center border border-border p-3 text-center"
                  style={{ boxShadow: shadow }}
                >
                  <span className="font-mono text-sm font-bold text-foreground">{spec.value}</span>
                  <span className="mt-0.5 font-mono text-[9px] uppercase tracking-wider text-muted-foreground">{spec.label}</span>
                </motion.div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
