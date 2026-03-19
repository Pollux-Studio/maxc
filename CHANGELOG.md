# Changelog

All notable changes to maxc are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/).

---

## [0.2.0] - 2026-03-19

### Added

- **Native browser integration** — embedded Tauri webview renders directly inside browser panes using `@tauri-apps/api/webview`. No iframes, no CORS, real browser engine (WebView2 on Windows, WebKit on macOS).
- **Browser detection** — Settings > Integrations tab with "Detect Browsers" button. Scans `%LOCALAPPDATA%`, `%ProgramFiles%`, `%ProgramFiles(x86)%` dynamically for Chrome, Chrome Beta/Dev/Canary, Brave, Vivaldi, Edge, and WebView2.
- **Manual browser entry** — add custom browser executables via file picker in the Integrations tab. Persisted to localStorage.
- **`system.browsers` RPC method** — returns all detected browsers with names, paths, and runtime types.
- **`system.config.rate_limit` RPC method** — dynamically change backend rate limits at runtime. Settings rate limit now applies to both frontend and backend.
- **`browser_runtime` param on `browser.create`** — per-request browser runtime override. Frontend sends selected browser preference to backend.
- **`browser_headless` config** — `MAXC_BROWSER_HEADLESS` environment variable controls whether browsers launch in headless mode.
- **`maxc browser detect` CLI command** — lists available browsers from the command line.
- **Workspace edit drawer** — click the edit icon on a workspace to open the right drawer with populated name, folder, and environment variables. Add, edit, or delete env vars and save.
- **Terminal padding** — 6px top/bottom, 8px left/right padding inside terminal panes.
- **Scrollable settings tabs** — tabs in the settings dialog scroll horizontally when they overflow.
- **`.scrollbar-none` CSS utility** — cross-browser hidden scrollbar class.

### Fixed

- **Auto-restart after update** — `app.restart()` called after `download_and_install` completes. App relaunches automatically with the new version.
- **NSIS installer mode** — changed to `installMode: "currentUser"`. Installs to `%LOCALAPPDATA%` instead of `Program Files`. No admin/UAC prompt required.
- **First-launch "INTERNAL: internal" error** — event store path now uses Tauri's `app_data_dir()` (absolute, writable) instead of a relative CWD path that failed in system directories.
- **"No session token" on fresh install** — fixed by resolving the event store path issue above. Session creation now succeeds on first launch.
- **Browser webview z-order** — native webviews collapse to 0x0 at (-9999,-9999) when switching tabs or opening dialogs. Restored to correct position/size when switching back. No more browser UI bleeding over terminals or dialogs.
- **Browser selection not applied** — user's browser choice from Integrations tab now sent as `browser_runtime` param to `browser.create`. Backend matches the selected runtime to the correct executable.
- **Chrome/Edge confusion** — Chrome and Edge are now discovered as separate targets (`chrome`, `edge`) instead of being lumped under `chromium-cdp`. No more silent fallthrough from Chrome to Edge.
- **Hardcoded browser paths** — replaced `C:\Program Files\...` with dynamic resolution via `%LOCALAPPDATA%`, `%ProgramFiles%`, `%ProgramFiles(x86)%` environment variables. Works on any drive letter or install location.
- **Terminal input stuck on fast typing** — removed in-flight serialization guard. Terminal input is now fire-and-forget with 4ms batching. No more keystroke queuing behind slow RPC responses.
- **Terminal input lag** — reduced poll interval from 500ms to 100ms (5x faster output), reduced batch timer from 12ms to 4ms, exempted `terminal.input` and `terminal.history` from the global RPC rate limiter via `rpcDirect`.

### Changed

- **Rate limit setting** — now syncs to both frontend sliding-window limiter AND backend token-bucket limiter via `system.config.rate_limit` RPC. Label updated to "Applies to both frontend and backend rate limits."
- **Rate limit synced on startup** — saved rate limit preference applied to backend on app launch.
- Removed theme toggle button from workspace sidebar header.
- Tauri `unstable` feature enabled for multiwebview support.
- 7 new webview permissions added to capabilities.

---

## [0.1.1] - 2026-03-14

### Added

- Terminal input batching to reduce lag.
- Settings dialog with tabs (Workspace, Agent, Shortcuts, Rate Limit, Updates, About).
- App icons and title bar logo (light/dark modes).
- Versioned release name and updater fallback.
- MAXC environment variables display with copy button.
- CI: WebKitGTK and GTK dev libs for Linux builds.

### Fixed

- Clippy warnings resolved.
- Storage test fixes.
- CLI test formatting.
- BrowserView props fix.
- Bundle identifier fix.

---

## [0.1.0] - 2026-03-12

### Added

- Initial release.
- Rust backend with event-sourced storage, JSON-RPC server (52 methods), terminal runtime (ConPTY), browser runtime (CDP), agent worker system.
- Tauri v2 desktop app with React frontend.
- Workspace management (create, list, update, delete, layout).
- Pane system (create, split, close, resize, list) with react-resizable-panels.
- Surface system (create, close, focus, list) with tab bar.
- xterm.js terminal rendering with full PTY support.
- Browser automation (27 RPC methods for navigation, clicks, screenshots, cookies, storage, network, traces).
- Agent worker system (create, list, get, close, task start/cancel/list/get, terminal/browser attach/detach).
- Notification system (send, list, clear) with toast overlay and panel.
- CLI with 40+ commands covering all RPC methods.
- `maxc run` and `maxc open` convenience commands.
- `MAXC_TOKEN`, `MAXC_SOCKET_PATH`, `MAXC_WORKSPACE_ID`, `MAXC_SURFACE_ID`, `MAXC_PANE_ID` environment variable fallbacks in CLI.
- Multi-window support via `Ctrl+Shift+N`.
- Landing page (Next.js + shadcn + Tailwind).
