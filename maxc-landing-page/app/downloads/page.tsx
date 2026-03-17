import Image from "next/image";
import { Navbar } from "@/components/landing/navbar";
import { Footer } from "@/components/landing/footer";

export const dynamic = "force-dynamic";

type PlatformEntry = {
  signature: string;
  url: string;
};

type LatestJson = {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Record<string, PlatformEntry>;
};

type Asset = {
  name: string;
  url: string;
  signature: string;
  platformKey: string;
};

const latestJsonUrl =
  "https://github.com/Pollux-Studio/maxc/releases/download/stable/latest.json";

const getFileName = (url: string) => {
  const parts = url.split("/");
  return parts[parts.length - 1] || url;
};

const formatDate = (value: string) => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("en-US", { dateStyle: "medium", timeStyle: "short" });
};

const dedupeByUrl = (assets: Asset[]) => {
  const map = new Map<string, Asset>();
  for (const asset of assets) {
    if (!map.has(asset.url)) {
      map.set(asset.url, asset);
    }
  }
  return Array.from(map.values());
};

const getPlatformLabel = (platformKey: string) =>
  platformKey.replace(/-/g, " ").replace(/_/g, " ").trim();

function WindowsLogo({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      aria-hidden="true"
      className={className}
      fill="currentColor"
    >
      <rect x="2" y="3" width="9" height="8" rx="1" />
      <rect x="13" y="3" width="9" height="8" rx="1" />
      <rect x="2" y="13" width="9" height="8" rx="1" />
      <rect x="13" y="13" width="9" height="8" rx="1" />
    </svg>
  );
}

function AssetRow({ asset }: { asset: Asset }) {
  return (
    <div className="rounded-lg border border-[rgba(255,255,255,0.08)] bg-white/[0.02] px-4 py-3">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="text-sm text-white/85 font-mono break-all">{asset.name}</div>
          <div className="text-[10px] text-white/40 mt-1">{getPlatformLabel(asset.platformKey)}</div>
        </div>
        <a
          href={asset.url}
          className="text-xs font-semibold text-black bg-white rounded-md px-3 py-1.5 whitespace-nowrap hover:opacity-85 transition-opacity"
        >
          Download
        </a>
      </div>
    </div>
  );
}

export default async function DownloadsPage() {
  const response = await fetch(latestJsonUrl, { cache: "no-store" });
  const latest = (await response.json()) as LatestJson;

  const assets = dedupeByUrl(
    Object.entries(latest.platforms).map(([platformKey, entry]) => ({
      name: getFileName(entry.url),
      url: entry.url,
      signature: entry.signature,
      platformKey,
    }))
  );

  const windowsAssets = assets.filter((asset) => asset.platformKey.startsWith("windows"));
  const macAssets = assets.filter((asset) => asset.platformKey.startsWith("darwin"));
  const linuxAssets = assets.filter((asset) => asset.platformKey.startsWith("linux"));

  return (
    <main className="min-h-screen bg-[var(--background)] text-foreground">
      <Navbar />

      <section className="wrapper wrapper--ticks border-t border-nickel px-6 sm:px-10 py-14 sm:py-20">
        <div className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-3">
            <h1 className="text-white text-3xl sm:text-4xl font-bold tracking-tight">Downloads</h1>
            <span className="text-[10px] uppercase tracking-widest text-black bg-white rounded-full px-2 py-1">
              Latest
            </span>
          </div>
          <div className="text-white/70 text-sm">
            maxc stable &middot; version {latest.version} &middot; published {formatDate(latest.pub_date)}
          </div>
          <div className="text-white/50 text-sm">{latest.notes}</div>
        </div>
      </section>

      <section className="wrapper wrapper--ticks border-t border-nickel px-6 sm:px-10 py-10">
        <div className="grid gap-6 lg:grid-cols-3">
          <div className="rounded-xl border border-[rgba(255,255,255,0.08)] bg-white/[0.02] p-5">
            <div className="flex items-center gap-2">
              <WindowsLogo className="h-5 w-5 text-white/70" />
              <div className="text-white font-semibold text-lg">Windows</div>
            </div>
            <div className="text-white/40 text-xs mt-1">x64</div>
            <div className="mt-5 space-y-3">
              {windowsAssets.map((asset) => (
                <AssetRow key={asset.url} asset={asset} />
              ))}
            </div>
          </div>

          <div className="rounded-xl border border-[rgba(255,255,255,0.08)] bg-white/[0.02] p-5">
            <div className="flex items-center gap-2">
              <Image
                src="/apple-logo-svgrepo-com.svg"
                alt="Apple"
                width={20}
                height={20}
                className="h-5 w-5 object-contain invert opacity-70"
              />
              <div className="text-white font-semibold text-lg">macOS</div>
            </div>
            <div className="text-white/40 text-xs mt-1">Apple Silicon</div>
            <div className="mt-5 space-y-3">
              {macAssets.map((asset) => (
                <AssetRow key={asset.url} asset={asset} />
              ))}
            </div>
          </div>

          <div className="rounded-xl border border-[rgba(255,255,255,0.08)] bg-white/[0.02] p-5">
            <div className="flex items-center gap-2">
              <Image
                src="/linux-svgrepo-com.svg"
                alt="Linux"
                width={20}
                height={20}
                className="h-5 w-5 object-contain grayscale invert opacity-70"
              />
              <div className="text-white font-semibold text-lg">Linux</div>
            </div>
            <div className="text-white/40 text-xs mt-1">AppImage, DEB, RPM</div>
            <div className="mt-5 space-y-3">
              {linuxAssets.map((asset) => (
                <AssetRow key={asset.url} asset={asset} />
              ))}
            </div>
          </div>
        </div>
      </section>

      <Footer />
    </main>
  );
}
