import { Terminal, Globe2, Bot } from "lucide-react";

type MockWorkspaceProps = {
  heightClass?: string;
};

export function MockWorkspace({ heightClass = "h-[280px]" }: MockWorkspaceProps) {
  return (
    <div className="rounded-lg overflow-hidden border border-[rgba(255,255,255,0.08)] bg-[#0a0c10]">
      <div className="flex items-center justify-between border-b border-[rgba(255,255,255,0.08)] px-4 py-2.5 bg-[#0d0f14]">
        <span className="text-[10px] text-white/60 font-mono">maxc workspace &mdash; dev</span>
        <div className="flex items-center gap-3 text-white/60 text-xs font-mono select-none">
          <span className="leading-none">_</span>
          <span className="leading-none">□</span>
          <span className="leading-none">×</span>
        </div>
      </div>

      <div className={`flex ${heightClass}`}>
        <div className="w-28 border-r border-[rgba(255,255,255,0.06)] bg-[#0c0e13] p-3 space-y-1.5">
          <div className="rounded bg-white/10 px-2 py-1.5 text-[10px] text-white/70 font-medium">dev</div>
          <div className="rounded px-2 py-1.5 text-[10px] text-white/30">server</div>
          <div className="rounded px-2 py-1.5 text-[10px] text-white/30">tests</div>
        </div>

        <div className="flex-1 flex">
          <div className="flex-1 flex flex-col border-r border-[rgba(255,255,255,0.06)]">
            <div className="border-b border-[rgba(255,255,255,0.06)] px-3 py-1.5 flex items-center gap-1.5">
              <Terminal className="size-3 text-white/60" />
              <span className="text-[10px] text-white/40 font-mono">Terminal 1</span>
            </div>
            <div className="flex-1 p-3 font-mono text-[10px] text-white/40 leading-5 space-y-0.5">
              <div><span className="text-white/60">$</span> npm run dev</div>
              <div className="text-white/20">ready on 0.0.0.0:3000</div>
              <div className="text-white/20">compiled in 1.2s</div>
              <div className="text-blue-400/60">GET /api/health 200 12ms</div>
            </div>
          </div>

          <div className="flex-1 flex flex-col">
            <div className="flex-1 border-b border-[rgba(255,255,255,0.06)] flex flex-col">
              <div className="border-b border-[rgba(255,255,255,0.06)] px-3 py-1.5 flex items-center gap-1.5">
                <Globe2 className="size-3 text-blue-400" />
                <span className="text-[10px] text-white/40 font-mono">Browser</span>
              </div>
              <div className="flex-1 flex items-center justify-center">
                <div className="w-20 h-14 rounded bg-white/[0.02] border border-[rgba(255,255,255,0.05)] flex items-center justify-center">
                  <span className="text-[8px] text-white/20">localhost:3000</span>
                </div>
              </div>
            </div>

            <div className="h-24 flex flex-col">
              <div className="border-b border-[rgba(255,255,255,0.06)] px-3 py-1.5 flex items-center gap-1.5">
                <Bot className="size-3 text-purple-400" />
                <span className="text-[10px] text-white/40 font-mono">Agent</span>
                <span className="ml-auto text-[8px] text-white/70 bg-white/10 rounded px-1.5 py-0.5">Running</span>
              </div>
              <div className="flex-1 p-3 font-mono text-[10px] text-white/30 leading-4">
                <div>Running tests...</div>
                <div className="text-white/50">12/14 passed</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
