import { Terminal, Globe2, Bot, LayoutGrid, Zap, Code2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";

type Feature = { icon: LucideIcon; title: string; description: string };

const features: Feature[] = [
  { icon: Terminal, title: "Instant Terminal Sessions", description: "On demand terminal spawning with full PTY support and split pane multiplexing." },
  { icon: Globe2, title: "Browser Automation", description: "Control real Chromium browsers. Navigate, click, type, screenshot — all via API." },
  { icon: Bot, title: "AI Agent Workers", description: "Attach any AI coding agent to terminals and browsers. Start tasks, observe, cancel." },
  { icon: Code2, title: "JSON RPC API", description: "52 methods. Automate every action using CLI, socket, or environment hooks." },
];

export function Features() {
  return (
    <>
      <section
        id="features"
        className="wrapper wrapper--ticks border-t border-nickel px-5 sm:px-10 py-14 sm:py-28 flex flex-col justify-center gap-3 text-center items-center"
      >
        <h2 className="text-white max-w-2xl text-balance text-center text-3xl sm:text-4xl font-bold tracking-tight">
          Run terminals and browsers in one workspace
        </h2>
        <p className="max-w-md text-white/70 text-balance sm:text-pretty">
          Terminals, browsers, and agents in one programmable surface.
        </p>
      </section>

      <section className="wrapper wrapper--ticks border-t border-nickel grid lg:grid-cols-2 divide-x divide-y divide-nickel">
        {features.map((f, i) => (
          <div
            key={f.title}
            className={`p-5 sm:p-10 flex flex-col gap-3 ${i % 2 === 1 ? "border-r-0" : ""} ${i >= features.length - 2 ? "lg:border-b-0" : ""}`}
          >
            <div className="flex items-center gap-3">
              <f.icon className="size-5 text-white/70" />
              <h5 className="text-white font-semibold">{f.title}</h5>
            </div>
            <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
              {f.description}
            </p>
          </div>
        ))}
      </section>
    </>
  );
}
