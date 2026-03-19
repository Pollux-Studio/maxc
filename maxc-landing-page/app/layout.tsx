import type { Metadata } from "next";
import { JetBrains_Mono, Space_Grotesk, Geist } from "next/font/google";
import { Analytics } from "@vercel/analytics/next";
import "./globals.css";
import { cn } from "@/lib/utils";

const geist = Geist({subsets:['latin'],variable:'--font-sans'});

const spaceGrotesk = Space_Grotesk({
  variable: "--font-space",
  subsets: ["latin"],
});

const jetbrainsMono = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  metadataBase: new URL("https://maxc.polluxstudio.in"),
  title: {
    default: "maxc - Workspace for AI Coding Agents",
    template: "%s - maxc",
  },
  description:
    "maxc is a programmable workspace for AI coding agents. Run terminals, automate browsers, and orchestrate AI agents from one developer environment.",
  keywords: [
    "AI coding agents",
    "developer automation tools",
    "terminal workspace",
    "browser automation tools",
    "AI development environment",
    "agent orchestration",
    "developer productivity tools",
  ],
  applicationName: "maxc",
  alternates: {
    canonical: "/",
  },
  robots: {
    index: true,
    follow: true,
  },
  openGraph: {
    title: "maxc - Control Center for AI Coding Agents",
    description:
      "Run terminals, control browsers, and orchestrate AI agents from one programmable workspace.",
    url: "/",
    siteName: "maxc",
    images: [
      {
        url: "/og.png",
        width: 1200,
        height: 630,
        alt: "maxc - Control Center for AI Coding Agents",
      },
    ],
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "maxc - AI Coding Agent Workspace",
    description: "Run terminals, browsers, and AI agents in one programmable workspace.",
    images: ["/og.png"],
  },
  icons: {
    icon: [{ url: "/maxc_logo_full_white_single.svg" }],
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const structuredData = {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: "maxc",
    applicationCategory: "DeveloperApplication",
    description:
      "maxc is a programmable workspace for AI coding agents that allows developers to run terminals, automate browsers, and orchestrate agent workflows.",
    operatingSystem: ["Windows", "macOS", "Linux"],
    url: "https://maxc.polluxstudio.in",
  };

  return (
    <html lang="en" className={cn("dark font-sans", geist.variable)}>
      <head>
        <script
          type="application/ld+json"
          dangerouslySetInnerHTML={{ __html: JSON.stringify(structuredData) }}
        />
      </head>
      <body
        className={`${spaceGrotesk.variable} ${jetbrainsMono.variable} antialiased`}
      >
        {children}
        <Analytics />
      </body>
    </html>
  );
}
