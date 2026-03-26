"use client"

import type { TechSection } from "@/lib/sections-data"
import { SectionKernel } from "./sections/section-kernel"
import { SectionNetwork } from "./sections/section-network"
import { SectionLedger } from "./sections/section-ledger"
import { SectionCompiler } from "./sections/section-compiler"
import { SectionGraphics } from "./sections/section-graphics"
import { SectionLogic } from "./sections/section-logic"
import { SectionConcurrency } from "./sections/section-concurrency"
import { SectionHardware } from "./sections/section-hardware"

interface DomainSectionProps {
  section: TechSection
  index: number
}

const sectionMap: Record<string, React.FC<{ section: TechSection }>> = {
  "terminal-engine": SectionKernel,
  "rpc-api": SectionNetwork,
  "browser-automation": SectionLedger,
  "cli-commands": SectionCompiler,
  "workspace-architecture": SectionGraphics,
  "agent-system": SectionLogic,
  "storage-recovery": SectionConcurrency,
  "security-diagnostics": SectionHardware,
}

export function DomainSection({ section }: DomainSectionProps) {
  const SectionComponent = sectionMap[section.id] ?? SectionKernel

  return (
    <section id={section.id} className="relative border-b border-border">
      <SectionComponent section={section} />
    </section>
  )
}
