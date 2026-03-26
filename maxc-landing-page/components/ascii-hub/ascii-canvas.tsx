"use client"

import { useState } from "react"
import { motion } from "framer-motion"

interface AsciiCanvasProps {
  art: string
  label: string
}

export function AsciiCanvas({ art, label }: AsciiCanvasProps) {
  const [isHovered, setIsHovered] = useState(false)

  return (
    <motion.div
      className="relative overflow-hidden border border-border bg-secondary/30 p-4 transition-all duration-200"
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      whileTap={{ scale: 0.98 }}
      role="img"
      aria-label={label}
    >
      <div className="mb-2 flex items-center gap-2">
        <div className="h-1.5 w-1.5 bg-foreground" />
        <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          ASCII Schematic
        </span>
      </div>
      <pre
        className={`overflow-x-auto font-mono text-[9px] leading-[12px] transition-opacity duration-300 md:text-[11px] md:leading-[14px] ${
          isHovered ? "text-foreground opacity-100" : "text-foreground/80 opacity-80"
        }`}
      >
        {art}
      </pre>
      <span className="sr-only">{label}</span>
    </motion.div>
  )
}
