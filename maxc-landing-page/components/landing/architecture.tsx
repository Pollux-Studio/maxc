export function Architecture() {
  return (
    <section
      id="architecture"
      className="wrapper wrapper--ticks border-t border-nickel grid lg:grid-cols-2 divide-x divide-y divide-nickel"
    >
      <div className="p-5 sm:p-10 flex flex-col gap-3">
        <h2 className="text-white text-xl font-semibold">A programmable developer environment</h2>
        <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
          The Rust backend processes every command through an event-sourced pipeline. All state is recoverable, all actions are replayable.
        </p>
        <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
          Agents connect via CLI commands, JSON-RPC socket, or environment hooks. No SDK required.
        </p>
      </div>

      <div className="flex flex-col gap-3 justify-center border-r-0 p-5 sm:p-10">
        <div className="font-mono text-sm space-y-2">
          {[
            { label: "Frontend UI", sub: "Tauri + React + xterm.js", color: "text-blue-400" },
            { label: "RPC Client", sub: "JSON-RPC over named pipe", color: "text-cyan-400" },
            { label: "maxc Backend", sub: "Rust automation server", color: "text-white/70" },
          ].map((layer, i, arr) => (
            <div key={layer.label}>
              <div className="rounded-md border border-[rgba(255,255,255,0.08)] bg-white/[0.02] px-4 py-3 text-center">
                <div className={`text-xs font-semibold ${layer.color}`}>{layer.label}</div>
                <div className="text-[10px] text-white/30 mt-0.5">{layer.sub}</div>
              </div>
              {i < arr.length - 1 && (
                <div className="flex justify-center py-1">
                  <div className="w-px h-4 bg-white/10" />
                </div>
              )}
            </div>
          ))}

          <div className="flex justify-center py-1">
            <div className="w-px h-4 bg-white/10" />
          </div>

          <div className="grid grid-cols-2 gap-2">
            {[
              { label: "Terminal", sub: "ConPTY / PTY" },
              { label: "Browser", sub: "Chromium CDP" },
              { label: "Agent", sub: "Worker + Task" },
              { label: "Events", sub: "Append-only log" },
            ].map((rt) => (
              <div key={rt.label} className="rounded-md border border-[rgba(255,255,255,0.08)] bg-white/[0.02] px-3 py-2 text-center">
                <div className="text-[10px] font-semibold text-white/60">{rt.label}</div>
                <div className="text-[9px] text-white/25 mt-0.5">{rt.sub}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
