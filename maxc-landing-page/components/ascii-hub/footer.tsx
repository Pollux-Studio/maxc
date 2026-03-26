"use client"

import { motion } from "framer-motion"
import { Github, ArrowUp } from "lucide-react"

const socialLinks = [
  { name: "GitHub", icon: Github, href: "https://github.com/Pollux-Studio/maxc" },
]

export function Footer() {
  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: "smooth" })
  }

  return (
    <footer className="border-t border-border">
      <div className="mx-auto max-w-7xl px-4 py-16 lg:px-8 lg:py-24">
        <div className="grid gap-12 lg:grid-cols-3">
          {/* Logo */}
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5 }}
          >
            <div aria-label="maxc logo" role="img">
              <img src="/maxc_logo_white.svg" alt="maxc" className="h-12 w-auto opacity-40 invert dark:invert-0" />
            </div>
            <p className="mt-4 max-w-xs font-mono text-xs leading-relaxed text-muted-foreground">
              Open-source programmable developer workspace.
              Terminals, browsers, and agents unified.
            </p>
          </motion.div>

          {/* Social Grid */}
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5, delay: 0.1 }}
          >
            <span className="mb-4 block font-mono text-xs uppercase tracking-widest text-muted-foreground">
              Connect
            </span>
            <div className="flex flex-col gap-2">
              {socialLinks.map((link) => (
                <a
                  key={link.name}
                  href={link.href}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="group flex items-center gap-3 py-2 font-mono text-sm text-muted-foreground transition-all duration-200 hover:text-foreground focus-visible:ring-2 focus-visible:ring-foreground focus-visible:outline-none"
                >
                  <link.icon size={14} />
                  <span>{link.name}</span>
                  <span className="ml-auto opacity-0 transition-opacity duration-200 group-hover:opacity-100">
                    {"->"}
                  </span>
                </a>
              ))}
            </div>
          </motion.div>

          {/* Meta & Back to top */}
          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            transition={{ duration: 0.5, delay: 0.2 }}
            className="flex flex-col justify-between"
          >
            <div>
              <span className="mb-4 block font-mono text-xs uppercase tracking-widest text-muted-foreground">
                Tech Stack
              </span>
              <div className="flex flex-wrap gap-2">
                {["Rust", "Tauri v2", "React 19", "TypeScript", "xterm.js", "CDP"].map(
                  (tech) => (
                    <span
                      key={tech}
                      className="border border-border px-2 py-1 font-mono text-[10px] text-muted-foreground"
                    >
                      {tech}
                    </span>
                  )
                )}
              </div>
            </div>

            <button
              onClick={scrollToTop}
              className="mt-8 flex items-center gap-2 self-start font-mono text-xs text-muted-foreground transition-all duration-200 hover:text-foreground focus-visible:ring-2 focus-visible:ring-foreground focus-visible:outline-none lg:self-end"
              aria-label="Back to top"
            >
              <ArrowUp size={12} />
              <span>BACK TO TOP</span>
            </button>
          </motion.div>
        </div>

        {/* Bottom bar */}
        <div className="mt-16 flex flex-col items-center justify-between gap-4 border-t border-border pt-8 sm:flex-row">
          <span className="font-mono text-[10px] text-muted-foreground">
            {"// "} maxc &mdash; {new Date().getFullYear()}
          </span>
          <span className="font-mono text-[10px] text-muted-foreground">
            Built with Rust. Powered by Tauri.
          </span>
        </div>
      </div>
    </footer>
  )
}
