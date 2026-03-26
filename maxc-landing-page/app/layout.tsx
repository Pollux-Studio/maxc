import type { Metadata, Viewport } from "next"
import { Geist_Mono, Silkscreen } from "next/font/google"
import { GeistPixelLine } from "geist/font/pixel"
import { Analytics } from "@vercel/analytics/next"
import "./globals.css"

const geistMono = Geist_Mono({
  subsets: ["latin"],
  variable: "--font-mono",
})

const silkscreen = Silkscreen({
  weight: ["400", "700"],
  subsets: ["latin"],
  variable: "--font-pixel",
})

const geistPixelLine = GeistPixelLine

const siteUrl = "https://maxc.polluxstudio.in"
const siteName = "maxc"
const siteTitle = "maxc | Programmable Developer Workspace"
const siteDescription =
  "Open-source programmable developer workspace unifying terminals, browser automation, and task orchestration. Built with Rust + Tauri v2 + React. 52 RPC methods. 40+ CLI commands."

export const metadata: Metadata = {
  metadataBase: new URL(siteUrl),
  title: {
    default: siteTitle,
    template: "%s | maxc",
  },
  description: siteDescription,
  generator: "maxc",
  applicationName: siteName,
  authors: [{ name: "Pollux Studio", url: "https://github.com/Pollux-Studio" }],
  creator: "Pollux Studio",
  publisher: "Pollux Studio",
  keywords: [
    "developer workspace",
    "terminal multiplexer",
    "browser automation",
    "task orchestration",
    "Rust",
    "Tauri",
    "ConPTY",
    "CDP",
    "CLI",
    "open-source",
    "developer tools",
    "workspace manager",
    "xterm.js",
    "JSON-RPC",
    "agent system",
  ],
  icons: {
    icon: [
      {
        url: "/maxc_logo_full_white_single.svg",
        type: "image/svg+xml",
      },
    ],
    apple: "/apple-icon.png",
  },
  openGraph: {
    type: "website",
    locale: "en_US",
    url: siteUrl,
    siteName,
    title: siteTitle,
    description: siteDescription,
    images: [
      {
        url: `${siteUrl}/og.png`,
        width: 1200,
        height: 630,
        alt: "maxc — Programmable Developer Workspace",
        type: "image/png",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: siteTitle,
    description: siteDescription,
    images: [`${siteUrl}/og.png`],
  },
  robots: {
    index: true,
    follow: true,
    googleBot: {
      index: true,
      follow: true,
      "max-video-preview": -1,
      "max-image-preview": "large",
      "max-snippet": -1,
    },
  },
  alternates: {
    canonical: siteUrl,
  },
  category: "Developer Tools",
}

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#ffffff" },
    { media: "(prefers-color-scheme: dark)", color: "#000000" },
  ],
  width: "device-width",
  initialScale: 1,
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html lang="en" className={`dark ${geistPixelLine.variable}`}>
      <head>
        <link rel="icon" href="/maxc_logo_full_white_single.svg" type="image/svg+xml" />
      </head>
      <body
        className={`${geistMono.variable} ${silkscreen.variable} font-mono antialiased`}
      >
        {children}
        <Analytics />
      </body>
    </html>
  )
}
