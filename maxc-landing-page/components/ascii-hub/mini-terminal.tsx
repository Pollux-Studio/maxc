"use client"

import { useEffect, useState } from "react"
import { motion } from "framer-motion"

interface MiniTerminalProps {
  lines: string[]
  title: string
}

export function MiniTerminal({ lines, title }: MiniTerminalProps) {
  const [visibleLines, setVisibleLines] = useState<string[]>([])
  const [isInView, setIsInView] = useState(false)

  useEffect(() => {
    if (!isInView) return

    let index = 0
    const interval = setInterval(() => {
      if (index < lines.length) {
        const currentLine = lines[index]
        index++
        if (currentLine != null) {
          setVisibleLines((prev) => [...prev, currentLine])
        }
      } else {
        clearInterval(interval)
      }
    }, 200)

    return () => clearInterval(interval)
  }, [isInView, lines])

  return (
    <motion.div
      className="flex h-full flex-col border border-border"
      onViewportEnter={() => setIsInView(true)}
      viewport={{ once: true }}
    >
      {/* Terminal header */}
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <div className="h-2 w-2 bg-foreground" />
        <div className="h-2 w-2 bg-muted-foreground/50" />
        <div className="h-2 w-2 bg-muted-foreground/30" />
        <span className="ml-2 font-mono text-[10px] text-muted-foreground">
          {title} ~ terminal
        </span>
      </div>

      {/* Terminal body */}
      <div className="flex-1 bg-secondary/20 p-4">
        <div className="flex flex-col gap-0.5">
          {visibleLines.map((line, i) => (
            <motion.div
              key={`${title}-line-${i}`}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.15 }}
              className={`font-mono text-xs ${
                typeof line === "string" && line.startsWith("$")
                  ? "text-foreground"
                  : "text-muted-foreground"
              }`}
            >
              {line}
            </motion.div>
          ))}
          {visibleLines.length >= lines.length && (
            <div className="mt-1 font-mono text-xs text-foreground">
              {"$ "}
              <span className="animate-blink">{"█"}</span>
            </div>
          )}
        </div>
      </div>
    </motion.div>
  )
}
