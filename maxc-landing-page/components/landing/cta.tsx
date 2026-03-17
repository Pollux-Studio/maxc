import Link from "next/link";

export function CTA() {
  return (
    <section className="wrapper wrapper--ticks border-t border-nickel px-5 sm:px-10 py-20 sm:py-32 flex flex-col justify-center items-center text-center">
      <h3 className="text-white max-w-2xl text-balance text-3xl sm:text-4xl font-bold tracking-tight">
        Start building with maxc today
      </h3>
      <p className="mt-4 text-white/50 max-w-md text-pretty">
        Bring terminals, browsers, and AI agents into one workspace.
      </p>
      <div className="flex flex-wrap items-center justify-center gap-4 mt-10">
        <Link href="/downloads" className="inline-flex items-center gap-2 px-6 py-2.5 text-sm font-semibold rounded-lg bg-white text-black hover:opacity-85 transition-opacity">
          Download
        </Link>
        <Link href="/docs" className="inline-flex items-center gap-2 px-6 py-2.5 text-sm font-semibold rounded-lg border border-[rgba(255,255,255,0.08)] text-white hover:bg-white/[0.04] transition-colors">
          Documentation
        </Link>
      </div>
    </section>
  );
}
