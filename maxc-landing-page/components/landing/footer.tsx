import Image from "next/image";
import Link from "next/link";

const columns = [
  {
    title: "Product",
    links: [
      { label: "Documentation", href: "/docs" },
    ],
  },
  // {
  //   title: "Resources",
  //   links: [
  //     { label: "API Reference", href: "#" },
  //     { label: "Changelog", href: "#" },
  //   ],
  // },
  {
    title: "Social",
    links: [
      { label: "GitHub", href: "https://github.com/Pollux-Studio/maxc" },
      { label: "LinkedIn", href: "https://www.linkedin.com" },
    ],
  },
];

export function Footer() {
  return (
    <footer className="wrapper wrapper--ticks border-t border-nickel">
      <div className="px-8 sm:px-10 py-12 sm:py-16">
        <div className="grid gap-10 sm:grid-cols-2 lg:grid-cols-4">
          <div>
            <div className="flex items-center">
              <Image src="/maxc_logo_white.svg" alt="maxc" width={140} height={28} className="h-7 w-auto" />
            </div>
            <p className="mt-2 text-xs text-white/30 leading-relaxed max-w-[200px]">
              The control center for AI coding agents. Open source, Rust powered.
            </p>
          </div>
          {columns.map((col) => (
            <div key={col.title}>
              <div className="text-xs font-semibold text-white/50 uppercase tracking-wider mb-4">{col.title}</div>
              <ul className="space-y-2.5">
                {col.links.map((link) => {
                  const isInternal = link.href.startsWith("/");
                  return (
                    <li key={link.label}>
                      {isInternal ? (
                        <Link href={link.href} className="text-sm text-white/30 hover:text-white/60 transition-colors">
                          {link.label}
                        </Link>
                      ) : (
                        <a
                          href={link.href}
                          className="text-sm text-white/30 hover:text-white/60 transition-colors"
                          target="_blank"
                          rel="noreferrer"
                        >
                          {link.label}
                        </a>
                      )}
                    </li>
                  );
                })}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-12 pt-6 border-t border-[rgba(255,255,255,0.05)] flex flex-col sm:flex-row items-center justify-between gap-4">
          <span className="text-xs text-white/20">&copy; 2026 maxc. All rights reserved.</span>
          <span className="text-xs text-white/20">Built with Rust, Tauri, and React.</span>
        </div>
      </div>
    </footer>
  );
}
