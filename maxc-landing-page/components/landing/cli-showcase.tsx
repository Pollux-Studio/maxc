const commands = [
  { cmd: "maxc terminal spawn", desc: "Create a new terminal session" },
  { cmd: 'maxc terminal input "npm run dev"', desc: "Send input to a terminal" },
  { cmd: "maxc browser open http://localhost:3000", desc: "Open a browser and navigate" },
  { cmd: 'maxc run "npm test"', desc: "One command to spawn + run" },
  { cmd: 'maxc notify --title "Done" --level success', desc: "Send a notification" },
];

export function CliShowcase() {
  return (
    <section
      id="cli"
      className="wrapper wrapper--ticks border-t border-nickel grid lg:grid-cols-2 divide-x divide-y divide-nickel"
    >
      <div className="p-5 sm:p-10 flex flex-col gap-3">
        <h5 className="text-white font-semibold">Control everything from the command line</h5>
        <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
          Any AI agent that can run shell commands can control maxc. No SDK, no adapters, no configuration.
        </p>
        <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
          Works with Claude Code, OpenAI Codex, Cursor, OpenCode, or custom Python agents.
        </p>
      </div>

      <div className="flex flex-col gap-3 justify-center border-r-0 p-5 sm:p-10">
        <div className="rounded-lg overflow-hidden border border-[rgba(255,255,255,0.08)] bg-[#0a0c10]">
          <div className="flex items-center gap-2 border-b border-[rgba(255,255,255,0.08)] px-4 py-2.5 bg-[#0d0f14]">
            <span className="text-xs text-grey font-mono">terminal</span>
          </div>
          <div className="p-4 font-mono text-xs space-y-3">
            {commands.map((c) => (
              <div key={c.cmd}>
                <div className="flex items-start gap-2">
                  <span className="text-white/60 select-none shrink-0">$</span>
                  <span className="text-white/80">{c.cmd}</span>
                </div>
                <div className="text-white/20 ml-4 mt-0.5 text-[10px]"># {c.desc}</div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}
