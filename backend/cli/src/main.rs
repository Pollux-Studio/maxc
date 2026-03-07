use maxc_automation::{RpcId, RpcRequest};
use serde_json::{json, Value};
use std::collections::HashMap;

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
    BrowserTabOpen {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        url: String,
    },
    BrowserGoto {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        tab_id: String,
        url: String,
    },
    BrowserHistory {
        token: String,
        workspace_id: String,
        surface_id: String,
        browser_session_id: String,
        from_sequence: Option<u64>,
        max_events: Option<usize>,
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = run_cli(
        std::env::args().skip(1).collect(),
        &NamedPipeTransport::new(r"\\.\pipe\maxc-rpc"),
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
    pipe_name: String,
}

impl NamedPipeTransport {
    fn new(pipe_name: &str) -> Self {
        Self {
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
    let response = transport.send(build_request(command)).await?;
    render_response(&response, pretty)
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
                token: required(&flags, "--token")?,
            }
        }
        "diagnostics" => {
            let flags = parse_flags(&args[1..])?;
            Command::Diagnostics {
                token: required(&flags, "--token")?,
            }
        }
        "session" => parse_session(&args[1..])?,
        "terminal" => parse_terminal(&args[1..])?,
        "browser" => parse_browser(&args[1..])?,
        "agent" => parse_agent(&args[1..])?,
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
            token: required(&flags, "--token")?,
        }),
        "revoke" => Ok(Command::SessionRevoke {
            token: required(&flags, "--token")?,
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
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            cols: optional_parse(&flags, "--cols")?.unwrap_or(120),
            rows: optional_parse(&flags, "--rows")?.unwrap_or(30),
        }),
        "input" => Ok(Command::TerminalInput {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
            input: required(&flags, "--input")?,
        }),
        "resize" => Ok(Command::TerminalResize {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
            cols: required(&flags, "--cols")?.parse()?,
            rows: required(&flags, "--rows")?.parse()?,
        }),
        "history" => Ok(Command::TerminalHistory {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
            from_sequence: optional_parse(&flags, "--from-sequence")?,
            max_events: optional_parse(&flags, "--max-events")?,
        }),
        "kill" => Ok(Command::TerminalKill {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
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
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
        }),
        "tab-open" => Ok(Command::BrowserTabOpen {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            url: required(&flags, "--url")?,
        }),
        "goto" => Ok(Command::BrowserGoto {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            tab_id: required(&flags, "--tab-id")?,
            url: required(&flags, "--url")?,
        }),
        "history" => Ok(Command::BrowserHistory {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            browser_session_id: required(&flags, "--browser-session-id")?,
            from_sequence: optional_parse(&flags, "--from-sequence")?,
            max_events: optional_parse(&flags, "--max-events")?,
        }),
        "close" => Ok(Command::BrowserClose {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            browser_session_id: required(&flags, "--browser-session-id")?,
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
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
        }),
        ("worker", "list") => Ok(Command::AgentWorkerList {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
        }),
        ("worker", "get") => Ok(Command::AgentWorkerGet {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        ("worker", "close") => Ok(Command::AgentWorkerClose {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        ("task", "start") => Ok(Command::AgentTaskStart {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
            prompt: required(&flags, "--prompt")?,
        }),
        ("task", "list") => Ok(Command::AgentTaskList {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
        }),
        ("task", "get") => Ok(Command::AgentTaskGet {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_task_id: required(&flags, "--agent-task-id")?,
            agent_worker_id: flags.get("--agent-worker-id").cloned(),
        }),
        ("task", "cancel") => Ok(Command::AgentTaskCancel {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_task_id: required(&flags, "--agent-task-id")?,
            reason: flags.get("--reason").cloned(),
        }),
        ("attach", "terminal") => Ok(Command::AgentAttachTerminal {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
            terminal_session_id: required(&flags, "--terminal-session-id")?,
        }),
        ("detach", "terminal") => Ok(Command::AgentDetachTerminal {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        ("attach", "browser") => Ok(Command::AgentAttachBrowser {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
            browser_session_id: required(&flags, "--browser-session-id")?,
        }),
        ("detach", "browser") => Ok(Command::AgentDetachBrowser {
            token: required(&flags, "--token")?,
            workspace_id: required(&flags, "--workspace-id")?,
            surface_id: required(&flags, "--surface-id")?,
            agent_worker_id: required(&flags, "--agent-worker-id")?,
        }),
        _ => Err("unknown agent subcommand".into()),
    }
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
    format!("cli-{prefix}")
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
        assert_eq!(command_id("demo"), "cli-demo");
        assert_eq!(request("system.health", None).method, "system.health");
        assert!(NamedPipeTransport::new("pipe-demo")
            .pipe_name
            .contains("pipe-demo"));
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
                token,
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
    }
}
