"use client";

import Image from "next/image";
import Link from "next/link";
import { Star } from "lucide-react";
import { useState } from "react";

const navLinks = [
  { label: "Home", href: "/#home", type: "hash" },
  { label: "Features", href: "/#features", type: "hash" },
  { label: "Product", href: "/#product", type: "hash" },
  { label: "Open Source", href: "/#opensource", type: "hash" },
  { label: "Changelogs", href: "/changelogs", type: "route" },
];

export function Navbar() {
  const [showBanner, setShowBanner] = useState(true);

  return (
    <div className="sticky top-0 z-50 w-full bg-[var(--background)] border-b border-nickel">
      {showBanner ? (
        <div className="top-banner hidden md:block relative w-full overflow-hidden border-b border-white/10">
          <Link
            href="/downloads"
            aria-label="Maxc v0.2.0 announcement"
            className="group block relative w-full no-underline text-white"
          >
            <div aria-hidden="true" className="absolute inset-0 z-0">
              <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,_rgba(255,255,255,0.15),_rgba(7,10,15,0.85))]" />
              <div className="absolute inset-0 bg-[linear-gradient(120deg,_rgba(255,255,255,0.08),_transparent_45%,_rgba(255,255,255,0.06))]" />
            </div>
            <div className="relative z-10 w-full h-10 flex px-4">
              <div className="flex items-center gap-2 w-full max-w-360 mx-auto px-4">
                <Image
                  src="/@maxc_logo@.svg"
                  alt="maxc logo"
                  width={12}
                  height={12}
                  className="w-5 h-5 shrink-0 hidden sm:block drop-shadow-md/70"
                />
                <span className="text-xs font-medium font-mono leading-snug tracking-wide uppercase whitespace-nowrap overflow-hidden text-ellipsis text-shadow-md/50">
                  Maxcv0.2.0 is live
                </span>
                <svg
                  className="ml-auto shrink-0 transition-transform duration-200 group-hover:translate-x-1"
                  width="20"
                  height="20"
                  viewBox="0 0 20 20"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  <rect width="20" height="20" rx="4" fill="#08060D"></rect>
                  <rect
                    x="0.5"
                    y="0.5"
                    width="19"
                    height="19"
                    rx="3.5"
                    stroke="white"
                    strokeOpacity="0.15"
                  ></rect>
                  <path
                    d="M10 6L14 10L10 14"
                    stroke="white"
                    strokeWidth="1.2"
                    strokeLinejoin="round"
                  ></path>
                  <path
                    d="M14 10L6 10"
                    stroke="white"
                    strokeWidth="1.2"
                    strokeLinejoin="round"
                  ></path>
                </svg>
              </div>
            </div>
          </Link>
          <button
            aria-label="Close banner"
            className="absolute right-2 top-1/2 -translate-y-1/2 z-20 p-2 text-white hover:opacity-70 transition-opacity"
            type="button"
            onClick={(event) => {
              event.preventDefault();
              setShowBanner(false);
            }}
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              aria-hidden="true"
              role="img"
              width="1em"
              height="1em"
              viewBox="0 0 24 24"
              className="size-5"
            >
              <path
                fill="none"
                stroke="currentColor"
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth="2"
                d="M18 6L6 18M6 6l12 12"
              ></path>
            </svg>
          </button>
        </div>
      ) : null}
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
