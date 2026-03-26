"use client"

import { motion, useInView } from "framer-motion"
import { useRef, useState, useEffect } from "react"
import type { TechSection } from "@/lib/sections-data"

const shadow = "rgba(14, 63, 126, 0.04) 0px 0px 0px 1px, rgba(42, 51, 69, 0.04) 0px 1px 1px -0.5px, rgba(42, 51, 70, 0.04) 0px 3px 3px -1.5px, rgba(42, 51, 70, 0.04) 0px 6px 6px -3px, rgba(14, 63, 126, 0.04) 0px 12px 12px -6px, rgba(14, 63, 126, 0.04) 0px 24px 24px -12px"

/*
  SECTION 06: AGENT SYSTEM
  Style: Blueprint / schematic. Grid paper background.
  Interactive readiness calculator where you can toggle inputs.
  Gate diagram rendered with CSS.
  Totally different feel: structured, precise, engineering-drawing look.
*/

function InteractiveTruthTable() {
  const [inputs, setInputs] = useState({ A: 0, B: 0, C: 0, D: 0 })

  const toggle = (key: "A" | "B" | "C" | "D") => {
    setInputs((prev) => ({ ...prev, [key]: prev[key] === 0 ? 1 : 0 }))
  }

  // READY = (TERM AND BROW) OR (TASK AND PROM)
  const Q = (inputs.A && inputs.B) || (inputs.C && inputs.D) ? 1 : 0
  const AB = inputs.A && inputs.B ? 1 : 0
  const CD = inputs.C && inputs.D ? 1 : 0

  return (
    <div className="border border-border p-6" style={{ boxShadow: shadow }}>
      <div className="mb-4 flex items-center gap-2">
        <div className="h-1.5 w-1.5 bg-foreground" />
        <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
          Agent Readiness Calculator
        </span>
      </div>

      {/* Input toggles */}
      <div className="grid grid-cols-4 gap-3">
        {(["A", "B", "C", "D"] as const).map((key) => {
          const labels: Record<string, string> = { A: "TERM", B: "BROW", C: "TASK", D: "PROM" }
          return (
            <button
              key={key}
              onClick={() => toggle(key)}
              className={`flex flex-col items-center gap-2 border p-3 font-mono transition-all duration-200 focus-visible:ring-2 focus-visible:ring-foreground focus-visible:outline-none ${
                inputs[key]
                  ? "border-foreground bg-foreground text-background"
                  : "border-border text-muted-foreground hover:border-foreground hover:text-foreground"
              }`}
            >
              <span className="text-lg font-bold">{labels[key]}</span>
              <span className="text-[10px]">{inputs[key]}</span>
            </button>
          )
        })}
      </div>

      {/* Gate visualization */}
      <div className="mt-6 flex items-center justify-center gap-6 font-mono text-xs">
        <div className="flex flex-col items-center gap-1">
          <span className="text-[10px] text-muted-foreground">TERM & BROW</span>
          <div className={`flex h-8 w-16 items-center justify-center border ${AB ? "border-foreground bg-foreground text-background" : "border-border text-muted-foreground"}`}>
            AND
          </div>
          <span className="font-bold">{AB}</span>
        </div>

        <div className="flex flex-col items-center gap-1">
          <span className="text-[10px] text-muted-foreground">{"-->"}</span>
          <div className={`flex h-8 w-16 items-center justify-center border ${Q ? "border-foreground bg-foreground text-background" : "border-border text-muted-foreground"}`}>
            OR
          </div>
          <span className="text-lg font-bold text-foreground">READY={Q}</span>
        </div>

        <div className="flex flex-col items-center gap-1">
          <span className="text-[10px] text-muted-foreground">TASK & PROM</span>
          <div className={`flex h-8 w-16 items-center justify-center border ${CD ? "border-foreground bg-foreground text-background" : "border-border text-muted-foreground"}`}>
            AND
          </div>
          <span className="font-bold">{CD}</span>
        </div>
      </div>

      <div className="mt-4 text-center font-mono text-[10px] text-muted-foreground">
        READY = (TERM & BROW) OR (TASK & PROM) — Click to toggle
      </div>
    </div>
  )
}

function AnimatedWaveform() {
  const ref = useRef(null)
  const isInView = useInView(ref, { once: true })
  const [ticks, setTicks] = useState<number[]>(() =>
    Array.from({ length: 32 }, () => Math.random() > 0.5 ? 1 : 0)
  )

  useEffect(() => {
    if (!isInView) return
    const interval = setInterval(() => {
      setTicks((prev) => {
        const next = [...prev.slice(1), Math.random() > 0.5 ? 1 : 0]
        return next
      })
    }, 200)
    return () => clearInterval(interval)
  }, [isInView])

  return (
    <div ref={ref} className="border border-border p-4" style={{ boxShadow: shadow }}>
      <div className="mb-3 flex items-center gap-2">
        <motion.div
          className="h-1.5 w-1.5 bg-foreground"
          animate={{ opacity: [1, 0.3, 1] }}
          transition={{ repeat: Infinity, duration: 1 }}
        />
        <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">
          Agent Heartbeat
        </span>
      </div>
      <div className="flex h-12 items-end gap-px">
        {ticks.map((val, i) => (
          <motion.div
            key={i}
            animate={{ height: val ? "100%" : "20%" }}
            className="flex-1 bg-foreground"
            transition={{ duration: 0.15, ease: "easeOut" }}
          />
        ))}
      </div>
    </div>
  )
}

export function SectionLogic({ section }: { section: TechSection }) {
  return (
    <div className="relative mx-auto max-w-7xl px-4 py-20 lg:px-8 lg:py-32">
      {/* Blueprint grid background */}
      <div className="pointer-events-none absolute inset-0 opacity-[0.03]" aria-hidden="true">
        <div
          className="h-full w-full"
          style={{
            backgroundImage: `
              linear-gradient(currentColor 1px, transparent 1px),
              linear-gradient(90deg, currentColor 1px, transparent 1px)
            `,
            backgroundSize: "40px 40px",
          }}
        />
      </div>

      {/* Header with ghost number and blueprint stamp look */}
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        className="relative mb-12 flex items-start gap-6"
      >
        <span className="font-pixel-line text-7xl font-bold leading-none text-foreground/[0.08] md:text-9xl">
          {section.number}
        </span>
        <div className="flex-1 pt-2">
          <div className="inline-block border-2 border-foreground px-6 py-4" style={{ boxShadow: shadow }}>
            <span className="font-mono text-xs text-muted-foreground">{section.subtitle}</span>
            <h2 className="font-pixel-line text-3xl font-bold text-foreground md:text-5xl">{section.title}</h2>
          </div>
          <p className="mt-6 max-w-2xl font-mono text-xs leading-relaxed text-muted-foreground">{section.description}</p>
        </div>
      </motion.div>

      <div className="relative grid gap-6 lg:grid-cols-2">
        {/* Left: Interactive truth table */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ delay: 0.15 }}
        >
          <InteractiveTruthTable />
        </motion.div>

        {/* Right: Specs + waveform */}
        <div className="flex flex-col gap-6">
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ delay: 0.2 }}
          >
            <AnimatedWaveform />
          </motion.div>

          {/* Specs as horizontal data sheet */}
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ delay: 0.25 }}
            className="border border-border" style={{ boxShadow: shadow }}
          >
            <div className="flex items-center gap-2 border-b border-border px-4 py-2">
              <div className="h-1.5 w-1.5 bg-foreground" />
              <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">Data Sheet</span>
            </div>
            <div className="grid grid-cols-2 gap-0">
              {section.specs.map((spec, i) => (
                <div
                  key={spec.label}
                  className={`flex flex-col gap-1 p-4 font-mono ${
                    i < section.specs.length - 2 ? "border-b border-border" : ""
                  } ${i % 2 === 0 ? "border-r border-border" : ""}`}
                >
                  <span className="text-[10px] uppercase tracking-wider text-muted-foreground">{spec.label}</span>
                  <span className="text-sm font-bold text-foreground">{spec.value}</span>
                </div>
              ))}
            </div>
          </motion.div>
        </div>
      </div>
    </div>
  )
}
