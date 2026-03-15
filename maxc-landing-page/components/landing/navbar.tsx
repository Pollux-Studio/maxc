import Image from "next/image";
import Link from "next/link";
import { Star } from "lucide-react";

const navLinks = [
  { label: "Home", href: "/#home", type: "hash" },
  { label: "Features", href: "/#features", type: "hash" },
  { label: "Product", href: "/#product", type: "hash" },
  { label: "Open Source", href: "/#opensource", type: "hash" },
  { label: "Docs", href: "/docs", type: "route" },
];

export function Navbar() {
  return (
    <div className="sticky top-0 z-50 w-full bg-[var(--background)] border-b border-nickel">
      <header className="wrapper px-6 py-5 flex items-center justify-between">
        <div className="flex gap-10 items-center">
          <Link href="/" className="flex items-center">
            <Image src="/maxc_logo_white.svg" alt="maxc" width={160} height={40} className="h-10 w-auto" priority />
          </Link>
          <nav className="hidden lg:flex items-center gap-1">
            {navLinks.map((link) => (
              <Link
                key={link.label}
                href={link.href}
                className="px-3 py-2 text-sm font-medium text-white hover:opacity-70 transition-opacity"
              >
                {link.label}
              </Link>
            ))}
          </nav>
        </div>
        <div className="flex items-center gap-4">
          <a
            href="https://github.com/Pollux-Studio/maxc"
            className="flex items-center gap-2 rounded-full border border-[rgba(255,255,255,0.12)] bg-white/[0.02] px-3 py-1.5 text-xs text-white/80 hover:text-white hover:bg-white/[0.04] transition-colors"
            aria-label="Star on GitHub"
            target="_blank"
            rel="noreferrer"
          >
            <Star className="size-3" />
            <span className="font-semibold">Star</span>
            <span className="text-white/40">GitHub</span>
          </a>
        </div>
      </header>
      <div className="wrapper relative h-0 border-l border-r border-nickel" />
    </div>
  );
}
