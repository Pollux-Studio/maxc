import { MockWorkspace } from "@/components/landing/mock-workspace";

export function Hero() {
  return (
    <section
      id="home"
      className="wrapper wrapper--ticks border-t border-nickel grid md:grid-cols-2 w-full md:divide-x divide-nickel"
    >
      {/* Left: text + CTA */}
      <div className="flex flex-col p-8 sm:p-10 justify-between gap-10 items-center md:items-start">
        <div className="flex flex-col gap-5 items-center md:items-start text-center md:text-left">
          <span className="text-grey text-xs font-mono uppercase tracking-wide">
            Open Source &middot; Built with Rust
          </span>

          <h1 className="text-white text-pretty max-w-[25rem] text-4xl sm:text-5xl font-bold tracking-tight leading-[1.1]">
            The Control Center for AI Coding Agents
          </h1>

          <p className="text-white/70 md:text-lg max-w-[27rem] text-pretty">
            Run terminals, browsers, and AI agents in one programmable workspace.
          </p>
          <p className="text-white/50 text-sm max-w-[30rem] text-pretty">
            maxc is a programmable workspace designed for AI coding agents. Developers can run terminal sessions,
            automate browsers, and orchestrate AI agents inside a unified development environment. With a JSON-RPC
            automation interface and real runtime execution, maxc enables powerful automation workflows for modern
            software teams.
          </p>

          <div className="flex items-center gap-4 mt-6">
            <a href="/downloads" className="inline-flex items-center gap-2 px-6 py-2.5 text-sm font-semibold rounded-lg bg-white text-black hover:opacity-85 transition-opacity">
              Download now
            </a>
          </div>
        </div>

      </div>

      {/* Right: product UI */}
      <div className="flex flex-col sm:min-h-[32rem] overflow-hidden">
        <div className="relative px-8 sm:px-10 py-10 h-full flex flex-col justify-center overflow-hidden">
          <div className="origin-center scale-[1.12] translate-x-6 md:translate-x-10">
            <MockWorkspace heightClass="h-[340px]" />
          </div>
        </div>
      </div>
    </section>
  );
}
