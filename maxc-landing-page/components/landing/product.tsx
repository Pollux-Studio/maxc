import { MockWorkspace } from "@/components/landing/mock-workspace";

export function Product() {
  return (
    <>
      <section
        id="product"
        className="wrapper wrapper--ticks border-t border-nickel px-5 sm:px-10 py-14 sm:py-28 flex flex-col justify-center gap-3 text-center items-center"
      >
        <h2 className="text-white max-w-2xl text-balance text-center text-3xl sm:text-4xl font-bold tracking-tight">
          Automate development workflows with AI agents
        </h2>
      </section>

      <section className="wrapper border-t border-nickel grid lg:grid-cols-2 divide-x divide-y divide-nickel">
        <div className="p-5 sm:p-10 flex flex-col gap-3 border-l border-nickel">
          <h5 className="text-white font-semibold">Workspace Layouts</h5>
          <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
            Split panes, surface tabs, and resizable layouts. Organize terminals, browsers, and agent panels side by side.
          </p>
          <p className="sm:max-w-[28rem] text-white/50 text-pretty text-sm leading-relaxed">
            Every layout persists across restarts via the durable event store.
          </p>
        </div>
        <div className="flex flex-col gap-3 justify-between border-r-0">
          <div className="p-5 sm:p-10 border-r border-nickel">
            <MockWorkspace />
          </div>
        </div>
      </section>
    </>
  );
}
