import { Navbar } from "@/components/landing/navbar";
import { Footer } from "@/components/landing/footer";

const examples = [
  {
    title: "Automated test runner",
    desc: "Spawn a terminal, run test suites, and notify agents on completion.",
  },
  {
    title: "Browser regression sweep",
    desc: "Open Chromium, execute scripted flows, and capture screenshots.",
  },
  {
    title: "Agent-driven build pipeline",
    desc: "Coordinate multiple AI agents to compile, lint, and deploy.",
  },
];

export default function ExamplesPage() {
  return (
    <main className="min-h-screen bg-[var(--background)] text-foreground">
      <Navbar />

      <section className="wrapper wrapper--ticks border-t border-nickel px-6 sm:px-10 py-14 sm:py-20">
        <div className="flex flex-col gap-4">
          <h1 className="text-white text-3xl sm:text-4xl font-bold tracking-tight">Examples</h1>
          <p className="text-white/60 text-sm max-w-2xl">
            Practical workflows that show how maxc automates terminals, browsers, and AI agents.
          </p>
        </div>
      </section>

      <section className="wrapper border-t border-nickel grid lg:grid-cols-3 divide-x divide-y divide-nickel">
        {examples.map((example, index) => (
          <div key={example.title} className={`p-6 sm:p-10 ${index === examples.length - 1 ? "lg:border-r-0" : ""}`}>
            <h2 className="text-white text-lg font-semibold">{example.title}</h2>
            <p className="mt-3 text-white/50 text-sm leading-relaxed">{example.desc}</p>
          </div>
        ))}
      </section>

      <Footer />
    </main>
  );
}
