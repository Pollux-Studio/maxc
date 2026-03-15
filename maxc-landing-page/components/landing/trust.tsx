const badges = [
  "Open Source",
  "Rust Powered",
  "Agent Ready",
  "JSON RPC API",
  "Event Sourced",
  "Cross Platform",
];

export function Trust() {
  return (
    <section className="wrapper wrapper--ticks border-t border-nickel px-8 sm:px-10 py-6 sm:py-8 flex flex-col justify-center gap-5">
      <h6 className="text-center md:text-start text-white text-sm font-semibold">
        Built for developers building autonomous software systems
      </h6>
      <div className="flex flex-wrap items-center justify-center md:justify-start gap-3">
        {badges.map((label) => (
          <span
            key={label}
            className="rounded-md border border-[rgba(255,255,255,0.08)] bg-white/[0.02] px-3 py-1.5 text-xs text-grey font-mono"
          >
            {label}
          </span>
        ))}
      </div>
    </section>
  );
}
