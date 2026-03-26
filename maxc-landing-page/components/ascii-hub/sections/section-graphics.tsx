"use client"

import { motion, useInView } from "framer-motion"
import { useRef, useState, useEffect } from "react"
import type { TechSection } from "@/lib/sections-data"

const shadow = "rgba(14, 63, 126, 0.04) 0px 0px 0px 1px, rgba(42, 51, 69, 0.04) 0px 1px 1px -0.5px, rgba(42, 51, 70, 0.04) 0px 3px 3px -1.5px, rgba(42, 51, 70, 0.04) 0px 6px 6px -3px, rgba(14, 63, 126, 0.04) 0px 12px 12px -6px, rgba(14, 63, 126, 0.04) 0px 24px 24px -12px"

/*
  SECTION 05: WORKSPACE ARCHITECTURE
  Style: Oscilloscope / scanline waveform animation.
*/

const POINTS = 120

function Oscilloscope() {
  const ref = useRef<HTMLCanvasElement>(null)
  const animRef = useRef<number>(0)

  useEffect(() => {
    const canvas = ref.current
    if (!canvas) return
    const ctx = canvas.getContext("2d")
    if (!ctx) return

    let t = 0

    const draw = () => {
      const W = canvas.width
      const H = canvas.height
      ctx.clearRect(0, 0, W, H)

      // Scanline glow background lines
      ctx.strokeStyle = "rgba(255,255,255,0.03)"
      ctx.lineWidth = 1
      for (let y = 0; y < H; y += 4) {
        ctx.beginPath()
        ctx.moveTo(0, y)
        ctx.lineTo(W, y)
        ctx.stroke()
      }

      // Grid
      ctx.strokeStyle = "rgba(255,255,255,0.06)"
      ctx.lineWidth = 0.5
      const cols = 8
      const rows = 4
      for (let i = 0; i <= cols; i++) {
        const x = (W / cols) * i
        ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke()
      }
      for (let i = 0; i <= rows; i++) {
        const y = (H / rows) * i
        ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(W, y); ctx.stroke()
      }

      // Main waveform — composite of multiple frequencies
      ctx.strokeStyle = "rgba(255,255,255,0.85)"
      ctx.lineWidth = 2
      ctx.shadowColor = "rgba(255,255,255,0.5)"
      ctx.shadowBlur = 8
      ctx.beginPath()
      for (let i = 0; i <= POINTS; i++) {
        const x = (W / POINTS) * i
        const phase = (i / POINTS) * Math.PI * 2
        const y =
          H / 2 +
          (H / 5) * (
            Math.sin(phase * 3 + t) * 0.5 +
            Math.sin(phase * 7 - t * 1.3) * 0.25 +
            Math.sin(phase * 11 + t * 0.7) * 0.15 +
            Math.sin(phase * 2 - t * 0.4) * 0.1
          )
        i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)
      }
      ctx.stroke()

      // Secondary dim waveform
      ctx.strokeStyle = "rgba(255,255,255,0.2)"
      ctx.lineWidth = 1
      ctx.shadowBlur = 0
      ctx.beginPath()
      for (let i = 0; i <= POINTS; i++) {
        const x = (W / POINTS) * i
        const phase = (i / POINTS) * Math.PI * 2
        const y =
          H / 2 +
          (H / 6) * Math.sin(phase * 5 + t * 1.8 + 1.2)
        i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)
      }
      ctx.stroke()

      t += 0.025
      animRef.current = requestAnimationFrame(draw)
    }

    // Resize observer
    const observer = new ResizeObserver(() => {
      canvas.width = canvas.offsetWidth
      canvas.height = canvas.offsetHeight
    })
    observer.observe(canvas)
    canvas.width = canvas.offsetWidth
    canvas.height = canvas.offsetHeight

    animRef.current = requestAnimationFrame(draw)
    return () => {
      cancelAnimationFrame(animRef.current)
      observer.disconnect()
    }
  }, [])

  return (
    <canvas
      ref={ref}
      className="h-48 w-full border border-border bg-background md:h-64"
      style={{ boxShadow: shadow }}
    />
  )
}

const pipelineStages = [
  { name: "Window", icon: "WN", desc: "App container" },
  { name: "Workspace", icon: "WS", desc: "Layout context" },
  { name: "Pane", icon: "PN", desc: "Split region" },
  { name: "Surface", icon: "SF", desc: "Content host" },
  { name: "Panel", icon: "PL", desc: "Term or Browser" },
]

export function SectionGraphics({ section }: { section: TechSection }) {
  return (
    <div className="py-20 lg:py-32">
      <div className="mx-auto max-w-7xl px-4 lg:px-8">

        {/* Header */}
        <motion.div
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={{ once: true }}
          className="mb-10 flex items-end gap-6"
        >
          <span className="font-pixel-line text-7xl font-bold leading-none text-foreground/[0.08] md:text-9xl">
            {section.number}
          </span>
          <div className="pb-4">
            <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">{section.subtitle}</span>
            <h2 className="font-pixel-line text-3xl font-bold text-foreground md:text-5xl">
              {section.title}
            </h2>
          </div>
        </motion.div>

        {/* Oscilloscope */}
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 0.6 }}
        >
          <div className="mb-2 flex items-center gap-2">
            <span className="h-2 w-2 animate-pulse bg-foreground" />
            <span className="font-mono text-[10px] uppercase tracking-widest text-muted-foreground">SIGNAL — WORKSPACE ACTIVITY</span>
          </div>
          <Oscilloscope />
        </motion.div>

        {/* Description */}
        <motion.p
          initial={{ opacity: 0, y: 15 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          className="mt-8 max-w-3xl font-mono text-sm leading-relaxed text-muted-foreground"
        >
          {section.description}
        </motion.p>

        {/* Pipeline stages */}
        <div className="mt-12 flex flex-wrap items-center justify-center gap-0">
          {pipelineStages.map((stage, i) => (
            <motion.div
              key={stage.name}
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ delay: 0.1 + i * 0.1 }}
              className="flex items-center"
            >
              <div className="flex flex-col items-center gap-2 px-4 py-2">
                <div className="flex h-12 w-12 items-center justify-center border border-foreground bg-foreground font-mono text-sm font-bold text-background">
                  {stage.icon}
                </div>
                <span className="font-mono text-[10px] font-bold text-foreground">{stage.name}</span>
                <span className="font-mono text-[9px] text-muted-foreground">{stage.desc}</span>
              </div>
              {i < pipelineStages.length - 1 && (
                <div className="hidden items-center md:flex" aria-hidden="true">
                  <div className="h-px w-8 bg-border" />
                  <div className="h-0 w-0 border-y-[3px] border-l-[5px] border-y-transparent border-l-foreground" />
                </div>
              )}
            </motion.div>
          ))}
        </div>

        {/* Specs */}
        <div className="mt-12 grid grid-cols-2 gap-4 md:grid-cols-4">
          {section.specs.map((spec, i) => (
            <motion.div
              key={spec.label}
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ delay: 0.2 + i * 0.08 }}
              className="flex flex-col items-center border border-border p-6 text-center"
              style={{ boxShadow: shadow }}
            >
              <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">{spec.label}</span>
              <span className="mt-2 font-mono text-sm font-bold text-foreground md:text-lg">{spec.value}</span>
            </motion.div>
          ))}
        </div>
      </div>
    </div>
  )
}
