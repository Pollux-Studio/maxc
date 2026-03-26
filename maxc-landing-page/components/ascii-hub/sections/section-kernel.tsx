"use client"

import { motion, useInView } from "framer-motion"
import { useRef, useState, useEffect } from "react"
import type { TechSection } from "@/lib/sections-data"

const shadow = "rgba(14, 63, 126, 0.04) 0px 0px 0px 1px, rgba(42, 51, 69, 0.04) 0px 1px 1px -0.5px, rgba(42, 51, 70, 0.04) 0px 3px 3px -1.5px, rgba(42, 51, 70, 0.04) 0px 6px 6px -3px, rgba(14, 63, 126, 0.04) 0px 12px 12px -6px, rgba(14, 63, 126, 0.04) 0px 24px 24px -12px"

/*
  SECTION 01: TERMINAL ENGINE
  Style: Full-width terminal takeover. The entire section looks like one big terminal window.
  The background is inverted (white on black), with a persistent "menu bar" at top.
*/

function BootSequence() {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true })
  const [lines, setLines] = useState<string[]>([])
  const bootLines = [
    "[  0.000000] maxc terminal engine v0.2.0 starting",
    "[  0.000012] detecting platform: Windows 11 (x86_64)",
    "[  0.000034] ConPTY interface: AVAILABLE",
    "[  0.000089] fallback stdio: STANDBY",
    "[  0.001204] shell registry: PowerShell 7.4, Bash 5.2",
    "[  0.002100] shell registry: Node 22.x, Python 3.12",
    "[  0.003400] session pool: 0/32 allocated",
    "[  0.004200] subscription manager: READY",
    "[  0.005100] resize handler: ACTIVE",
    "[  OK  ] Terminal engine ready.",
  ]

  useEffect(() => {
    if (!isInView) return
    let i = 0
    const interval = setInterval(() => {
      if (i < bootLines.length) {
        const currentLine = bootLines[i]
        i++
        setLines((prev) => [...prev, currentLine])
      } else {
        clearInterval(interval)
      }
    }, 150)
    return () => clearInterval(interval)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isInView])

  return (
    <div ref={ref} className="bg-background p-6 font-mono text-xs leading-relaxed text-foreground">
      {lines.map((line, i) => (
        <motion.div
          key={i}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          className={
            typeof line === "string" && line.startsWith("[  OK")
              ? "mt-2 font-bold text-foreground"
              : "text-muted-foreground"
          }
        >
          {line}
        </motion.div>
      ))}
      {lines.length < bootLines.length && (
        <span className="animate-blink inline-block text-foreground">{"_"}</span>
      )}
    </div>
  )
}

export function SectionKernel({ section }: { section: TechSection }) {
  return (
    <div className="mx-auto max-w-7xl px-4 py-20 lg:px-8 lg:py-32">
      {/* Section label with ghost number */}
      <motion.div
        initial={{ opacity: 0 }}
        whileInView={{ opacity: 1 }}
        viewport={{ once: true }}
        className="mb-6 flex items-end gap-6"
      >
        <span className="font-pixel-line text-7xl font-bold leading-none text-foreground/[0.08] md:text-9xl">
          {section.number}
        </span>
        <div className="flex-1 pb-2">
          <div className="flex items-center gap-3">
            <div className="h-px flex-1 bg-border" />
          </div>
        </div>
      </motion.div>

      {/* Giant terminal window */}
      <motion.div
        initial={{ opacity: 0, y: 40 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.6 }}
        className="overflow-hidden border border-border"
        style={{ boxShadow: shadow }}
      >
        {/* Terminal title bar */}
        <div className="flex items-center justify-between border-b border-border bg-foreground px-4 py-2">
          <div className="flex items-center gap-3">
            <div className="flex gap-1.5">
              <div className="h-2.5 w-2.5 border border-background/30 bg-background" />
              <div className="h-2.5 w-2.5 border border-background/30 bg-background/60" />
              <div className="h-2.5 w-2.5 border border-background/30 bg-background/30" />
            </div>
            <span className="font-mono text-xs text-background">
              maxc@terminal:~
            </span>
          </div>
          <span className="font-mono text-[10px] text-background/50">maxc v0.2.0</span>
        </div>

        {/* Terminal body with two columns */}
        <div className="grid lg:grid-cols-5">
          {/* Left: Boot sequence (3 cols) */}
          <div className="border-b border-border lg:col-span-3 lg:border-b-0 lg:border-r">
            <div className="border-b border-border px-4 py-2">
              <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
                Init Sequence
              </span>
            </div>
            <BootSequence />
          </div>

          {/* Right: System info panel (2 cols) */}
          <div className="flex flex-col lg:col-span-2">
            <div className="border-b border-border px-4 py-2">
              <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
                Engine Overview
              </span>
            </div>

            {/* Title area */}
            <div className="border-b border-border p-6">
              <h2 className="font-pixel-line text-3xl font-bold text-foreground md:text-4xl">
                {section.title}
              </h2>
              <p className="mt-3 font-mono text-xs leading-relaxed text-muted-foreground">
                {section.description}
              </p>
            </div>

            {/* Specs as system parameters */}
            <div className="flex-1 p-4">
              <div className="flex flex-col gap-3">
                {section.specs.map((spec, i) => (
                  <motion.div
                    key={spec.label}
                    initial={{ opacity: 0, x: 10 }}
                    whileInView={{ opacity: 1, x: 0 }}
                    viewport={{ once: true }}
                    transition={{ delay: 0.3 + i * 0.1 }}
                    className="flex items-start gap-2 font-mono text-xs"
                  >
                    <span className="text-muted-foreground">{">"}</span>
                    <span className="text-muted-foreground">{spec.label}:</span>
                    <span className="font-bold text-foreground">{spec.value}</span>
                  </motion.div>
                ))}
              </div>
            </div>
          </div>
        </div>

        {/* Bottom ASCII schematic */}
        <div className="border-t border-border">
          <div className="flex items-center gap-2 border-b border-border px-4 py-2">
            <div className="h-1.5 w-1.5 bg-foreground" />
            <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
              Engine Schematic
            </span>
          </div>
          <pre className="overflow-x-auto p-6 font-mono text-xs leading-relaxed text-muted-foreground">
            {section.ascii}
          </pre>
        </div>
      </motion.div>
    </div>
  )
}
