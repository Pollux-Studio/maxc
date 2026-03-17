import { Code2, Globe2, Bot, Monitor } from "lucide-react";

const highlights = [
  { icon: Code2, label: "Rust backend", desc: "High-performance event-sourced server" },
  { icon: Globe2, label: "52 RPC methods", desc: "Full automation surface via JSON-RPC" },
  { icon: Monitor, label: "Tauri desktop", desc: "Native window with React + xterm.js" },
  { icon: Bot, label: "Agent agnostic", desc: "Claude, Codex, Cursor, any agent" },
];

export function OpenSource() {
  return (
    <>
      <section
        id="opensource"
        className="wrapper wrapper--ticks border-t border-nickel px-5 sm:px-10 py-14 sm:py-28 flex flex-col justify-center gap-3 text-center items-center"
      >
        <h2 className="text-white max-w-2xl text-balance text-center text-3xl sm:text-4xl font-bold tracking-tight">
          Open source AI agent workspace
        </h2>
        <p className="max-w-md text-white/70 text-balance sm:text-pretty">
          Inspect every line, contribute, or fork. maxc is built for extensibility.
        </p>
      </section>

      <section className="wrapper wrapper--ticks border-t border-nickel grid sm:grid-cols-2 lg:grid-cols-4 divide-x divide-y divide-nickel">
        {highlights.map((h, i) => (
          <div
            key={h.label}
            className={`p-5 sm:p-8 flex flex-col gap-2 ${i === highlights.length - 1 ? "border-r-0" : ""}`}
          >
            <h.icon className="size-5 text-white/70 mb-1" />
            <div className="text-sm font-semibold text-white">{h.label}</div>
            <div className="text-xs text-white/40 leading-relaxed">{h.desc}</div>
          </div>
        ))}
      </section>
    </>
  );
}
