# 🚀 maxc

<p align="center">
<b>A programmable developer workspace for terminals, browsers, and AI agents</b>
</p>

<p align="center">
Build faster. Control everything. Work from one environment.
</p>

---

## ⬇️ Download

Download **maxc v0.2.0**:

[![Windows](https://img.shields.io/badge/Windows-MSI-0078D6?logo=windows&logoColor=white)](https://github.com/Pollux-Studio/maxc/releases/tag/v0.2.0)

## ✨ Overview

**maxc** is an open source developer workspace that brings **terminals, browser automation, logs, and task orchestration into one unified environment**.

Instead of managing many terminal windows, browser tabs, and background processes separately, **maxc organizes everything into structured workspaces** that can be controlled manually or programmatically through a CLI and automation API.

The goal of maxc is simple:

> Turn the traditional terminal into a **programmable development control center**.


## 🌍 Vision

Modern development workflows are becoming increasingly complex.

Developers now manage:

* multiple terminal processes
* web applications running in browsers
* automated tests
* background services
* AI coding agents

Traditional terminals were not designed for this level of orchestration.

**maxc aims to create a unified environment where developers and AI agents can coordinate tasks, automate workflows, and control development systems from a single workspace.**

## 🎯 Mission

Our mission is to build a **fast, scriptable, and extensible workspace environment** that allows developers to:

* manage multiple terminals efficiently
* automate browser interactions
* orchestrate development tasks
* integrate AI coding agents seamlessly
* build automated development pipelines

All inside one consistent and programmable system.

## 🧭 Motto

> **One workspace. Total control.**

## 🤔 Why maxc?

Typical development setups often look like this:

```
Terminal 1  -> backend server
Terminal 2  -> frontend build
Terminal 3  -> logs
Browser     -> application preview
Terminal 4  -> test runner
```

This quickly becomes difficult to manage and almost impossible to automate.

**maxc solves this by organizing everything inside a single structured workspace.**

Example workspace:

```
Workspace: project-dev

Pane 1
 ├ Terminal: backend server
 ├ Terminal: test runner

Pane 2
 └ Browser: application preview

Pane 3
 └ Terminal: logs
```

Everything stays organized and can be controlled programmatically.

---

## 🧩 Key Features

### 🖥 Terminal Multiplexing

Run multiple terminal sessions inside a single workspace with split panes and tabbed surfaces.

### 🗂 Workspace Management

Organize development environments into structured workspaces.

### 🌐 Browser Surfaces

Embed browser sessions directly inside the workspace and automate them.

### ⚙ Automation API

Control the workspace programmatically using CLI commands or socket based RPC.

### 🤖 AI Agent Integration

Allow AI coding agents to coordinate tasks across multiple terminals and browsers.

### 🔔 Notifications

Receive alerts when tasks complete, tests fail, or agents require attention.

### 📊 Sidebar Metadata

Display workspace information such as logs, progress indicators, and status updates.

## 🏗 Core Concepts

maxc organizes development sessions using a structured hierarchy.

```
Window
 └ Workspace
     └ Pane
         └ Surface
             └ Panel
```

### Window

Application window that contains one or more workspaces.

### Workspace

A development environment containing multiple panes.

### Pane

A split region inside a workspace.

### Surface

A tab inside a pane.

### Panel

The actual content running in the surface.

Panel types include:

* Terminal
* Browser

## 🏛 Architecture

maxc uses a modular architecture designed for performance and extensibility.

```
                     maxc
────────────────────────────────────

                   UI Layer
         window management + layout

                        │
                        ▼

               Workspace Manager
     Window → Workspace → Pane → Surface

                        │
          ┌─────────────┴─────────────┐
          │                           │

      Terminal Engine            Browser Engine
         ConPTY               Chromium + Playwright

          │                           │
          ▼                           ▼

      Shell Processes            Chromium Runtime
      PowerShell / Bash          DOM + JavaScript


                Automation Layer
         socket RPC + CLI interface

                        │
                        ▼

             Notification System
        desktop alerts + sidebar
```

## 📂 Repository Structure

```
maxc/

core/
  workspace_manager     # workspace lifecycle + state
  pane_manager          # split panes + layout state
  surface_manager       # tabs/surfaces + routing

terminal/
  conpty_engine         # Windows PTY integration
  terminal_parser       # VT parsing + buffer model
  terminal_renderer     # terminal render pipeline

browser/
  chromium_runtime      # embedded Chromium runtime
  playwright_driver     # browser automation driver
  dom_controller        # DOM interaction layer
  automation_api        # browser RPC/commands

automation/
  rpc_server            # JSON-RPC server
  command_dispatcher    # RPC method routing
  socket_protocol       # transport + protocol

ui/
  window_manager        # top-level window system
  layout_engine         # pane/surface layout
  sidebar               # workspace + metadata sidebar
  surface_tabs          # tab strip + surface switching

notifications/
  desktop_notifications # native notifications

cli/
  command_parser        # CLI parsing + commands

config/
  configuration_loader  # config files + defaults
```

## 🖥 Terminal Engine

Terminal surfaces run shell sessions using the Windows pseudo terminal system.

Responsibilities include:

* spawning terminal processes
* parsing terminal output
* rendering terminal buffers
* handling keyboard input

Supported shells include:

* PowerShell
* Bash via WSL
* Node.js REPL
* Python

## 🌐 Browser Engine

Browser surfaces allow developers and AI agents to interact with web applications directly inside the workspace.

Runtime direction: Chromium controlled through a Playwright-based backend driver.

Capabilities include:

* page navigation
* DOM interaction
* JavaScript execution
* screenshots
* cookie and storage management

## 🔌 Automation API

maxc exposes a programmable interface using JSON RPC over a local socket.

Example request:

```
{
 "id":"req1",
 "method":"workspace.list",
 "params":{}
}
```

Example response:

```
{
 "ok":true,
 "result":{"workspaces":[...]}
}
```

This enables scripts, automation tools, and AI agents to control the workspace.

## 💻 CLI Interface

The CLI communicates with the automation API.

Example commands:

```
maxc list-workspaces
maxc new-workspace
maxc send "npm run build"
maxc notify --title "Build finished"
```

## 🔔 Notifications

maxc supports desktop notifications and workspace alerts for events such as:

* build completion
* test failures
* deployment status
* agent requests

## 🧭 User Workflow (Quick Start)

Here is a practical end-to-end flow for using maxc:

1. **Launch maxc**
   - The main window opens with a sidebar and a primary workspace area.
   - Open additional windows with `Ctrl+Shift+N`.

2. **Create a workspace**
   - Click the `+` in the sidebar (or press `Ctrl+N`).
   - Give it a name, select a folder, and optionally set environment variables.
   - A root pane is created automatically.

3. **Split panes and open surfaces**
   - Split right: `Ctrl+D`
   - Split down: `Ctrl+Shift+D`
   - New terminal surface: `Ctrl+T`
   - New browser surface: `Ctrl+B`

4. **Use terminals and browsers**
   - Each pane has its own tab bar (surfaces are tabs).
   - Terminals run your shell sessions.
   - Browser surfaces open embedded sessions for app previews and automation.

5. **Notifications**
   - Agents and scripts can send notifications.
   - The bell icon shows unread counts and opens the notification panel.

6. **Automation via CLI / RPC**
   - Any agent or script can control maxc via CLI or JSON-RPC:

```bash
maxc terminal spawn --workspace-id ws-1 --surface-id sf-1
maxc terminal input --workspace-id ws-1 --surface-id sf-1 --terminal-session-id ts-1 --input "npm test\n"
maxc browser create --workspace-id ws-1 --surface-id sf-2
maxc browser tab-open --workspace-id ws-1 --surface-id sf-2 --browser-session-id bs-1 --url http://localhost:3000
maxc notify --title "Tests complete" --body "All tests passed" --level success
```

## ⚙ Configuration

Configuration files allow customization of appearance and behavior.

Example configuration:

```
font-family = JetBrains Mono
font-size = 14
scrollback-limit = 50000
working-directory = ~/projects
```

Configuration location:

```
~/.config/maxc/config
```

## 🌱 Environment Variables

maxc provides environment variables to processes running inside surfaces.

```
MAXC_WORKSPACE_ID
MAXC_SURFACE_ID
MAXC_SOCKET_PATH
```

These variables enable automation scripts and integrations.

## 🧰 Technology Stack

maxc is built with modern systems technologies.

* Rust
* Tokio async runtime
* ConPTY terminal backend
* VTE terminal parser
* Chromium browser runtime
* Playwright automation driver
* Winit GPU based UI
* Clap CLI framework
* Serde JSON RPC

## 📋 Changelog

See [CHANGELOG.md](CHANGELOG.md) for a detailed history of all releases.

## 📚 Documentation

For detailed backend implementation, RPC usage, CLI commands, operations, testing, and development workflow, see [docs/README.md](docs/README.md).

## 🤝 Contributing

Contributions are welcome from the community.

Ways to contribute:

* reporting issues
* submitting pull requests
* improving documentation
* suggesting new features

## 📜 License

maxc is released as **Free and Open Source Software (FOSS)**.

License: [Apache-2.0](LICENSE).

## ❤️ Final Words

maxc aims to redefine the developer terminal experience by turning it into a **programmable workspace capable of coordinating complex development workflows and AI driven automation**.

The future of development will involve collaboration between humans and intelligent tools.

**maxc is where that collaboration happens.**
