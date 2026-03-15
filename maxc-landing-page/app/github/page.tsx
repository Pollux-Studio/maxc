import { Navbar } from "@/components/landing/navbar";
import { Footer } from "@/components/landing/footer";

export default function GitHubPage() {
  return (
    <main className="min-h-screen bg-[var(--background)] text-foreground">
      <Navbar />

      <section className="wrapper wrapper--ticks border-t border-nickel px-6 sm:px-10 py-14 sm:py-20">
        <div className="flex flex-col gap-4">
          <h1 className="text-white text-3xl sm:text-4xl font-bold tracking-tight">GitHub</h1>
          <p className="text-white/60 text-sm max-w-2xl">
            Explore the maxc source code, releases, and contribution guides on GitHub.
          </p>
          <a
            href="https://github.com/Pollux-Studio/maxc"
            className="inline-flex w-fit items-center gap-2 px-4 py-2 text-sm font-semibold rounded-lg bg-white text-black hover:opacity-85 transition-opacity"
            target="_blank"
            rel="noreferrer"
          >
            Visit GitHub
          </a>
        </div>
      </section>

      <Footer />
    </main>
  );
}
