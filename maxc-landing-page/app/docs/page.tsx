import Link from "next/link";
import { Navbar } from "@/components/landing/navbar";
import { Footer } from "@/components/landing/footer";

export default function DocsPage() {
  return (
    <main className="min-h-screen bg-[var(--background)] text-foreground">
      <Navbar />

      <section className="wrapper wrapper--ticks border-t border-nickel px-6 sm:px-10 py-14 sm:py-20">
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-white text-3xl sm:text-4xl font-bold tracking-tight">Docs</h1>
            <span className="text-[10px] uppercase tracking-widest text-black bg-white rounded-full px-2 py-1">
              Stable
            </span>
          </div>
          <p className="text-white/70 text-sm max-w-2xl">
            Everything you need to run maxc locally, automate it with the CLI, and wire it into your agent workflows.
          </p>
        </div>
      </section>

      <section className="wrapper border-t border-nickel grid lg:grid-cols-2 divide-x divide-y divide-nickel">
        <div className="p-6 sm:p-10 flex flex-col gap-4">
          <h2 className="text-white text-xl font-semibold">Quick Start</h2>
          <p className="text-white/50 text-sm leading-relaxed">
            Install the desktop app, open a workspace, and attach terminals or browsers in seconds.
          </p>
          <div className="rounded-lg border border-[rgba(255,255,255,0.08)] bg-[#0f1117] p-4">
            <div className="text-xs text-white/50 font-mono uppercase tracking-wide">Steps</div>
            <ol className="mt-3 space-y-2 text-sm text-white/80">
              <li>1. Download the installer for your OS.</li>
              <li>2. Launch maxc and create a workspace.</li>
              <li>3. Spawn a terminal or browser surface.</li>
            </ol>
          </div>
        </div>

        <div className="p-6 sm:p-10 flex flex-col gap-4 border-r-0">
          <h2 className="text-white text-xl font-semibold">CLI</h2>
          <p className="text-white/50 text-sm leading-relaxed">
            Control every surface programmatically with one-line commands.
          </p>
          <div className="rounded-lg border border-[rgba(255,255,255,0.08)] bg-[#0f1117] p-4">
            <pre className="text-xs font-mono text-white/80 leading-6">
              <span className="text-white/50">$ </span>maxc terminal spawn
              {"\n"}
              <span className="text-white/50">$ </span>maxc browser open http://localhost:3000
              {"\n"}
              <span className="text-white/50">$ </span>maxc notify --title "Done" --level success
            </pre>
          </div>
        </div>
      </section>

      <section className="wrapper border-t border-nickel grid lg:grid-cols-2 divide-x divide-y divide-nickel">
        <div className="p-6 sm:p-10 flex flex-col gap-4">
          <h2 className="text-white text-xl font-semibold">RPC</h2>
          <p className="text-white/50 text-sm leading-relaxed">
            Use JSON-RPC to drive workspaces and surfaces from any agent or script.
          </p>
          <div className="rounded-lg border border-[rgba(255,255,255,0.08)] bg-[#0f1117] p-4">
            <pre className="text-xs font-mono text-white/80 leading-6">
              {"{"}
              {"\n"}  "id": "req-1",
              {"\n"}  "method": "workspace.list",
              {"\n"}  "params": {}
              {"\n"}{"}"}
            </pre>
          </div>
        </div>

        <div className="p-6 sm:p-10 flex flex-col gap-4 border-r-0">
          <h2 className="text-white text-xl font-semibold">Configuration</h2>
          <p className="text-white/50 text-sm leading-relaxed">
            Tune fonts, scrollback, and workspace defaults through the config file.
          </p>
          <div className="rounded-lg border border-[rgba(255,255,255,0.08)] bg-[#0f1117] p-4">
            <pre className="text-xs font-mono text-white/80 leading-6">
              font-family = "JetBrains Mono"
              {"\n"}font-size = 14
              {"\n"}scrollback-limit = 50000
            </pre>
          </div>
        </div>
      </section>

      <section className="wrapper border-t border-nickel px-6 sm:px-10 py-12">
        <div className="rounded-xl border border-[rgba(255,255,255,0.08)] bg-white/[0.02] p-6 flex flex-col gap-3">
          <h2 className="text-white text-xl font-semibold">Downloads</h2>
          <p className="text-white/50 text-sm">
            Grab the latest stable installer from the downloads page.
          </p>
          <Link
            href="/downloads"
            className="inline-flex w-fit items-center gap-2 px-4 py-2 text-sm font-semibold rounded-lg bg-white text-black hover:opacity-85 transition-opacity"
          >
            Go to Downloads
          </Link>
        </div>
      </section>

      <Footer />
    </main>
  );
}
