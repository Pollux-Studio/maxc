"use client"

import { motion, useInView } from "framer-motion"
import { useRef, useState, useEffect } from "react"
import type { TechSection } from "@/lib/sections-data"

const shadow = "rgba(14, 63, 126, 0.04) 0px 0px 0px 1px, rgba(42, 51, 69, 0.04) 0px 1px 1px -0.5px, rgba(42, 51, 70, 0.04) 0px 3px 3px -1.5px, rgba(42, 51, 70, 0.04) 0px 6px 6px -3px, rgba(14, 63, 126, 0.04) 0px 12px 12px -6px, rgba(14, 63, 126, 0.04) 0px 24px 24px -12px"

/*
  SECTION 07: STORAGE & RECOVERY
  Style: System activity monitor. Animated horizontal lanes
  representing event streams over time, like a profiler timeline view.
  Real-time feeling with animated progress bars and status updates.
*/

interface ThreadLane {
  id: string
  label: string
  segments: { start: number; end: number; type: "work" | "wait" | "blocked" }[]
}

const threadNames = ["terminal", "browser", "agent", "workspace", "snapshot", "replay"]

type SegType = "work" | "wait" | "blocked"

function generateLanes(): ThreadLane[] {
  return threadNames.map((label, i) => {
    const segments: ThreadLane["segments"] = []
    let cursor = Math.floor(Math.random() * 10)
    const types: SegType[] = ["work", "wait", "blocked"]
    while (cursor < 100) {
      const type: SegType = i === 5 ? "work" : types[Math.floor(Math.random() * (i === 4 ? 2 : 3))]
      const len = Math.floor(Math.random() * 30) + 5
      const end = Math.min(100, cursor + len)
      segments.push({ start: cursor, end, type })
      cursor = end + Math.floor(Math.random() * 5)
    }
    return { id: `t${i + 1}`, label, segments }
  })
}

function TimelineView() {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true })
  const [lanes, setLanes] = useState<ThreadLane[]>(generateLanes)
  const [tick, setTick] = useState(0)

  useEffect(() => {
    if (!isInView) return
    const interval = setInterval(() => {
      setLanes(generateLanes())
      setTick((t) => t + 1)
    }, 2000)
    return () => clearInterval(interval)
  }, [isInView])

  return (
    <div ref={ref} className="overflow-hidden border border-border" style={{ boxShadow: shadow }}>
      <div className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-2">
          <motion.div
            className="h-2 w-2 bg-foreground"
            animate={isInView ? { opacity: [1, 0.3, 1] } : {}}
            transition={{ repeat: Infinity, duration: 1 }}
          />
          <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
            Event Stream Monitor
          </span>
        </div>
        <span className="font-mono text-[10px] text-muted-foreground">0 — 100 events</span>
      </div>

      {/* Time scale */}
      <div className="flex border-b border-border px-4 py-1">
        <div className="w-20 flex-shrink-0" />
        <div className="flex flex-1 justify-between font-mono text-[9px] text-muted-foreground/50">
          {[0, 20, 40, 60, 80, 100].map((t) => (
            <span key={t}>{t}</span>
          ))}
        </div>
      </div>

      {/* Thread lanes */}
      {lanes.map((thread, i) => (
        <motion.div
          key={thread.id}
          initial={{ opacity: 0 }}
          animate={isInView ? { opacity: 1 } : {}}
          transition={{ delay: 0.2 + i * 0.1 }}
          className="flex items-center border-b border-border last:border-b-0"
        >
          <div className="flex w-20 flex-shrink-0 items-center gap-2 px-4 py-3">
            <div className={`h-1.5 w-1.5 ${thread.segments.some(s => s.type === "blocked") ? "bg-muted-foreground" : "bg-foreground"}`} />
            <span className="font-mono text-[10px] text-muted-foreground">{thread.label}</span>
          </div>
          <div className="relative flex-1 py-3 pr-4">
            <div className="h-4 w-full bg-border/30">
              {thread.segments.map((seg, j) => (
                <motion.div
                  key={`${tick}-${j}`}
                  className={`absolute top-3 h-4 ${
                    seg.type === "work" ? "bg-foreground"
                    : seg.type === "wait" ? "bg-foreground/20"
                    : "bg-muted-foreground/40"
                  }`}
                  style={{ left: `${seg.start}%`, width: `${seg.end - seg.start}%` }}
                  initial={{ scaleX: 0, originX: 0 }}
                  animate={{ scaleX: 1 }}
                  transition={{ duration: 0.5, delay: i * 0.06 + j * 0.04, ease: "easeOut" }}
                />
              ))}
            </div>
          </div>
        </motion.div>
      ))}

      {/* Legend */}
      <div className="flex gap-6 border-t border-border px-4 py-2">
        {[
          { label: "Write", cls: "bg-foreground" },
          { label: "Pending", cls: "bg-foreground/20" },
          { label: "Stalled", cls: "bg-muted-foreground/40" },
        ].map((l) => (
          <div key={l.label} className="flex items-center gap-1.5">
            <div className={`h-2 w-4 ${l.cls}`} />
            <span className="font-mono text-[9px] text-muted-foreground">{l.label}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

function ChannelMonitor() {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true })
  const [bufferFill, setBufferFill] = useState(0)

  useEffect(() => {
    if (!isInView) return
    const interval = setInterval(() => {
      setBufferFill((prev) => {
        const delta = (Math.random() - 0.4) * 20
        return Math.max(0, Math.min(128, prev + delta))
      })
    }, 300)
    return () => clearInterval(interval)
  }, [isInView])

  return (
    <div ref={ref} className="border border-border p-4" style={{ boxShadow: shadow }}>
      <div className="mb-3 flex items-center gap-2">
        <div className="h-1.5 w-1.5 bg-foreground" />
        <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
          Event Buffer
        </span>
      </div>
      <div className="flex items-center gap-4">
        <div className="flex h-20 flex-1 gap-px">
          {Array.from({ length: 32 }).map((_, i) => (
            <div
              key={i}
              className="flex-1 bg-foreground transition-opacity duration-200"
              style={{ opacity: i < Math.floor(bufferFill / 4) ? 1 : 0.05 }}
            />
          ))}
        </div>
        <div className="flex flex-col gap-1 font-mono text-xs">
          <span className="text-foreground font-bold">{Math.floor(bufferFill)}/128</span>
          <span className="text-[10px] text-muted-foreground">capacity</span>
        </div>
      </div>
    </div>
  )
}

export function SectionConcurrency({ section }: { section: TechSection }) {
  return (
    <div className="mx-auto max-w-7xl px-4 py-20 lg:px-8 lg:py-32">
      {/* Header with ghost number */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="mb-12 flex items-end gap-6"
      >
        <span className="font-pixel-line text-7xl font-bold leading-none text-foreground/[0.08] md:text-9xl">
          {section.number}
        </span>
        <div className="max-w-xl pb-2">
          <div className="flex items-center gap-2 font-mono text-xs text-muted-foreground">
            <span className="text-[10px] uppercase tracking-widest">{section.subtitle}</span>
          </div>
          <h2 className="mt-3 font-pixel-line text-3xl font-bold text-foreground md:text-5xl">
            {section.title}
          </h2>
          <p className="mt-4 font-mono text-xs leading-relaxed text-muted-foreground">{section.description}</p>
        </div>
      </motion.div>

      {/* Timeline view: full width */}
      <motion.div
        initial={{ opacity: 0, y: 30 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ delay: 0.15 }}
      >
        <TimelineView />
      </motion.div>

      {/* Bottom row: channel monitor + specs */}
      <div className="mt-6 grid gap-6 lg:grid-cols-2">
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ delay: 0.25 }}
        >
          <ChannelMonitor />
        </motion.div>

        {/* Specs as big number cards */}
        <div className="grid grid-cols-2 gap-3">
          {section.specs.map((spec, i) => (
            <motion.div
              key={spec.label}
              initial={{ opacity: 0, scale: 0.95 }}
              whileInView={{ opacity: 1, scale: 1 }}
              viewport={{ once: true }}
              transition={{ delay: 0.3 + i * 0.08 }}
              className="flex flex-col items-center justify-center border border-border p-4 text-center"
              style={{ boxShadow: shadow }}
            >
              <span className="font-mono text-lg font-bold text-foreground">{spec.value}</span>
              <span className="mt-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground">{spec.label}</span>
            </motion.div>
          ))}
        </div>
      </div>
    </div>
  )
}
