import { Navbar } from "@/components/landing/navbar";
import { Footer } from "@/components/landing/footer";
import fs from "node:fs/promises";
import path from "node:path";

type InlinePart = {
  type: "text" | "bold" | "code" | "link";
  value: string;
  href?: string;
};

const parseInline = (text: string) => {
  const parts: InlinePart[] = [];
  let remaining = text;
  const pattern = /(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\([^)]+\))/;

  while (remaining.length > 0) {
    const match = remaining.match(pattern);
    if (!match || match.index === undefined) {
      parts.push({ type: "text", value: remaining });
      break;
    }

    const index = match.index;
    if (index > 0) {
      parts.push({ type: "text", value: remaining.slice(0, index) });
    }

    const token = match[0];
    if (token.startsWith("**")) {
      parts.push({ type: "bold", value: token.slice(2, -2) });
    } else if (token.startsWith("`")) {
      parts.push({ type: "code", value: token.slice(1, -1) });
    } else if (token.startsWith("[")) {
      const labelEnd = token.indexOf("]");
      const urlStart = token.indexOf("(") + 1;
      const label = token.slice(1, labelEnd);
      const href = token.slice(urlStart, -1);
      parts.push({ type: "link", value: label, href });
    }

    remaining = remaining.slice(index + token.length);
  }

  return parts;
};

const renderInline = (text: string, keyPrefix: string) => {
  const parts = parseInline(text);
  return parts.map((part, index) => {
    const key = `${keyPrefix}-${index}`;
    if (part.type === "bold") {
      return (
        <strong key={key} className="text-white">
          {part.value}
        </strong>
      );
    }
    if (part.type === "code") {
      return (
        <code
          key={key}
          className="rounded bg-white/10 px-1.5 py-0.5 font-mono text-xs text-white/80"
        >
          {part.value}
        </code>
      );
    }
    if (part.type === "link" && part.href) {
      return (
        <a
          key={key}
          href={part.href}
          className="text-white underline decoration-white/40 hover:decoration-white"
          target="_blank"
          rel="noreferrer"
        >
          {part.value}
        </a>
      );
    }
    return <span key={key}>{part.value}</span>;
  });
};

const renderMarkdown = (content: string) => {
  const lines = content.split(/\r?\n/);
  const elements: React.ReactNode[] = [];
  let listItems: string[] = [];

  const flushList = () => {
    if (listItems.length === 0) return;
    const items = listItems;
    listItems = [];
    elements.push(
      <ul key={`list-${elements.length}`} className="space-y-2 pl-5 text-white/80">
        {items.map((item, index) => (
          <li key={`list-item-${index}`} className="list-disc">
            {renderInline(item, `list-${elements.length}-${index}`)}
          </li>
        ))}
      </ul>
    );
  };

  lines.forEach((line, index) => {
    const trimmed = line.trim();

    if (!trimmed) {
      flushList();
      return;
    }

    if (trimmed === "---") {
      flushList();
      elements.push(<hr key={`hr-${index}`} className="border-white/10" />);
      return;
    }

    if (trimmed.startsWith("### ")) {
      flushList();
      elements.push(
        <h3 key={`h3-${index}`} className="text-lg font-semibold text-white">
          {renderInline(trimmed.slice(4), `h3-${index}`)}
        </h3>
      );
      return;
    }

    if (trimmed.startsWith("## ")) {
      flushList();
      elements.push(
        <h2 key={`h2-${index}`} className="text-2xl font-semibold text-white mt-6">
          {renderInline(trimmed.slice(3), `h2-${index}`)}
        </h2>
      );
      return;
    }

    if (trimmed.startsWith("# ")) {
      flushList();
      elements.push(
        <h1 key={`h1-${index}`} className="text-3xl font-bold text-white">
          {renderInline(trimmed.slice(2), `h1-${index}`)}
        </h1>
      );
      return;
    }

    if (trimmed.startsWith("- ")) {
      listItems.push(trimmed.slice(2));
      return;
    }

    flushList();
    elements.push(
      <p key={`p-${index}`} className="text-white/70">
        {renderInline(trimmed, `p-${index}`)}
      </p>
    );
  });

  flushList();
  return elements;
};

const loadChangelog = async () => {
  const candidates = [
    path.join(process.cwd(), "CHANGELOG.md"),
    path.resolve(process.cwd(), "..", "CHANGELOG.md"),
  ];

  for (const candidate of candidates) {
    try {
      return await fs.readFile(candidate, "utf8");
    } catch {
      continue;
    }
  }

  return "# Changelog\n\nChangelog file not found.";
};

export default async function ChangelogPage() {
  const changelog = await loadChangelog();

  return (
    <main className="min-h-screen bg-[var(--background)] text-foreground">
      <Navbar />

      <section className="wrapper wrapper--ticks border-t border-nickel px-6 sm:px-10 py-14 sm:py-20">
        <div className="flex flex-col gap-6">{renderMarkdown(changelog)}</div>
      </section>

      <Footer />
    </main>
  );
}
