"use client"

import { motion } from "framer-motion"

interface SpecTableProps {
  specs: { label: string; value: string }[]
}

export function SpecTable({ specs }: SpecTableProps) {
  return (
    <div className="border border-border">
      <div className="flex items-center gap-2 border-b border-border px-4 py-2">
        <div className="h-1.5 w-1.5 bg-foreground" />
        <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
          Specifications
        </span>
      </div>
      <div className="divide-y divide-border">
        {specs.map((spec, i) => (
          <motion.div
            key={spec.label}
            initial={{ opacity: 0, x: -10 }}
            whileInView={{ opacity: 1, x: 0 }}
            viewport={{ once: true }}
            transition={{ delay: i * 0.05, duration: 0.3 }}
            className="flex items-center justify-between px-4 py-2.5 transition-colors duration-200 hover:bg-secondary/50"
          >
            <span className="font-mono text-xs text-muted-foreground">
              {spec.label}
            </span>
            <span className="font-mono text-xs text-foreground">
              {spec.value}
            </span>
          </motion.div>
        ))}
      </div>
    </div>
  )
}
