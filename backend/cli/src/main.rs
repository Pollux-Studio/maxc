use maxc_automation::{RpcId, RpcRequest};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COMMAND_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
enum Command {
    Health,
    Readiness {
        token: String,
    },
    Diagnostics {
        token: String,
    },
    SessionCreate,
    SessionRefresh {
        token: String,
    },
    SessionRevoke {
        token: String,
    },
    TerminalSpawn {
        token: String,
        workspace_id: String,
        surface_id: String,
        cols: u16,
        rows: u16,
    },
    TerminalInput {
        token: String,
        workspace_id: String,
        surface_id: String,
        terminal_session_id: String,
        input: String,
    },
    TerminalResize {
        token: String,
        workspace_id: String,
        surface_id: String,
        terminal_session_id: String,
        cols: u16,
        rows: u16,
    },
    TerminalHistory {
        token: String,
        workspace_id: String,
        surface_id: String,
        terminal_session_id: String,
        from_sequence: Option<u64>,
        max_events: Option<usize>,
    },
    TerminalKill {
        token: String,
        workspace_id: String,
        surface_id: String,
        terminal_session_id: String,
    },
    BrowserCreate {
        token: String,
        workspace_id: String,
        surface_id: String,
    },
    BrowserTabList {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
    },
    BrowserTabOpen {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        url: String,
    },
    BrowserTabFocus {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserTabClose {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserGoto {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        url: String,
    },
    BrowserReload {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserBack {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserForward {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserHistory {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        from_sequence: Option<u64>,
        max_events: Option<usize>,
    },
    BrowserClick {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        selector: String,
    },
    BrowserType {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        selector: String,
        text: String,
    },
    BrowserKey {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        key: String,
    },
    BrowserWait {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        selector: Option<String>,
        expression: Option<String>,
        timeout_ms: Option<u64>,
    },
    BrowserScreenshot {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserEvaluate {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        expression: String,
    },
    BrowserCookieGet {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserCookieSet {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        name: String,
        value: String,
    },
    BrowserStorageGet {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        key: String,
    },
    BrowserStorageSet {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        key: String,
        value: String,
    },
    BrowserNetworkIntercept {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserUpload {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        selector: String,
        path: String,
    },
    BrowserDownload {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        url: String,
    },
    BrowserTraceStart {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserTraceStop {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
    },
    BrowserClose {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
    },
    AgentWorkerCreate {
        token: String,
        workspace_id: String,
        surface_id: String,
    },
    AgentWorkerList {
        token: String,
        workspace_id: String,
        surface_id: String,
    },
    AgentWorkerGet {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
    },
    AgentWorkerClose {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
    },
    AgentTaskStart {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
        prompt: String,
    },
    AgentTaskList {
        token: String,
        workspace_id: String,
        surface_id: String,
    },
    AgentTaskGet {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_task_id: String,
        agent_worker_id: Option<String>,
    },
    AgentTaskCancel {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_task_id: String,
        reason: Option<String>,
    },
    AgentAttachTerminal {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
        terminal_session_id: String,
    },
    AgentDetachTerminal {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
    },
    AgentAttachBrowser {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
        browser_session_id: String,
    },
    AgentDetachBrowser {
        token: String,
        workspace_id: String,
        surface_id: String,
        agent_worker_id: String,
    },
    WorkspaceCreate {
        token: String,
        name: String,
        folder: Option<String>,
    },
    WorkspaceList {
        token: String,
    },
    WorkspaceUpdate {
        token: String,
        workspace_id: String,
        name: Option<String>,
        folder: Option<String>,
    },
    WorkspaceDelete {
        token: String,
        workspace_id: String,
    },
    WorkspaceLayout {
        token: String,
        workspace_id: String,
    },
    PaneCreate {
        token: String,
        workspace_id: String,
    },
    PaneSplit {
        token: String,
        pane_id: String,
        direction: String,
        ratio: Option<f64>,
    },
    PaneList {
        token: String,
        workspace_id: String,
    },
    PaneClose {
        token: String,
        pane_id: String,
    },
    PaneResize {
        token: String,
        pane_id: String,
        ratio: f64,
    },
    SurfaceCreate {
        token: String,
        pane_id: String,
        workspace_id: String,
        panel_type: Option<String>,
        title: Option<String>,
    },
    SurfaceList {
        token: String,
        workspace_id: String,
        pane_id: Option<String>,
    },
    SurfaceClose {
        token: String,
        surface_id: String,
    },
    SurfaceFocus {
        token: String,
        surface_id: String,
    },
    NotificationSend {
        token: String,
        title: String,
        body: Option<String>,
        level: Option<String>,
        source: Option<String>,
        workspace_id: Option<String>,
    },
    NotificationList {
        token: String,
        workspace_id: Option<String>,
        unread_only: Option<bool>,
        limit: Option<u64>,
    },
    NotificationClear {
        token: String,
        notification_id: Option<String>,
        workspace_id: Option<String>,
    },
    BrowserDetect {
        token: String,
    },
    /// High-level: spawn a terminal surface, run a command, print IDs.
    Run {
        token: String,
        workspace_id: Option<String>,
        command: String,
    },
    /// High-level: open a browser surface, navigate to a URL, print IDs.
    Open {
        token: String,
        workspace_id: Option<String>,
        url: String,
    },
}

const DEFAULT_SOCKET_PATH: &str = r"\\.\pipe\maxc-rpc";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = std::env::var("MAXC_SOCKET_PATH").unwrap_or(DEFAULT_SOCKET_PATH.to_string());
    let output = run_cli(
        std::env::args().skip(1).collect(),
        &NamedPipeTransport::new(&socket_path),
    )
    .await?;
    println!("{output}");
    Ok(())
}

trait RpcTransport {
    fn send(
        &self,
        request: RpcRequest,
    ) -> impl std::future::Future<Output = Result<Value, Box<dyn std::error::Error>>> + Send;
}

struct NamedPipeTransport {
    #[cfg(windows)]
    pipe_name: String,
}

impl NamedPipeTransport {
    fn new(pipe_name: &str) -> Self {
        #[cfg(not(windows))]
        let _ = pipe_name;
        Self {
            #[cfg(windows)]
            pipe_name: pipe_name.to_string(),
        }
    }
}

impl RpcTransport for NamedPipeTransport {
    async fn send(&self, request: RpcRequest) -> Result<Value, Box<dyn std::error::Error>> {
        #[cfg(windows)]
        {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
            use tokio::net::windows::named_pipe::ClientOptions;

            let stream = ClientOptions::new().open(&self.pipe_name)?;
            let (read_half, mut write_half) = tokio::io::split(stream);
            write_half
                .write_all(format!("{}\n", serde_json::to_string(&request)?).as_bytes())
                .await?;
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            Ok(serde_json::from_str(line.trim_end())?)
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            Err("named pipe transport is only supported on Windows".into())
        }
    }
}

async fn run_cli(
    args: Vec<String>,
    transport: &impl RpcTransport,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pretty, command) = parse_cli(args)?;

    // Multi-step convenience commands
    match &command {
        Command::Run {
            token,
            workspace_id,
            command: cmd,
        } => {
            return run_convenience_run(transport, token, workspace_id.as_deref(), cmd, pretty)
                .await
        }
        Command::Open {
            token,
            workspace_id,
            url,
        } => {
            return run_convenience_open(transport, token, workspace_id.as_deref(), url, pretty)
                .await
        }
        _ => {}
    }

    let response = transport.send(build_request(command)).await?;
    render_response(&response, pretty)
}

/// `maxc run "npm test"` — workspace.list → pane.list → surface.create → terminal.spawn → terminal.input
async fn run_convenience_run(
    transport: &impl RpcTransport,
    token: &str,
    workspace_id: Option<&str>,
    cmd: &str,
    pretty: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let ws_id = if let Some(id) = workspace_id {
        id.to_string()
    } else {
        let ws = transport
            .send(request(
                "workspace.list",
                Some(json!({ "auth": {"token": token} })),
            ))
            .await?;
        ws["result"]["workspaces"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|w| w["workspace_id"].as_str())
            .ok_or("no workspaces found; create one first")?
            .to_string()
    };

    // Find first pane
    let panes = transport
        .send(request(
            "pane.list",
            Some(json!({ "workspace_id": ws_id, "auth": {"token": token} })),
        ))
        .await?;
    let pane_id = panes["result"]["panes"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["pane_id"].as_str())
        .ok_or("no panes found in workspace")?
        .to_string();

    // Create surface
    let surface = transport
        .send(request(
            "surface.create",
            Some(json!({
                "command_id": command_id("run-surface"),
                "pane_id": pane_id,
                "workspace_id": ws_id,
                "panel_type": "terminal",
                "title": format!("$ {}", &cmd[..cmd.len().min(30)]),
                "auth": {"token": token}
            })),
        ))
        .await?;
    let surface_id = surface["result"]["surface_id"]
        .as_str()
        .ok_or("surface.create failed")?
        .to_string();

    // Spawn terminal
    let spawn = transport
        .send(request(
            "terminal.spawn",
            Some(json!({
                "command_id": command_id("run-spawn"),
                "workspace_id": ws_id,
                "surface_id": surface_id,
                "cols": 120, "rows": 34,
                "auth": {"token": token}
            })),
        ))
        .await?;
    let term_id = spawn["result"]["terminal_session_id"]
        .as_str()
        .ok_or("terminal.spawn failed")?
        .to_string();

    // Send command as input
    let input_text = if cmd.ends_with('\n') {
        cmd.to_string()
    } else {
        format!("{cmd}\n")
    };
    transport
        .send(request(
            "terminal.input",
            Some(json!({
                "command_id": command_id("run-input"),
                "workspace_id": ws_id,
                "surface_id": surface_id,
                "terminal_session_id": term_id,
                "input": input_text,
                "auth": {"token": token}
            })),
        ))
        .await?;

    let result = json!({
        "workspace_id": ws_id,
        "pane_id": pane_id,
        "surface_id": surface_id,
        "terminal_session_id": term_id,
        "command": cmd,
        "status": "running"
    });
    render_response(&json!({ "result": result }), pretty)
}

/// `maxc open https://localhost:3000` — workspace.list → pane.list → surface.create → browser.create → browser.tab.open → browser.goto
async fn run_convenience_open(
    transport: &impl RpcTransport,
    token: &str,
    workspace_id: Option<&str>,
    url: &str,
    pretty: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let ws_id = if let Some(id) = workspace_id {
        id.to_string()
    } else {
        let ws = transport
            .send(request(
                "workspace.list",
                Some(json!({ "auth": {"token": token} })),
            ))
            .await?;
        ws["result"]["workspaces"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|w| w["workspace_id"].as_str())
            .ok_or("no workspaces found; create one first")?
            .to_string()
    };

    // Find first pane
    let panes = transport
        .send(request(
            "pane.list",
            Some(json!({ "workspace_id": ws_id, "auth": {"token": token} })),
        ))
        .await?;
    let pane_id = panes["result"]["panes"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|p| p["pane_id"].as_str())
        .ok_or("no panes found in workspace")?
        .to_string();

    // Create surface
    let surface = transport
        .send(request(
            "surface.create",
            Some(json!({
                "command_id": command_id("open-surface"),
                "pane_id": pane_id,
                "workspace_id": ws_id,
                "panel_type": "browser",
                "title": url,
                "auth": {"token": token}
            })),
        ))
        .await?;
    let surface_id = surface["result"]["surface_id"]
        .as_str()
        .ok_or("surface.create failed")?
        .to_string();

    // Create browser
    let browser = transport
        .send(request(
            "browser.create",
            Some(json!({
                "command_id": command_id("open-browser"),
                "workspace_id": ws_id,
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ))
        .await?;
    let browser_session_id = browser["result"]["browser_session_id"]
        .as_str()
        .ok_or("browser.create failed")?
        .to_string();

    // Open tab
    let tab = transport
        .send(request(
            "browser.tab.open",
            Some(json!({
                "command_id": command_id("open-tab"),
                "workspace_id": ws_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "url": url,
                "auth": {"token": token}
            })),
        ))
        .await?;
    let tab_id = tab["result"]["browser_tab_id"]
        .as_str()
        .unwrap_or("")
        .to_string();

    // Navigate
    transport
        .send(request(
            "browser.goto",
            Some(json!({
                "command_id": command_id("open-goto"),
                "workspace_id": ws_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "url": url,
                "auth": {"token": token}
            })),
        ))
        .await?;

    let result = json!({
        "workspace_id": ws_id,
        "pane_id": pane_id,
        "surface_id": surface_id,
        "browser_session_id": browser_session_id,
        "tab_id": tab_id,
        "url": url,
        "status": "open"
    });
    render_response(&json!({ "result": result }), pretty)
}

fn render_response(response: &Value, pretty: bool) -> Result<String, Box<dyn std::error::Error>> {
    if pretty {
        Ok(serde_json::to_string_pretty(response)?)
    } else {
        Ok(serde_json::to_string(response)?)
    }
}

fn parse_cli(args: Vec<String>) -> Result<(bool, Command), Box<dyn std::error::Error>> {
    let mut args = args;
    let pretty = remove_flag(&mut args, "--pretty");
    if args.is_empty() {
        return Err("missing command".into());
    }
    let command = match args[0].as_str() {
        "health" => Command::Health,
        "readiness" => {
            let flags = parse_flags(&args[1..])?;
            Command::Readiness {
                token: resolve_token(&flags)?,
            }
        }
        "diagnostics" => {
            let flags = parse_flags(&args[1..])?;
            Command::Diagnostics {
                token: resolve_token(&flags)?,
            }
        }
        "session" => parse_session(&args[1..])?,
        "terminal" => parse_terminal(&args[1..])?,
        "browser" => parse_browser(&args[1..])?,
        "agent" => parse_agent(&args[1..])?,
        "workspace" => parse_workspace(&args[1..])?,
        "pane" => parse_pane(&args[1..])?,
        "surface" => parse_surface(&args[1..])?,
        "notification" => parse_notification(&args[1..])?,
        "notify" => parse_notify(&args[1..])?,
        "run" => parse_run(&args[1..])?,
        "open" => parse_open(&args[1..])?,
        _ => return Err(format!("unknown command: {}", args[0]).into()),
    };
    Ok((pretty, command))
}

fn parse_session(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing session subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "create" => Ok(Command::SessionCreate),
        "refresh" => Ok(Command::SessionRefresh {
            token: resolve_token(&flags)?,
        }),
        "revoke" => Ok(Command::SessionRevoke {
            token: resolve_token(&flags)?,
        }),
        _ => Err("unknown session subcommand".into()),
    }
}

fn parse_terminal(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing terminal subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "spawn" => Ok(Command::TerminalSpawn {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            cols: optional_parse(&flags, "--cols")?.unwrap_or(120),
            rows: optional_parse(&flags, "--rows")?.unwrap_or(30),
        }),
        "input" => Ok(Command::TerminalInput {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
            input: required(&flags, "--input")?,
        }),
        "resize" => Ok(Command::TerminalResize {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
            cols: required(&flags, "--cols")?.parse()?,
            rows: required(&flags, "--rows")?.parse()?,
        }),
        "history" => Ok(Command::TerminalHistory {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
            from_sequence: optional_parse(&flags, "--from-sequence")?,
            max_events: optional_parse(&flags, "--max-events")?,
        }),
        "kill" => Ok(Command::TerminalKill {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
        }),
        _ => Err("unknown terminal subcommand".into()),
    }
}

fn parse_browser(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing browser subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "create" => Ok(Command::BrowserCreate {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
        }),
        "tab-list" => Ok(Command::BrowserTabList {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
        }),
        "tab-open" => Ok(Command::BrowserTabOpen {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            url: required(&flags, "--url")?,
        }),
        "tab-focus" => Ok(Command::BrowserTabFocus {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "tab-close" => Ok(Command::BrowserTabClose {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "goto" => Ok(Command::BrowserGoto {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            url: required(&flags, "--url")?,
        }),
        "reload" => Ok(Command::BrowserReload {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "back" => Ok(Command::BrowserBack {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "forward" => Ok(Command::BrowserForward {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "history" => Ok(Command::BrowserHistory {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            from_sequence: optional_parse(&flags, "--from-sequence")?,
            max_events: optional_parse(&flags, "--max-events")?,
        }),
        "click" => Ok(Command::BrowserClick {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            selector: required(&flags, "--selector")?,
        }),
        "type" => Ok(Command::BrowserType {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            selector: required(&flags, "--selector")?,
            text: required(&flags, "--text")?,
        }),
        "key" => Ok(Command::BrowserKey {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            key: required(&flags, "--key")?,
        }),
        "wait" => Ok(Command::BrowserWait {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            selector: flags.get("--selector").cloned(),
            expression: flags.get("--expression").cloned(),
            timeout_ms: optional_parse(&flags, "--timeout-ms")?,
        }),
        "screenshot" => Ok(Command::BrowserScreenshot {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "evaluate" => Ok(Command::BrowserEvaluate {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            expression: required(&flags, "--expression")?,
        }),
        "cookie-get" => Ok(Command::BrowserCookieGet {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "cookie-set" => Ok(Command::BrowserCookieSet {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            name: required(&flags, "--name")?,
            value: required(&flags, "--value")?,
        }),
        "storage-get" => Ok(Command::BrowserStorageGet {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            key: required(&flags, "--key")?,
        }),
        "storage-set" => Ok(Command::BrowserStorageSet {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            key: required(&flags, "--key")?,
            value: required(&flags, "--value")?,
        }),
        "intercept" => Ok(Command::BrowserNetworkIntercept {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "upload" => Ok(Command::BrowserUpload {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            selector: required(&flags, "--selector")?,
            path: required(&flags, "--file")?,
        }),
        "download" => Ok(Command::BrowserDownload {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            url: required(&flags, "--url")?,
        }),
        "trace-start" => Ok(Command::BrowserTraceStart {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "trace-stop" => Ok(Command::BrowserTraceStop {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
        }),
        "close" => Ok(Command::BrowserClose {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            browser_session_id: required(&flags, "--browser-session-id")?,
        }),
        "detect" => Ok(Command::BrowserDetect {
            token: resolve_token(&flags)?,
        }),
        _ => Err("unknown browser subcommand".into()),
    }
}

fn parse_agent(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.len() < 2 {
        return Err("missing agent subcommand".into());
    }
    let flags = parse_flags(&args[2..])?;
    match (args[0].as_str(), args[1].as_str()) {
        ("worker", "create") => Ok(Command::AgentWorkerCreate {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
        }),
        ("worker", "list") => Ok(Command::AgentWorkerList {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
        }),
        ("worker", "get") => Ok(Command::AgentWorkerGet {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        ("worker", "close") => Ok(Command::AgentWorkerClose {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        ("task", "start") => Ok(Command::AgentTaskStart {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
            prompt: required(&flags, "--prompt")?,
        }),
        ("task", "list") => Ok(Command::AgentTaskList {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
        }),
        ("task", "get") => Ok(Command::AgentTaskGet {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_task_id: required(&flags, "--agent-task-id")?,
            agent_worker_id: flags.get("--agent-worker-id").cloned(),
        }),
        ("task", "cancel") => Ok(Command::AgentTaskCancel {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_task_id: required(&flags, "--agent-task-id")?,
            reason: flags.get("--reason").cloned(),
        }),
        ("attach", "terminal") => Ok(Command::AgentAttachTerminal {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
        }),
        ("detach", "terminal") => Ok(Command::AgentDetachTerminal {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        ("attach", "browser") => Ok(Command::AgentAttachBrowser {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
            browser_session_id: required(&flags, "--browser-session-id")?,
        }),
        ("detach", "browser") => Ok(Command::AgentDetachBrowser {
            token: resolve_token(&flags)?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: resolve_surface_id(&flags)?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        _ => Err("unknown agent subcommand".into()),
    }
}

fn parse_workspace(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing workspace subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "create" => Ok(Command::WorkspaceCreate {
            token: resolve_token(&flags)?,
            name: required(&flags, "--name")?,
            folder: flags.get("--folder").cloned(),
        }),
        "list" => Ok(Command::WorkspaceList {
            token: resolve_token(&flags)?,
        }),
        "update" => Ok(Command::WorkspaceUpdate {
            token: resolve_token(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
            name: flags.get("--name").cloned(),
            folder: flags.get("--folder").cloned(),
        }),
        "delete" => Ok(Command::WorkspaceDelete {
            token: resolve_token(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
        }),
        "layout" => Ok(Command::WorkspaceLayout {
            token: resolve_token(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
        }),
        _ => Err("unknown workspace subcommand".into()),
    }
}

fn parse_pane(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing pane subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "create" => Ok(Command::PaneCreate {
            token: resolve_token(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
        }),
        "split" => Ok(Command::PaneSplit {
            token: resolve_token(&flags)?,
            pane_id: resolve_pane_id(&flags)?,
            direction: required(&flags, "--direction")?,
            ratio: optional_parse(&flags, "--ratio")?,
        }),
        "list" => Ok(Command::PaneList {
            token: resolve_token(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
        }),
        "close" => Ok(Command::PaneClose {
            token: resolve_token(&flags)?,
            pane_id: resolve_pane_id(&flags)?,
        }),
        "resize" => Ok(Command::PaneResize {
            token: resolve_token(&flags)?,
            pane_id: resolve_pane_id(&flags)?,
            ratio: required(&flags, "--ratio")?.parse()?,
        }),
        _ => Err("unknown pane subcommand".into()),
    }
}

fn parse_surface(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing surface subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "create" => Ok(Command::SurfaceCreate {
            token: resolve_token(&flags)?,
            pane_id: resolve_pane_id(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
            panel_type: flags.get("--type").cloned(),
            title: flags.get("--title").cloned(),
        }),
        "list" => Ok(Command::SurfaceList {
            token: resolve_token(&flags)?,
            workspace_id: resolve_workspace_id(&flags)?,
            pane_id: flags.get("--pane-id").cloned(),
        }),
        "close" => Ok(Command::SurfaceClose {
            token: resolve_token(&flags)?,
            surface_id: resolve_surface_id(&flags)?,
        }),
        "focus" => Ok(Command::SurfaceFocus {
            token: resolve_token(&flags)?,
            surface_id: resolve_surface_id(&flags)?,
        }),
        _ => Err("unknown surface subcommand".into()),
    }
}

fn parse_notification(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("missing notification subcommand".into());
    }
    let flags = parse_flags(&args[1..])?;
    match args[0].as_str() {
        "list" => Ok(Command::NotificationList {
            token: resolve_token(&flags)?,
            workspace_id: flags.get("--workspace-id").cloned(),
            unread_only: optional_parse(&flags, "--unread-only")?,
            limit: optional_parse(&flags, "--limit")?,
        }),
        "clear" => Ok(Command::NotificationClear {
            token: resolve_token(&flags)?,
            notification_id: flags.get("--notification-id").cloned(),
            workspace_id: flags.get("--workspace-id").cloned(),
        }),
        "send" => Ok(Command::NotificationSend {
            token: resolve_token(&flags)?,
            title: required(&flags, "--title")?,
            body: flags.get("--body").cloned(),
            level: flags.get("--level").cloned(),
            source: flags.get("--source").cloned(),
            workspace_id: flags.get("--workspace-id").cloned(),
        }),
        _ => Err("unknown notification subcommand".into()),
    }
}

fn parse_notify(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    let flags = parse_flags(args)?;
    Ok(Command::NotificationSend {
        token: resolve_token(&flags)?,
        title: required(&flags, "--title")?,
        body: flags.get("--body").cloned(),
        level: flags.get("--level").cloned(),
        source: flags.get("--source").cloned(),
        workspace_id: flags.get("--workspace-id").cloned(),
    })
}

/// `maxc run "npm test"` — spawns a terminal surface and runs the command.
fn parse_run(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    // First positional arg is the command; remaining are flags.
    if args.is_empty() {
        return Err("usage: maxc run <command> [--token T] [--workspace-id WS]".into());
    }
    let command_str = args[0].clone();
    let flags = parse_flags(&args[1..])?;
    Ok(Command::Run {
        token: resolve_token(&flags)?,
        workspace_id: flags.get("--workspace-id").cloned(),
        command: command_str,
    })
}

/// `maxc open https://localhost:3000` — opens a browser surface and navigates.
fn parse_open(args: &[String]) -> Result<Command, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err("usage: maxc open <url> [--token T] [--workspace-id WS]".into());
    }
    let url = args[0].clone();
    let flags = parse_flags(&args[1..])?;
    Ok(Command::Open {
        token: resolve_token(&flags)?,
        workspace_id: flags.get("--workspace-id").cloned(),
        url,
    })
}

/// Resolve workspace ID: `--workspace-id` flag first, then `MAXC_WORKSPACE_ID` env var.
fn resolve_workspace_id(
    flags: &HashMap<String, String>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(id) = flags.get("--workspace-id") {
        return Ok(id.clone());
    }
    std::env::var("MAXC_WORKSPACE_ID")
        .map_err(|_| "missing --workspace-id (or set MAXC_WORKSPACE_ID env var)".into())
}

fn resolve_surface_id(
    flags: &HashMap<String, String>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(id) = flags.get("--surface-id") {
        return Ok(id.clone());
    }
    std::env::var("MAXC_SURFACE_ID")
        .map_err(|_| "missing --surface-id (or set MAXC_SURFACE_ID env var)".into())
}

fn resolve_pane_id(flags: &HashMap<String, String>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(id) = flags.get("--pane-id") {
        return Ok(id.clone());
    }
    std::env::var("MAXC_PANE_ID")
        .map_err(|_| "missing --pane-id (or set MAXC_PANE_ID env var)".into())
}

fn parse_flags(args: &[String]) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut flags = HashMap::new();
    let mut idx = 0;
    while idx < args.len() {
        let key = &args[idx];
        if !key.starts_with("--") {
            return Err(format!("unexpected argument: {key}").into());
        }
        let value = args
            .get(idx + 1)
            .ok_or_else(|| format!("missing value for {key}"))?;
        flags.insert(key.clone(), value.clone());
        idx += 2;
    }
    Ok(flags)
}

fn remove_flag(args: &mut Vec<String>, flag: &str) -> bool {
    if let Some(index) = args.iter().position(|value| value == flag) {
        args.remove(index);
        true
    } else {
        false
    }
}

fn required(
    flags: &HashMap<String, String>,
    key: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    flags
        .get(key)
        .cloned()
        .ok_or_else(|| format!("missing {key}").into())
}

/// Resolve the auth token: `--token` flag first, then `MAXC_TOKEN` env var.
fn resolve_token(flags: &HashMap<String, String>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(token) = flags.get("--token") {
        return Ok(token.clone());
    }
    std::env::var("MAXC_TOKEN").map_err(|_| "missing --token (or set MAXC_TOKEN env var)".into())
}

fn optional_parse<T: std::str::FromStr>(
    flags: &HashMap<String, String>,
    key: &str,
) -> Result<Option<T>, Box<dyn std::error::Error>>
where
    <T as std::str::FromStr>::Err: std::error::Error + 'static,
{
    match flags.get(key) {
        Some(value) => Ok(Some(value.parse()?)),
        None => Ok(None),
    }
}

fn build_request(command: Command) -> RpcRequest {
    match command {
        Command::Health => request("system.health", None),
        Command::Readiness { token } => request(
            "system.readiness",
            Some(auth_payload(&token, "system-readiness")),
        ),
        Command::Diagnostics { token } => request(
            "system.diagnostics",
            Some(auth_payload(&token, "system-diagnostics")),
        ),
        Command::SessionCreate => request(
            "session.create",
            Some(json!({"command_id": command_id("session-create")})),
        ),
        Command::SessionRefresh { token } => request(
            "session.refresh",
            Some(json!({"command_id": command_id("session-refresh"), "auth": {"token": token}})),
        ),
        Command::SessionRevoke { token } => request(
            "session.revoke",
            Some(json!({"command_id": command_id("session-revoke"), "auth": {"token": token}})),
        ),
        Command::TerminalSpawn {
            token,
            workspace_id,
            surface_id,
            cols,
            rows,
        } => request(
            "terminal.spawn",
            Some(json!({
                "command_id": command_id("terminal-spawn"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "cols": cols,
                "rows": rows,
                "auth": {"token": token}
            })),
        ),
        Command::TerminalInput {
            token,
            workspace_id,
            surface_id,
            terminal_session_id,
            input,
        } => request(
            "terminal.input",
            Some(json!({
                "command_id": command_id("terminal-input"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "terminal_session_id": terminal_session_id,
                "input": input,
                "auth": {"token": token}
            })),
        ),
        Command::TerminalResize {
            token,
            workspace_id,
            surface_id,
            terminal_session_id,
            cols,
            rows,
        } => request(
            "terminal.resize",
            Some(json!({
                "command_id": command_id("terminal-resize"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "terminal_session_id": terminal_session_id,
                "cols": cols,
                "rows": rows,
                "auth": {"token": token}
            })),
        ),
        Command::TerminalHistory {
            token,
            workspace_id,
            surface_id,
            terminal_session_id,
            from_sequence,
            max_events,
        } => request(
            "terminal.history",
            Some(json!({
                "command_id": command_id("terminal-history"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "terminal_session_id": terminal_session_id,
                "from_sequence": from_sequence,
                "max_events": max_events,
                "auth": {"token": token}
            })),
        ),
        Command::TerminalKill {
            token,
            workspace_id,
            surface_id,
            terminal_session_id,
        } => request(
            "terminal.kill",
            Some(json!({
                "command_id": command_id("terminal-kill"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "terminal_session_id": terminal_session_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserCreate {
            token,
            workspace_id,
            surface_id,
        } => request(
            "browser.create",
            Some(json!({
                "command_id": command_id("browser-create"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserTabList {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
        } => request(
            "browser.tab.list",
            Some(json!({
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserTabOpen {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            url,
        } => request(
            "browser.tab.open",
            Some(json!({
                "command_id": command_id("browser-tab-open"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "url": url,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserTabFocus {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.tab.focus",
            Some(json!({
                "command_id": command_id("browser-tab-focus"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserTabClose {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.tab.close",
            Some(json!({
                "command_id": command_id("browser-tab-close"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserGoto {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            url,
        } => request(
            "browser.goto",
            Some(json!({
                "command_id": command_id("browser-goto"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "url": url,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserReload {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.reload",
            Some(json!({
                "command_id": command_id("browser-reload"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserBack {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.back",
            Some(json!({
                "command_id": command_id("browser-back"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserForward {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.forward",
            Some(json!({
                "command_id": command_id("browser-forward"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserClose {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
        } => request(
            "browser.close",
            Some(json!({
                "command_id": command_id("browser-close"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserDetect { token } => request(
            "system.browsers",
            Some(json!({
                "auth": {"token": token}
            })),
        ),
        Command::BrowserHistory {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            from_sequence,
            max_events,
        } => request(
            "browser.history",
            Some(json!({
                "command_id": command_id("browser-history"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "from_sequence": from_sequence,
                "max_events": max_events,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserClick {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            selector,
        } => request(
            "browser.click",
            Some(json!({
                "command_id": command_id("browser-click"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "selector": selector,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserType {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            selector,
            text,
        } => request(
            "browser.type",
            Some(json!({
                "command_id": command_id("browser-type"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "selector": selector,
                "text": text,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserKey {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            key,
        } => request(
            "browser.key",
            Some(json!({
                "command_id": command_id("browser-key"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "key": key,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserWait {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            selector,
            expression,
            timeout_ms,
        } => request(
            "browser.wait",
            Some(json!({
                "command_id": command_id("browser-wait"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "selector": selector,
                "expression": expression,
                "timeout_ms": timeout_ms,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserScreenshot {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.screenshot",
            Some(json!({
                "command_id": command_id("browser-screenshot"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserEvaluate {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            expression,
        } => request(
            "browser.evaluate",
            Some(json!({
                "command_id": command_id("browser-evaluate"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "expression": expression,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserCookieGet {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.cookie.get",
            Some(json!({
                "command_id": command_id("browser-cookie-get"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserCookieSet {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            name,
            value,
        } => request(
            "browser.cookie.set",
            Some(json!({
                "command_id": command_id("browser-cookie-set"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "name": name,
                "value": value,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserStorageGet {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            key,
        } => request(
            "browser.storage.get",
            Some(json!({
                "command_id": command_id("browser-storage-get"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "key": key,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserStorageSet {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            key,
            value,
        } => request(
            "browser.storage.set",
            Some(json!({
                "command_id": command_id("browser-storage-set"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "key": key,
                "value": value,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserNetworkIntercept {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.network.intercept",
            Some(json!({
                "command_id": command_id("browser-intercept"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserUpload {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            selector,
            path,
        } => request(
            "browser.upload",
            Some(json!({
                "command_id": command_id("browser-upload"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "selector": selector,
                "path": path,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserDownload {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
            url,
        } => request(
            "browser.download",
            Some(json!({
                "command_id": command_id("browser-download"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "url": url,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserTraceStart {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.trace.start",
            Some(json!({
                "command_id": command_id("browser-trace-start"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::BrowserTraceStop {
            token,
            workspace_id,
            surface_id,
            browser_session_id,
            tab_id,
        } => request(
            "browser.trace.stop",
            Some(json!({
                "command_id": command_id("browser-trace-stop"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentWorkerCreate {
            token,
            workspace_id,
            surface_id,
        } => request(
            "agent.worker.create",
            Some(json!({
                "command_id": command_id("agent-worker-create"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentWorkerList {
            token,
            workspace_id,
            surface_id,
        } => request(
            "agent.worker.list",
            Some(json!({
                "command_id": command_id("agent-worker-list"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentWorkerGet {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
        } => request(
            "agent.worker.get",
            Some(json!({
                "command_id": command_id("agent-worker-get"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentWorkerClose {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
        } => request(
            "agent.worker.close",
            Some(json!({
                "command_id": command_id("agent-worker-close"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentTaskStart {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
            prompt,
        } => request(
            "agent.task.start",
            Some(json!({
                "command_id": command_id("agent-task-start"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "prompt": prompt,
                "auth": {"token": token}
            })),
        ),
        Command::AgentTaskList {
            token,
            workspace_id,
            surface_id,
        } => request(
            "agent.task.list",
            Some(json!({
                "command_id": command_id("agent-task-list"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentTaskGet {
            token,
            workspace_id,
            surface_id,
            agent_task_id,
            agent_worker_id,
        } => request(
            "agent.task.get",
            Some(json!({
                "command_id": command_id("agent-task-get"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_task_id": agent_task_id,
                "agent_worker_id": agent_worker_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentTaskCancel {
            token,
            workspace_id,
            surface_id,
            agent_task_id,
            reason,
        } => request(
            "agent.task.cancel",
            Some(json!({
                "command_id": command_id("agent-task-cancel"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_task_id": agent_task_id,
                "reason": reason,
                "auth": {"token": token}
            })),
        ),
        Command::AgentAttachTerminal {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
            terminal_session_id,
        } => request(
            "agent.attach.terminal",
            Some(json!({
                "command_id": command_id("agent-attach-terminal"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "terminal_session_id": terminal_session_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentDetachTerminal {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
        } => request(
            "agent.detach.terminal",
            Some(json!({
                "command_id": command_id("agent-detach-terminal"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentAttachBrowser {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
            browser_session_id,
        } => request(
            "agent.attach.browser",
            Some(json!({
                "command_id": command_id("agent-attach-browser"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "browser_session_id": browser_session_id,
                "auth": {"token": token}
            })),
        ),
        Command::AgentDetachBrowser {
            token,
            workspace_id,
            surface_id,
            agent_worker_id,
        } => request(
            "agent.detach.browser",
            Some(json!({
                "command_id": command_id("agent-detach-browser"),
                "workspace_id": workspace_id,
                "surface_id": surface_id,
                "agent_worker_id": agent_worker_id,
                "auth": {"token": token}
            })),
        ),
        Command::WorkspaceCreate {
            token,
            name,
            folder,
        } => request(
            "workspace.create",
            Some(json!({
                "command_id": command_id("workspace-create"),
                "name": name,
                "folder": folder.unwrap_or_default(),
                "env_vars": {},
                "auth": {"token": token}
            })),
        ),
        Command::WorkspaceList { token } => request(
            "workspace.list",
            Some(json!({
                "auth": {"token": token}
            })),
        ),
        Command::WorkspaceUpdate {
            token,
            workspace_id,
            name,
            folder,
        } => {
            let mut params = json!({
                "command_id": command_id("workspace-update"),
                "workspace_id": workspace_id,
                "auth": {"token": token}
            });
            if let Some(n) = name {
                params["name"] = json!(n);
            }
            if let Some(f) = folder {
                params["folder"] = json!(f);
            }
            request("workspace.update", Some(params))
        }
        Command::WorkspaceDelete {
            token,
            workspace_id,
        } => request(
            "workspace.delete",
            Some(json!({
                "command_id": command_id("workspace-delete"),
                "workspace_id": workspace_id,
                "auth": {"token": token}
            })),
        ),
        Command::WorkspaceLayout {
            token,
            workspace_id,
        } => request(
            "workspace.layout",
            Some(json!({
                "workspace_id": workspace_id,
                "auth": {"token": token}
            })),
        ),
        Command::PaneCreate {
            token,
            workspace_id,
        } => request(
            "pane.create",
            Some(json!({
                "command_id": command_id("pane-create"),
                "workspace_id": workspace_id,
                "auth": {"token": token}
            })),
        ),
        Command::PaneSplit {
            token,
            pane_id,
            direction,
            ratio,
        } => request(
            "pane.split",
            Some(json!({
                "command_id": command_id("pane-split"),
                "pane_id": pane_id,
                "direction": direction,
                "ratio": ratio.unwrap_or(0.5),
                "auth": {"token": token}
            })),
        ),
        Command::PaneList {
            token,
            workspace_id,
        } => request(
            "pane.list",
            Some(json!({
                "workspace_id": workspace_id,
                "auth": {"token": token}
            })),
        ),
        Command::PaneClose { token, pane_id } => request(
            "pane.close",
            Some(json!({
                "command_id": command_id("pane-close"),
                "pane_id": pane_id,
                "auth": {"token": token}
            })),
        ),
        Command::PaneResize {
            token,
            pane_id,
            ratio,
        } => request(
            "pane.resize",
            Some(json!({
                "command_id": command_id("pane-resize"),
                "pane_id": pane_id,
                "ratio": ratio,
                "auth": {"token": token}
            })),
        ),
        Command::SurfaceCreate {
            token,
            pane_id,
            workspace_id,
            panel_type,
            title,
        } => request(
            "surface.create",
            Some(json!({
                "command_id": command_id("surface-create"),
                "pane_id": pane_id,
                "workspace_id": workspace_id,
                "panel_type": panel_type.unwrap_or("terminal".to_string()),
                "title": title.unwrap_or_default(),
                "auth": {"token": token}
            })),
        ),
        Command::SurfaceList {
            token,
            workspace_id,
            pane_id,
        } => request(
            "surface.list",
            Some(json!({
                "workspace_id": workspace_id,
                "pane_id": pane_id,
                "auth": {"token": token}
            })),
        ),
        Command::SurfaceClose { token, surface_id } => request(
            "surface.close",
            Some(json!({
                "command_id": command_id("surface-close"),
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ),
        Command::SurfaceFocus { token, surface_id } => request(
            "surface.focus",
            Some(json!({
                "command_id": command_id("surface-focus"),
                "surface_id": surface_id,
                "auth": {"token": token}
            })),
        ),
        Command::NotificationSend {
            token,
            title,
            body,
            level,
            source,
            workspace_id,
        } => request(
            "notification.send",
            Some(json!({
                "command_id": command_id("notification-send"),
                "title": title,
                "body": body,
                "level": level,
                "source": source,
                "workspace_id": workspace_id,
                "auth": {"token": token}
            })),
        ),
        Command::NotificationList {
            token,
            workspace_id,
            unread_only,
            limit,
        } => request(
            "notification.list",
            Some(json!({
                "workspace_id": workspace_id,
                "unread_only": unread_only,
                "limit": limit,
                "auth": {"token": token}
            })),
        ),
        Command::NotificationClear {
            token,
            notification_id,
            workspace_id,
        } => request(
            "notification.clear",
            Some(json!({
                "command_id": command_id("notification-clear"),
                "notification_id": notification_id,
                "workspace_id": workspace_id,
                "auth": {"token": token}
            })),
        ),
        // Run and Open are handled in run_cli before build_request is called.
        Command::Run { .. } | Command::Open { .. } => {
            unreachable!("Run/Open are handled as multi-step commands in run_cli")
        }
    }
}

fn request(method: &str, params: Option<Value>) -> RpcRequest {
    RpcRequest {
        id: Some(RpcId::String(command_id(method))),
        method: method.to_string(),
        params,
    }
}

fn auth_payload(token: &str, command: &str) -> Value {
    json!({
        "command_id": command_id(command),
        "auth": {"token": token}
    })
}

fn command_id(prefix: &str) -> String {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis();
    let counter = COMMAND_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("cli-{prefix}-{timestamp_ms}-{counter}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use maxc_automation::RpcServer;
    use maxc_core::BackendConfig;

    struct InProcessTransport {
        server: RpcServer,
    }

    impl RpcTransport for InProcessTransport {
        async fn send(&self, request: RpcRequest) -> Result<Value, Box<dyn std::error::Error>> {
            let raw = self
                .server
                .handle_json_line("cli-test", &serde_json::to_string(&request)?)
                .await;
            Ok(serde_json::from_str(&raw)?)
        }
    }

    struct StaticTransport {
        response: Value,
    }

    impl RpcTransport for StaticTransport {
        async fn send(&self, _request: RpcRequest) -> Result<Value, Box<dyn std::error::Error>> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn parse_and_build_health_request() {
        let (_, command) = parse_cli(vec!["health".to_string()]).expect("parse");
        let req = build_request(command);
        assert_eq!(req.method, "system.health");
    }

    #[test]
    fn parse_terminal_spawn_flags() {
        let (_, command) = parse_cli(vec![
            "terminal".to_string(),
            "spawn".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
        ])
        .expect("parse");
        let req = build_request(command);
        assert_eq!(req.method, "terminal.spawn");
    }

    #[test]
    fn parse_terminal_history_flags() {
        let (_, command) = parse_cli(vec![
            "terminal".to_string(),
            "history".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--terminal-session-id".to_string(),
            "ts-1".to_string(),
            "--from-sequence".to_string(),
            "5".to_string(),
            "--max-events".to_string(),
            "10".to_string(),
        ])
        .expect("parse");
        let req = build_request(command);
        assert_eq!(req.method, "terminal.history");
        let params = req.params.expect("params");
        assert_eq!(params["from_sequence"], 5);
        assert_eq!(params["max_events"], 10);
    }

    #[test]
    fn parse_session_browser_and_pretty_commands() {
        let (pretty, session_create) = parse_cli(vec![
            "--pretty".to_string(),
            "session".to_string(),
            "create".to_string(),
        ])
        .expect("session create");
        assert!(pretty);
        assert!(matches!(session_create, Command::SessionCreate));

        let (_, refresh) = parse_cli(vec![
            "session".to_string(),
            "refresh".to_string(),
            "--token".to_string(),
            "tok".to_string(),
        ])
        .expect("session refresh");
        let refresh_req = build_request(refresh);
        assert_eq!(refresh_req.method, "session.refresh");
        assert_eq!(refresh_req.params.expect("params")["auth"]["token"], "tok");

        let (_, revoke) = parse_cli(vec![
            "session".to_string(),
            "revoke".to_string(),
            "--token".to_string(),
            "tok".to_string(),
        ])
        .expect("session revoke");
        assert_eq!(build_request(revoke).method, "session.revoke");

        let (_, browser_create) = parse_cli(vec![
            "browser".to_string(),
            "create".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
        ])
        .expect("browser create");
        assert_eq!(build_request(browser_create).method, "browser.create");

        let (_, detect) = parse_cli(vec![
            "browser".to_string(),
            "detect".to_string(),
            "--token".to_string(),
            "tok".to_string(),
        ])
        .expect("browser detect");
        assert_eq!(build_request(detect).method, "system.browsers");

        let (_, tab_open) = parse_cli(vec![
            "browser".to_string(),
            "tab-open".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--browser-session-id".to_string(),
            "bs-1".to_string(),
            "--url".to_string(),
            "https://example.com".to_string(),
        ])
        .expect("browser tab open");
        let tab_open_req = build_request(tab_open);
        assert_eq!(tab_open_req.method, "browser.tab.open");

        let (_, goto) = parse_cli(vec![
            "browser".to_string(),
            "goto".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--browser-session-id".to_string(),
            "bs-1".to_string(),
            "--tab-id".to_string(),
            "tab-1".to_string(),
            "--url".to_string(),
            "https://example.com/next".to_string(),
        ])
        .expect("browser goto");
        let goto_req = build_request(goto);
        assert_eq!(goto_req.method, "browser.goto");
        assert_eq!(goto_req.params.expect("params")["tab_id"], "tab-1");

        let (_, close) = parse_cli(vec![
            "browser".to_string(),
            "close".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--browser-session-id".to_string(),
            "bs-1".to_string(),
        ])
        .expect("browser close");
        assert_eq!(build_request(close).method, "browser.close");

        let (_, history) = parse_cli(vec![
            "browser".to_string(),
            "history".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--browser-session-id".to_string(),
            "bs-1".to_string(),
        ])
        .expect("browser history");
        assert_eq!(build_request(history).method, "browser.history");
    }

    #[test]
    fn parse_terminal_input_resize_and_kill_commands() {
        let (_, input) = parse_cli(vec![
            "terminal".to_string(),
            "input".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--terminal-session-id".to_string(),
            "ts-1".to_string(),
            "--input".to_string(),
            "echo hi".to_string(),
        ])
        .expect("terminal input");
        let input_req = build_request(input);
        assert_eq!(input_req.method, "terminal.input");
        assert_eq!(input_req.params.expect("params")["input"], "echo hi");

        let (_, resize) = parse_cli(vec![
            "terminal".to_string(),
            "resize".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--terminal-session-id".to_string(),
            "ts-1".to_string(),
            "--cols".to_string(),
            "140".to_string(),
            "--rows".to_string(),
            "50".to_string(),
        ])
        .expect("terminal resize");
        let resize_req = build_request(resize);
        assert_eq!(resize_req.method, "terminal.resize");
        let resize_params = resize_req.params.expect("params");
        assert_eq!(resize_params["cols"], 140);
        assert_eq!(resize_params["rows"], 50);

        let (_, kill) = parse_cli(vec![
            "terminal".to_string(),
            "kill".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
            "--terminal-session-id".to_string(),
            "ts-1".to_string(),
        ])
        .expect("terminal kill");
        assert_eq!(build_request(kill).method, "terminal.kill");
    }

    #[test]
    fn parse_agent_and_browser_history_commands() {
        let cases = vec![
            vec![
                "browser",
                "history",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--browser-session-id",
                "bs-1",
                "--from-sequence",
                "7",
            ],
            vec![
                "agent",
                "worker",
                "create",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
            ],
            vec![
                "agent",
                "worker",
                "list",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
            ],
            vec![
                "agent",
                "worker",
                "get",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
            ],
            vec![
                "agent",
                "worker",
                "close",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
            ],
            vec![
                "agent",
                "task",
                "start",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
                "--prompt",
                "run tests",
            ],
            vec![
                "agent",
                "task",
                "list",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
            ],
            vec![
                "agent",
                "task",
                "get",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-task-id",
                "at-1",
                "--agent-worker-id",
                "aw-1",
            ],
            vec![
                "agent",
                "task",
                "cancel",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-task-id",
                "at-1",
                "--reason",
                "cancel",
            ],
            vec![
                "agent",
                "attach",
                "terminal",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
                "--terminal-session-id",
                "ts-1",
            ],
            vec![
                "agent",
                "detach",
                "terminal",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
            ],
            vec![
                "agent",
                "attach",
                "browser",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
                "--browser-session-id",
                "bs-1",
            ],
            vec![
                "agent",
                "detach",
                "browser",
                "--token",
                "tok",
                "--workspace-id",
                "ws-1",
                "--surface-id",
                "sf-1",
                "--agent-worker-id",
                "aw-1",
            ],
        ];

        for args in cases {
            let (_, command) =
                parse_cli(args.into_iter().map(ToString::to_string).collect()).expect("parse");
            let request = build_request(command);
            assert!(request.method.starts_with("agent.") || request.method == "browser.history");
            assert!(request.params.is_some());
        }
    }

    #[test]
    fn parse_workspace_commands() {
        let (_, create) = parse_cli(vec![
            "workspace".to_string(),
            "create".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--name".to_string(),
            "my-project".to_string(),
            "--folder".to_string(),
            "/home/user/project".to_string(),
        ])
        .expect("workspace create");
        let req = build_request(create);
        assert_eq!(req.method, "workspace.create");
        let params = req.params.expect("params");
        assert_eq!(params["name"], "my-project");
        assert_eq!(params["folder"], "/home/user/project");
        assert_eq!(params["auth"]["token"], "tok");

        let (_, list) = parse_cli(vec![
            "workspace".to_string(),
            "list".to_string(),
            "--token".to_string(),
            "tok".to_string(),
        ])
        .expect("workspace list");
        let req = build_request(list);
        assert_eq!(req.method, "workspace.list");
        assert_eq!(req.params.expect("params")["auth"]["token"], "tok");

        // workspace create without folder is valid (folder is optional)
        let (_, create_no_folder) = parse_cli(vec![
            "workspace".to_string(),
            "create".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--name".to_string(),
            "bare".to_string(),
        ])
        .expect("workspace create no folder");
        let req = build_request(create_no_folder);
        assert_eq!(req.params.expect("params")["folder"], "");

        // workspace update
        let (_, update) = parse_cli(vec![
            "workspace".to_string(),
            "update".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--name".to_string(),
            "renamed".to_string(),
        ])
        .expect("workspace update");
        let req = build_request(update);
        assert_eq!(req.method, "workspace.update");
        let params = req.params.expect("params");
        assert_eq!(params["workspace_id"], "ws-1");
        assert_eq!(params["name"], "renamed");

        // workspace delete
        let (_, delete) = parse_cli(vec![
            "workspace".to_string(),
            "delete".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("workspace delete");
        assert_eq!(build_request(delete).method, "workspace.delete");

        // workspace layout
        let (_, layout) = parse_cli(vec![
            "workspace".to_string(),
            "layout".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("workspace layout");
        assert_eq!(build_request(layout).method, "workspace.layout");

        assert!(parse_workspace(&[]).is_err());
        assert!(parse_workspace(&["unknown".to_string()]).is_err());
    }

    #[test]
    fn parse_pane_commands() {
        let (_, create) = parse_cli(vec![
            "pane".to_string(),
            "create".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("pane create");
        let req = build_request(create);
        assert_eq!(req.method, "pane.create");
        assert_eq!(req.params.expect("params")["workspace_id"], "ws-1");

        let (_, split) = parse_cli(vec![
            "pane".to_string(),
            "split".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--pane-id".to_string(),
            "pane-1".to_string(),
            "--direction".to_string(),
            "vertical".to_string(),
            "--ratio".to_string(),
            "0.6".to_string(),
        ])
        .expect("pane split");
        let req = build_request(split);
        assert_eq!(req.method, "pane.split");
        let params = req.params.expect("params");
        assert_eq!(params["pane_id"], "pane-1");
        assert_eq!(params["direction"], "vertical");

        let (_, list) = parse_cli(vec![
            "pane".to_string(),
            "list".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("pane list");
        assert_eq!(build_request(list).method, "pane.list");

        let (_, close) = parse_cli(vec![
            "pane".to_string(),
            "close".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--pane-id".to_string(),
            "pane-1".to_string(),
        ])
        .expect("pane close");
        assert_eq!(build_request(close).method, "pane.close");

        let (_, resize) = parse_cli(vec![
            "pane".to_string(),
            "resize".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--pane-id".to_string(),
            "pane-1".to_string(),
            "--ratio".to_string(),
            "0.4".to_string(),
        ])
        .expect("pane resize");
        assert_eq!(build_request(resize).method, "pane.resize");

        assert!(parse_pane(&[]).is_err());
        assert!(parse_pane(&["unknown".to_string()]).is_err());
    }

    #[test]
    fn parse_surface_commands() {
        let (_, create) = parse_cli(vec![
            "surface".to_string(),
            "create".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--pane-id".to_string(),
            "pane-1".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--type".to_string(),
            "terminal".to_string(),
        ])
        .expect("surface create");
        let req = build_request(create);
        assert_eq!(req.method, "surface.create");
        let params = req.params.expect("params");
        assert_eq!(params["pane_id"], "pane-1");
        assert_eq!(params["panel_type"], "terminal");

        let (_, list) = parse_cli(vec![
            "surface".to_string(),
            "list".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("surface list");
        assert_eq!(build_request(list).method, "surface.list");

        let (_, close) = parse_cli(vec![
            "surface".to_string(),
            "close".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
        ])
        .expect("surface close");
        assert_eq!(build_request(close).method, "surface.close");

        let (_, focus) = parse_cli(vec![
            "surface".to_string(),
            "focus".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--surface-id".to_string(),
            "sf-1".to_string(),
        ])
        .expect("surface focus");
        assert_eq!(build_request(focus).method, "surface.focus");

        assert!(parse_surface(&[]).is_err());
        assert!(parse_surface(&["unknown".to_string()]).is_err());
    }

    #[test]
    fn parse_notification_commands() {
        let (_, list) = parse_cli(vec![
            "notification".to_string(),
            "list".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
            "--unread-only".to_string(),
            "true".to_string(),
            "--limit".to_string(),
            "50".to_string(),
        ])
        .expect("notification list");
        let req = build_request(list);
        assert_eq!(req.method, "notification.list");

        let (_, clear) = parse_cli(vec![
            "notification".to_string(),
            "clear".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("notification clear");
        let req = build_request(clear);
        assert_eq!(req.method, "notification.clear");

        let (_, send) = parse_cli(vec![
            "notification".to_string(),
            "send".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--title".to_string(),
            "Hello".to_string(),
            "--body".to_string(),
            "World".to_string(),
        ])
        .expect("notification send");
        let req = build_request(send);
        assert_eq!(req.method, "notification.send");

        let (_, notify) = parse_cli(vec![
            "notify".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--title".to_string(),
            "Hi".to_string(),
        ])
        .expect("notify");
        let req = build_request(notify);
        assert_eq!(req.method, "notification.send");
    }

    #[test]
    fn resolve_token_falls_back_to_env() {
        let flags = HashMap::new();
        // With no flag and no env, should fail
        std::env::remove_var("MAXC_TOKEN");
        assert!(resolve_token(&flags).is_err());

        // With env set, should succeed
        std::env::set_var("MAXC_TOKEN", "env-tok-123");
        let token = resolve_token(&flags).expect("env fallback");
        assert_eq!(token, "env-tok-123");
        std::env::remove_var("MAXC_TOKEN");

        // Flag takes precedence over env
        std::env::set_var("MAXC_TOKEN", "env-tok");
        let mut flags_with_token = HashMap::new();
        flags_with_token.insert("--token".to_string(), "flag-tok".to_string());
        let token = resolve_token(&flags_with_token).expect("flag precedence");
        assert_eq!(token, "flag-tok");
        std::env::remove_var("MAXC_TOKEN");
    }

    #[test]
    fn resolve_surface_and_pane_id_fall_back_to_env() {
        let flags = HashMap::new();
        std::env::remove_var("MAXC_SURFACE_ID");
        std::env::remove_var("MAXC_PANE_ID");
        assert!(resolve_surface_id(&flags).is_err());
        assert!(resolve_pane_id(&flags).is_err());

        std::env::set_var("MAXC_SURFACE_ID", "sf-env-1");
        std::env::set_var("MAXC_PANE_ID", "pane-env-1");
        assert_eq!(resolve_surface_id(&flags).expect("env"), "sf-env-1");
        assert_eq!(resolve_pane_id(&flags).expect("env"), "pane-env-1");
        std::env::remove_var("MAXC_SURFACE_ID");
        std::env::remove_var("MAXC_PANE_ID");

        let mut flags_with = HashMap::new();
        flags_with.insert("--surface-id".to_string(), "sf-flag".to_string());
        flags_with.insert("--pane-id".to_string(), "pane-flag".to_string());
        std::env::set_var("MAXC_SURFACE_ID", "sf-env");
        std::env::set_var("MAXC_PANE_ID", "pane-env");
        assert_eq!(
            resolve_surface_id(&flags_with).expect("flag wins"),
            "sf-flag"
        );
        assert_eq!(
            resolve_pane_id(&flags_with).expect("flag wins"),
            "pane-flag"
        );
        std::env::remove_var("MAXC_SURFACE_ID");
        std::env::remove_var("MAXC_PANE_ID");
    }

    #[test]
    fn parse_run_and_open_commands() {
        let (_, run) = parse_cli(vec![
            "run".to_string(),
            "npm test".to_string(),
            "--token".to_string(),
            "tok".to_string(),
        ])
        .expect("run");
        match run {
            Command::Run {
                token,
                command,
                workspace_id,
            } => {
                assert_eq!(token, "tok");
                assert_eq!(command, "npm test");
                assert!(workspace_id.is_none());
            }
            _ => panic!("expected Run"),
        }

        let (_, open) = parse_cli(vec![
            "open".to_string(),
            "https://localhost:3000".to_string(),
            "--token".to_string(),
            "tok".to_string(),
            "--workspace-id".to_string(),
            "ws-1".to_string(),
        ])
        .expect("open");
        match open {
            Command::Open {
                token,
                url,
                workspace_id,
            } => {
                assert_eq!(token, "tok");
                assert_eq!(url, "https://localhost:3000");
                assert_eq!(workspace_id, Some("ws-1".to_string()));
            }
            _ => panic!("expected Open"),
        }

        assert!(parse_run(&[]).is_err());
        assert!(parse_open(&[]).is_err());
    }

    #[test]
    fn parse_errors_and_helpers_are_stable() {
        assert!(parse_cli(vec![]).is_err());
        assert!(parse_cli(vec!["wat".to_string()]).is_err());
        assert!(parse_session(&[]).is_err());
        assert!(parse_terminal(&[]).is_err());
        assert!(parse_browser(&[]).is_err());
        assert!(parse_agent(&[]).is_err());
        assert!(parse_flags(&["value".to_string()]).is_err());
        assert!(parse_flags(&["--token".to_string()]).is_err());

        let mut args = vec!["--pretty".to_string(), "health".to_string()];
        assert!(remove_flag(&mut args, "--pretty"));
        assert!(!remove_flag(&mut args, "--pretty"));
        assert_eq!(args, vec!["health".to_string()]);

        let mut flags = HashMap::new();
        flags.insert("--value".to_string(), "42".to_string());
        assert_eq!(required(&flags, "--value").expect("required"), "42");
        assert!(required(&flags, "--missing").is_err());
        assert_eq!(
            optional_parse::<u16>(&flags, "--value").expect("optional"),
            Some(42)
        );
        assert_eq!(
            optional_parse::<u16>(&flags, "--missing").expect("optional missing"),
            None
        );
        assert!(optional_parse::<u16>(
            &HashMap::from([("--bad".to_string(), "x".to_string())]),
            "--bad"
        )
        .is_err());

        let auth = auth_payload("tok", "demo");
        assert_eq!(auth["auth"]["token"], "tok");
        let first = command_id("demo");
        let second = command_id("demo");
        assert!(first.starts_with("cli-demo-"));
        assert!(second.starts_with("cli-demo-"));
        assert_ne!(first, second);
        assert_eq!(request("system.health", None).method, "system.health");
        #[cfg(windows)]
        assert!(NamedPipeTransport::new("pipe-demo")
            .pipe_name
            .contains("pipe-demo"));
    }

    #[test]
    fn mutating_requests_get_fresh_command_ids() {
        let first = build_request(Command::SessionCreate);
        let second = build_request(Command::SessionCreate);
        let first_command = first
            .params
            .as_ref()
            .and_then(|params| params.get("command_id"))
            .and_then(Value::as_str)
            .expect("first command id");
        let second_command = second
            .params
            .as_ref()
            .and_then(|params| params.get("command_id"))
            .and_then(Value::as_str)
            .expect("second command id");
        assert_ne!(first_command, second_command);
        assert!(first_command.starts_with("cli-session-create-"));
        assert!(second_command.starts_with("cli-session-create-"));
    }

    #[tokio::test]
    async fn run_cli_renders_pretty_and_compact_output() {
        let transport = StaticTransport {
            response: json!({"result": {"ok": true}}),
        };
        let compact = run_cli(vec!["health".to_string()], &transport)
            .await
            .expect("compact");
        assert_eq!(compact, "{\"result\":{\"ok\":true}}");

        let pretty = run_cli(
            vec!["--pretty".to_string(), "health".to_string()],
            &transport,
        )
        .await
        .expect("pretty");
        assert!(pretty.contains('\n'));
        assert!(render_response(&json!({"ok": true}), false)
            .expect("render")
            .contains("\"ok\":true"));
    }

    #[tokio::test]
    async fn cli_smoke_flows_against_in_process_server() {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_millis();
        let cfg = BackendConfig {
            event_dir: std::env::temp_dir()
                .join(format!("maxc-cli-smoke-{millis}"))
                .to_string_lossy()
                .to_string(),
            ..BackendConfig::default()
        };
        let transport = InProcessTransport {
            server: RpcServer::new(cfg).expect("server"),
        };

        let created = transport
            .send(build_request(Command::SessionCreate))
            .await
            .expect("session create");
        let token = created["result"]["token"]
            .as_str()
            .expect("token")
            .to_string();

        let health = transport
            .send(build_request(Command::Health))
            .await
            .expect("health");
        assert_eq!(health["result"]["ok"], true);

        // workspace commands
        let ws_created = transport
            .send(build_request(Command::WorkspaceCreate {
                token: token.clone(),
                name: "cli-test-ws".to_string(),
                folder: Some("/tmp/test".to_string()),
            }))
            .await
            .expect("workspace create");
        assert!(ws_created["result"]["workspace_id"].is_string());
        assert_eq!(ws_created["result"]["name"], "cli-test-ws");

        let ws_list = transport
            .send(build_request(Command::WorkspaceList {
                token: token.clone(),
            }))
            .await
            .expect("workspace list");
        let workspaces = ws_list["result"]["workspaces"].as_array().expect("array");
        assert!(!workspaces.is_empty());

        // workspace update
        let ws_id = ws_created["result"]["workspace_id"]
            .as_str()
            .expect("ws id")
            .to_string();
        let ws_updated = transport
            .send(build_request(Command::WorkspaceUpdate {
                token: token.clone(),
                workspace_id: ws_id.clone(),
                name: Some("renamed-ws".to_string()),
                folder: None,
            }))
            .await
            .expect("workspace update");
        assert_eq!(ws_updated["result"]["updated"], true);
        assert_eq!(ws_updated["result"]["name"], "renamed-ws");

        // workspace layout
        let ws_layout = transport
            .send(build_request(Command::WorkspaceLayout {
                token: token.clone(),
                workspace_id: ws_id.clone(),
            }))
            .await
            .expect("workspace layout");
        assert!(ws_layout["result"]["layout"].is_object());
        assert_eq!(ws_layout["result"]["workspace_id"], ws_id);

        // workspace.create should auto-create a root pane
        let root_pane_id = ws_created["result"]["root_pane_id"]
            .as_str()
            .expect("root pane id")
            .to_string();

        // pane commands
        let pane_list = transport
            .send(build_request(Command::PaneList {
                token: token.clone(),
                workspace_id: ws_created["result"]["workspace_id"]
                    .as_str()
                    .expect("ws id")
                    .to_string(),
            }))
            .await
            .expect("pane list");
        let panes = pane_list["result"]["panes"]
            .as_array()
            .expect("panes array");
        assert!(!panes.is_empty());

        let split = transport
            .send(build_request(Command::PaneSplit {
                token: token.clone(),
                pane_id: root_pane_id.clone(),
                direction: "vertical".to_string(),
                ratio: Some(0.5),
            }))
            .await
            .expect("pane split");
        let child_a = split["result"]["child_a"]
            .as_str()
            .expect("child a")
            .to_string();

        // surface commands
        let surface = transport
            .send(build_request(Command::SurfaceCreate {
                token: token.clone(),
                pane_id: child_a.clone(),
                workspace_id: ws_created["result"]["workspace_id"]
                    .as_str()
                    .expect("ws id")
                    .to_string(),
                panel_type: Some("terminal".to_string()),
                title: Some("Test Terminal".to_string()),
            }))
            .await
            .expect("surface create");
        let surface_id = surface["result"]["surface_id"]
            .as_str()
            .expect("surface id")
            .to_string();

        let surface_list = transport
            .send(build_request(Command::SurfaceList {
                token: token.clone(),
                workspace_id: ws_created["result"]["workspace_id"]
                    .as_str()
                    .expect("ws id")
                    .to_string(),
                pane_id: Some(child_a),
            }))
            .await
            .expect("surface list");
        let surfaces = surface_list["result"]["surfaces"]
            .as_array()
            .expect("surfaces array");
        assert!(!surfaces.is_empty());

        let focused = transport
            .send(build_request(Command::SurfaceFocus {
                token: token.clone(),
                surface_id: surface_id.clone(),
            }))
            .await
            .expect("surface focus");
        assert_eq!(focused["result"]["focused"], true);

        let closed_surface = transport
            .send(build_request(Command::SurfaceClose {
                token: token.clone(),
                surface_id,
            }))
            .await
            .expect("surface close");
        assert_eq!(closed_surface["result"]["closed"], true);

        let readiness = transport
            .send(build_request(Command::Readiness {
                token: token.clone(),
            }))
            .await
            .expect("readiness");
        assert!(readiness["result"]["ready"].is_boolean());

        let browser = transport
            .send(build_request(Command::BrowserCreate {
                token: token.clone(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-1".to_string(),
            }))
            .await
            .expect("browser create");
        let browser_session_id = browser["result"]["browser_session_id"]
            .as_str()
            .expect("browser session id")
            .to_string();

        let worker = transport
            .send(build_request(Command::AgentWorkerCreate {
                token: token.clone(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-2".to_string(),
            }))
            .await
            .expect("worker create");
        let worker_id = worker["result"]["agent_worker_id"]
            .as_str()
            .expect("worker id")
            .to_string();

        let task = transport
            .send(build_request(Command::AgentTaskStart {
                token: token.clone(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-2".to_string(),
                agent_worker_id: worker_id.clone(),
                prompt: "echo cli-agent".to_string(),
            }))
            .await
            .expect("task start");
        let task_id = task["result"]["agent_task_id"]
            .as_str()
            .expect("task id")
            .to_string();

        let workers = transport
            .send(build_request(Command::AgentWorkerList {
                token: token.clone(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-2".to_string(),
            }))
            .await
            .expect("worker list");
        assert_eq!(
            workers["result"]["workers"][0]["agent_worker_id"],
            worker_id
        );

        let fetched_task = transport
            .send(build_request(Command::AgentTaskGet {
                token: token.clone(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-2".to_string(),
                agent_task_id: task_id.clone(),
                agent_worker_id: Some(worker_id.clone()),
            }))
            .await
            .expect("task get");
        assert_eq!(fetched_task["result"]["agent_task_id"], task_id);

        let cancelled = transport
            .send(build_request(Command::AgentTaskCancel {
                token: token.clone(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-2".to_string(),
                agent_task_id: task_id,
                reason: Some("cli test".to_string()),
            }))
            .await
            .expect("task cancel");
        assert_eq!(cancelled["result"]["cancelled"], true);

        let closed = transport
            .send(build_request(Command::AgentWorkerClose {
                token: created["result"]["token"]
                    .as_str()
                    .expect("token")
                    .to_string(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-2".to_string(),
                agent_worker_id: worker_id,
            }))
            .await
            .expect("worker close");
        assert_eq!(closed["result"]["closed"], true);

        let browser_closed = transport
            .send(build_request(Command::BrowserClose {
                token: created["result"]["token"]
                    .as_str()
                    .expect("token")
                    .to_string(),
                workspace_id: "ws-1".to_string(),
                surface_id: "sf-1".to_string(),
                browser_session_id,
            }))
            .await
            .expect("browser close");
        assert_eq!(browser_closed["result"]["closed"], true);

        // notification commands
        let notified = transport
            .send(build_request(Command::NotificationSend {
                token: token.clone(),
                title: "Build finished".to_string(),
                body: Some("All tests passed".to_string()),
                level: Some("success".to_string()),
                source: Some("cli".to_string()),
                workspace_id: Some(ws_id.clone()),
            }))
            .await
            .expect("notification send");
        assert!(notified["result"]["notification_id"].is_string());

        let listed = transport
            .send(build_request(Command::NotificationList {
                token: token.clone(),
                workspace_id: Some(ws_id.clone()),
                unread_only: Some(true),
                limit: Some(50),
            }))
            .await
            .expect("notification list");
        assert!(listed["result"]["notifications"].is_array());

        let cleared = transport
            .send(build_request(Command::NotificationClear {
                token: token.clone(),
                notification_id: None,
                workspace_id: Some(ws_id.clone()),
            }))
            .await
            .expect("notification clear");
        assert_eq!(cleared["result"]["cleared"], true);

        // workspace delete (at the end since it removes the workspace)
        let ws_deleted = transport
            .send(build_request(Command::WorkspaceDelete {
                token: token.clone(),
                workspace_id: ws_id.clone(),
            }))
            .await
            .expect("workspace delete");
        assert_eq!(ws_deleted["result"]["deleted"], true);

        // Verify workspace no longer appears in list
        let ws_list_after = transport
            .send(build_request(Command::WorkspaceList {
                token: token.clone(),
            }))
            .await
            .expect("workspace list after delete");
        let remaining = ws_list_after["result"]["workspaces"]
            .as_array()
            .expect("array");
        assert!(!remaining
            .iter()
            .any(|w| w["workspace_id"].as_str() == Some(&ws_id)));
    }
}
