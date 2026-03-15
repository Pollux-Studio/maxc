import { Navbar } from "@/components/landing/navbar";
import { Hero } from "@/components/landing/hero";
import { Trust } from "@/components/landing/trust";
import { Features } from "@/components/landing/features";
import { Product } from "@/components/landing/product";
import { Architecture } from "@/components/landing/architecture";
import { CliShowcase } from "@/components/landing/cli-showcase";
import { OpenSource } from "@/components/landing/open-source";
import { CTA } from "@/components/landing/cta";
import { Footer } from "@/components/landing/footer";

export default function Home() {
  return (
    <main className="min-h-screen bg-[var(--background)] text-foreground">
      <Navbar />
      <Hero />
      <Trust />
      <Features />
      <Product />
      <Architecture />
      <CliShowcase />
      <OpenSource />
      <CTA />
      <Footer />
    </main>
  );
}
