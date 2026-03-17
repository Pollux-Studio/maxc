use maxc_automation::RpcServer;
use maxc_core::BackendConfig;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::process::{Command as StdCommand, Stdio as StdStdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
struct ProfileConfig {
    iterations: usize,
    warmup: usize,
    fixture_events: usize,
}

#[derive(Debug, Clone, Default)]
struct ThresholdConfig {
    p95_ms: Option<f64>,
    max_ms: Option<f64>,
}

#[derive(Debug, Clone)]
struct BenchmarkResult {
    profile: String,
    mode: String,
    iterations: usize,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    throughput_ops_per_sec: f64,
    pass: bool,
}

#[derive(Debug, Clone)]
struct BrowserLaunchTarget {
    executable: String,
    config_value: String,
    runtime: String,
}

fn is_webview2_executable_path(value: &str) -> bool {
    let normalized = value.replace('/', "\\");
    normalized
        .rsplit('\\')
        .next()
        .map(|name| name.eq_ignore_ascii_case("msedgewebview2.exe"))
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = run_cli(std::env::args().skip(1).collect()).await?;
    if let Some(text) = output.output {
        print!("{text}");
    }
    if output.pass {
        Ok(())
    } else {
        Err("performance thresholds failed".into())
    }
}

struct CliRun {
    output: Option<String>,
    pass: bool,
}

async fn run_cli(args: Vec<String>) -> Result<CliRun, Box<dyn std::error::Error>> {
    let mut profile = String::from("ci");
    let mut mode = String::from("synthetic");
    let mut json_output = false;
    let mut probe_real_browser_runtime = false;
    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--profile" if idx + 1 < args.len() => {
                profile = args[idx + 1].clone();
                idx += 1;
            }
            "--mode" if idx + 1 < args.len() => {
                mode = args[idx + 1].clone();
                idx += 1;
            }
            "--json" => {
                json_output = true;
            }
            "--probe-real-browser-runtime" => {
                probe_real_browser_runtime = true;
            }
            _ => {}
        }
        idx += 1;
    }
    if probe_real_browser_runtime {
        return probe_real_browser_runtime_cli(json_output).await;
    }
    execute_suite(&profile, &mode, json_output).await
}

async fn probe_real_browser_runtime_cli(
    json_output: bool,
) -> Result<CliRun, Box<dyn std::error::Error>> {
    let runtime = probe_real_browser_runtime(&BackendConfig::default()).await?;
    let available = runtime.is_some();
    let output = if json_output {
        Some(format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "available": available,
                "config_value": runtime.as_ref().map(|value| value.config_value.clone()),
                "executable": runtime.as_ref().map(|value| value.executable.clone()),
                "runtime": runtime.as_ref().map(|value| value.runtime.clone())
            }))?
        ))
    } else {
        Some(match runtime {
            Some(runtime) => format!(
                "available=true runtime={} config_value={} executable={}\n",
                runtime.runtime, runtime.config_value, runtime.executable
            ),
            None => "available=false\n".to_string(),
        })
    };
    Ok(CliRun { output, pass: true })
}

async fn execute_suite(
    profile: &str,
    mode: &str,
    json_output: bool,
) -> Result<CliRun, Box<dyn std::error::Error>> {
    let mode = match mode {
        "synthetic" | "real-runtime" => mode,
        other => return Err(format!("unknown perf mode: {other}").into()),
    };
    let profiles = load_profile_configs(mode)?;
    let thresholds = load_thresholds(mode)?;
    let names = if profile == "ci" {
        default_suite_profiles(mode)
    } else {
        vec![profile]
    };

    let mut results = Vec::new();
    for name in names {
        let profile_cfg = profiles
            .get(name)
            .cloned()
            .unwrap_or_else(|| default_profile(name));
        let threshold = thresholds.get(name).cloned().unwrap_or_default();
        let result = run_profile(name, mode, &profile_cfg, &threshold).await?;
        results.push(result);
    }

    let ok = results.iter().all(|result| result.pass);
    let output = if json_output {
        let payload = json!({
            "suite": profile,
            "mode": mode,
            "pass": ok,
            "results": results.iter().map(|result| {
                json!({
                    "profile": result.profile,
                    "mode": result.mode,
                    "iterations": result.iterations,
                    "p50_ms": result.p50_ms,
                    "p95_ms": result.p95_ms,
                    "max_ms": result.max_ms,
                    "throughput_ops_per_sec": result.throughput_ops_per_sec,
                    "pass": result.pass
                })
            }).collect::<Vec<_>>()
        });
        Some(format!("{}\n", serde_json::to_string_pretty(&payload)?))
    } else {
        let mut text = String::new();
        for result in &results {
            text.push_str(&format!(
                "{} [{}]: p50={:.2}ms p95={:.2}ms max={:.2}ms throughput={:.2}/s pass={}\n",
                result.profile,
                result.mode,
                result.p50_ms,
                result.p95_ms,
                result.max_ms,
                result.throughput_ops_per_sec,
                result.pass
            ));
        }
        Some(text)
    };

    Ok(CliRun { output, pass: ok })
}

async fn run_profile(
    name: &str,
    mode: &str,
    profile_cfg: &ProfileConfig,
    threshold: &ThresholdConfig,
) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
    let mut samples = Vec::new();
    let started = Instant::now();
    let event_dir = temp_event_dir(name);
    let mut config = BackendConfig {
        event_dir: event_dir.to_string_lossy().to_string(),
        ..BackendConfig::default()
    };
    if mode == "synthetic" && name == "terminal_interactive" {
        config.terminal_runtime = "process-stdio".to_string();
    }
    if mode == "synthetic"
        && matches!(
            name,
            "browser_navigation" | "browser_fanout" | "restart_recovery"
        )
    {
        config.browser_executable_or_channel = "__synthetic__".to_string();
    }
    if mode == "real-runtime" {
        if !cfg!(windows) {
            return Err("real-runtime perf mode requires Windows".into());
        }
        if matches!(
            name,
            "browser_create_latency" | "browser_navigation_latency" | "browser_screenshot_latency"
        ) {
            pin_real_browser_target(&mut config)?;
            if probe_real_browser_runtime(&config).await?.is_none() {
                return Err(
                    "real-runtime browser benchmarks require a backend-confirmed real Chromium, Edge, or WebView2 runtime"
                        .into(),
                );
            }
        }
    }
    let server = RpcServer::new(config)?;

    match name {
        "rpc_health" => {
            for _ in 0..profile_cfg.warmup {
                let _ = issue(&server, 0, json!({"id": 1, "method": "system.health"})).await?;
            }
            for iter in 0..profile_cfg.iterations {
                let req = json!({"id": iter as i64, "method": "system.health"});
                samples.push(measure_async(issue(&server, iter, req)).await?);
            }
        }
        "session_lifecycle" => {
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64,
                    "method": "session.create",
                    "params": {"command_id": format!("cmd-session-{iter}")}
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "terminal_interactive" => {
            let token = create_token(&server).await?;
            let spawned = issue(
                &server,
                1,
                json!({
                    "id": 1,
                    "method": "terminal.spawn",
                    "params": {
                        "command_id":"cmd-term-spawn",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "auth":{"token": token},
                        "shell": benchmark_terminal_shell()
                    }
                }),
            )
            .await?;
            let terminal_session_id = spawned["result"]["terminal_session_id"]
                .as_str()
                .ok_or("missing terminal session id")?
                .to_string();
            tokio::time::sleep(Duration::from_millis(75)).await;
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 10,
                    "method": "terminal.input",
                    "params": {
                        "command_id": format!("cmd-term-input-{iter}"),
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "terminal_session_id": terminal_session_id,
                        "auth":{"token": token},
                        "input": benchmark_terminal_echo(iter)
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "browser_navigation" => {
            let token = create_token(&server).await?;
            let browser = issue(
                &server,
                1,
                json!({
                    "id": 1,
                    "method": "browser.create",
                    "params": {
                        "command_id":"cmd-browser-create",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "auth":{"token": token}
                    }
                }),
            )
            .await?;
            let browser_session_id = browser["result"]["browser_session_id"]
                .as_str()
                .ok_or("missing browser session id")?
                .to_string();
            let tab = issue(
                &server,
                2,
                json!({
                    "id": 2,
                    "method": "browser.tab.open",
                    "params": {
                        "command_id":"cmd-browser-tab",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id": browser_session_id,
                        "auth":{"token": token},
                        "url": "https://example.com"
                    }
                }),
            )
            .await?;
            let browser_tab_id = tab["result"]["browser_tab_id"]
                .as_str()
                .ok_or("missing browser tab id")?
                .to_string();
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 10,
                    "method": "browser.goto",
                    "params": {
                        "command_id": format!("cmd-browser-goto-{iter}"),
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id": browser_session_id,
                        "tab_id": browser_tab_id,
                        "auth":{"token": token},
                        "url": format!("https://example.com/{iter}")
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "browser_fanout" => {
            let token = create_token(&server).await?;
            let browser = issue(
                &server,
                1,
                json!({
                    "id": 1,
                    "method": "browser.create",
                    "params": {
                        "command_id":"cmd-fanout-create",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "auth":{"token": token}
                    }
                }),
            )
            .await?;
            let browser_session_id = browser["result"]["browser_session_id"]
                .as_str()
                .ok_or("missing browser session id")?
                .to_string();
            let tab = issue(
                &server,
                2,
                json!({
                    "id": 2,
                    "method": "browser.tab.open",
                    "params": {
                        "command_id":"cmd-fanout-tab",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id": browser_session_id,
                        "auth":{"token": token},
                        "url": "https://example.com"
                    }
                }),
            )
            .await?;
            let browser_tab_id = tab["result"]["browser_tab_id"]
                .as_str()
                .ok_or("missing browser tab id")?
                .to_string();
            for idx in 0..4 {
                let _ = issue(
                    &server,
                    idx,
                    json!({
                        "id": idx as i64 + 3,
                        "method":"browser.subscribe",
                        "params":{
                            "command_id": format!("cmd-fanout-sub-{idx}"),
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id": browser_session_id,
                            "auth":{"token": token}
                        }
                    }),
                )
                .await?;
            }
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 20,
                    "method": "browser.click",
                    "params": {
                        "command_id": format!("cmd-fanout-click-{iter}"),
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id": browser_session_id,
                        "tab_id": browser_tab_id,
                        "auth":{"token": token}
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "restart_recovery" => {
            let token = create_token(&server).await?;
            for iter in 0..profile_cfg.fixture_events.max(1) {
                let _ = issue(
                    &server,
                    iter,
                    json!({
                        "id": iter as i64,
                        "method": "browser.create",
                        "params": {
                            "command_id": format!("cmd-recovery-{iter}"),
                            "workspace_id":"ws-1",
                            "surface_id": format!("sf-{iter}"),
                            "auth":{"token": token}
                        }
                    }),
                )
                .await?;
            }
            let restart_start = Instant::now();
            let restarted = RpcServer::new(BackendConfig {
                event_dir: event_dir.to_string_lossy().to_string(),
                ..BackendConfig::default()
            })?;
            let _ = restarted.session_count().await;
            samples.push(restart_start.elapsed().as_secs_f64() * 1000.0);
        }
        "terminal_spawn_latency" => {
            let token = create_token(&server).await?;
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 100,
                    "method": "terminal.spawn",
                    "params": {
                        "command_id": format!("cmd-real-term-spawn-{iter}"),
                        "workspace_id": format!("ws-real-{iter}"),
                        "surface_id": format!("sf-real-{iter}"),
                        "auth":{"token": token},
                        "shell": benchmark_terminal_shell()
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "browser_create_latency" => {
            let token = create_token(&server).await?;
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 200,
                    "method": "browser.create",
                    "params": {
                        "command_id": format!("cmd-real-browser-create-{iter}"),
                        "workspace_id":"ws-real",
                        "surface_id": format!("sf-browser-{iter}"),
                        "auth":{"token": token}
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "browser_navigation_latency" => {
            let token = create_token(&server).await?;
            let browser = issue(
                &server,
                1,
                json!({
                    "id": 1,
                    "method": "browser.create",
                    "params": {
                        "command_id":"cmd-real-browser-create",
                        "workspace_id":"ws-real",
                        "surface_id":"sf-real",
                        "auth":{"token": token}
                    }
                }),
            )
            .await?;
            let browser_session_id = browser["result"]["browser_session_id"]
                .as_str()
                .ok_or("missing browser session id")?
                .to_string();
            let tab = issue(
                &server,
                2,
                json!({
                    "id": 2,
                    "method": "browser.tab.open",
                    "params": {
                        "command_id":"cmd-real-browser-tab",
                        "workspace_id":"ws-real",
                        "surface_id":"sf-real",
                        "browser_session_id": browser_session_id,
                        "auth":{"token": token},
                        "url":"https://example.com"
                    }
                }),
            )
            .await?;
            let browser_tab_id = tab["result"]["browser_tab_id"]
                .as_str()
                .ok_or("missing browser tab id")?
                .to_string();
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 210,
                    "method": "browser.goto",
                    "params": {
                        "command_id": format!("cmd-real-browser-goto-{iter}"),
                        "workspace_id":"ws-real",
                        "surface_id":"sf-real",
                        "browser_session_id": browser_session_id,
                        "tab_id": browser_tab_id,
                        "auth":{"token": token},
                        "url": format!("https://example.com/real/{iter}")
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "browser_screenshot_latency" => {
            let token = create_token(&server).await?;
            let browser = issue(
                &server,
                1,
                json!({
                    "id": 1,
                    "method": "browser.create",
                    "params": {
                        "command_id":"cmd-real-shot-create",
                        "workspace_id":"ws-real",
                        "surface_id":"sf-real",
                        "auth":{"token": token}
                    }
                }),
            )
            .await?;
            let browser_session_id = browser["result"]["browser_session_id"]
                .as_str()
                .ok_or("missing browser session id")?
                .to_string();
            let tab = issue(
                &server,
                2,
                json!({
                    "id": 2,
                    "method": "browser.tab.open",
                    "params": {
                        "command_id":"cmd-real-shot-tab",
                        "workspace_id":"ws-real",
                        "surface_id":"sf-real",
                        "browser_session_id": browser_session_id,
                        "auth":{"token": token},
                        "url":"https://example.com"
                    }
                }),
            )
            .await?;
            let browser_tab_id = tab["result"]["browser_tab_id"]
                .as_str()
                .ok_or("missing browser tab id")?
                .to_string();
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 220,
                    "method": "browser.screenshot",
                    "params": {
                        "command_id": format!("cmd-real-shot-{iter}"),
                        "workspace_id":"ws-real",
                        "surface_id":"sf-real",
                        "browser_session_id": browser_session_id,
                        "tab_id": browser_tab_id,
                        "auth":{"token": token}
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        "agent_worker_start_latency" => {
            let token = create_token(&server).await?;
            for iter in 0..(profile_cfg.warmup + profile_cfg.iterations) {
                let req = json!({
                    "id": iter as i64 + 300,
                    "method": "agent.worker.create",
                    "params": {
                        "command_id": format!("cmd-real-agent-worker-{iter}"),
                        "workspace_id":"ws-real",
                        "surface_id": format!("sf-agent-{iter}"),
                        "auth":{"token": token},
                        "shell": benchmark_terminal_shell()
                    }
                });
                let sample = measure_async(issue(&server, iter, req)).await?;
                if iter >= profile_cfg.warmup {
                    samples.push(sample);
                }
            }
        }
        other => {
            return Err(format!("unknown profile: {other}").into());
        }
    }

    let total_s = started.elapsed().as_secs_f64().max(0.000_001);
    let result = summarize(
        name,
        mode,
        profile_cfg.iterations.max(samples.len()),
        &samples,
        total_s,
        threshold,
    );
    let _ = fs::remove_dir_all(event_dir);
    Ok(result)
}

fn summarize(
    profile: &str,
    mode: &str,
    iterations: usize,
    samples: &[f64],
    total_s: f64,
    threshold: &ThresholdConfig,
) -> BenchmarkResult {
    let mut ordered = samples.to_vec();
    ordered.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = percentile(&ordered, 0.50);
    let p95 = percentile(&ordered, 0.95);
    let max_ms = ordered.last().copied().unwrap_or(0.0);
    let throughput = samples.len() as f64 / total_s;
    let pass_p95 = threshold.p95_ms.map(|limit| p95 <= limit).unwrap_or(true);
    let pass_max = threshold
        .max_ms
        .map(|limit| max_ms <= limit)
        .unwrap_or(true);
    BenchmarkResult {
        profile: profile.to_string(),
        mode: mode.to_string(),
        iterations,
        p50_ms: p50,
        p95_ms: p95,
        max_ms,
        throughput_ops_per_sec: throughput,
        pass: pass_p95 && pass_max,
    }
}

fn percentile(samples: &[f64], pct: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let idx = ((samples.len() - 1) as f64 * pct).round() as usize;
    samples[idx.min(samples.len() - 1)]
}

async fn measure_async<F>(op: F) -> Result<f64, Box<dyn std::error::Error>>
where
    F: Future<Output = Result<Value, Box<dyn std::error::Error>>>,
{
    let start = Instant::now();
    let _ = op.await?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

async fn issue(
    server: &RpcServer,
    connection_id: usize,
    request: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let raw = server
        .handle_json_line(&format!("perf-{connection_id}"), &request.to_string())
        .await;
    let parsed: Value = serde_json::from_str(&raw)?;
    if parsed.get("error").is_some() {
        return Err(format!("request failed: {parsed}").into());
    }
    Ok(parsed)
}

async fn create_token(server: &RpcServer) -> Result<String, Box<dyn std::error::Error>> {
    let response = issue(
        server,
        0,
        json!({
            "id": 1,
            "method": "session.create",
            "params": { "command_id": "cmd-auth" }
        }),
    )
    .await?;
    response["result"]["token"]
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| "missing token".into())
}

fn load_profile_configs(
    mode: &str,
) -> Result<BTreeMap<String, ProfileConfig>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(data_path(profile_file_name(mode)))?;
    let raw: BTreeMap<String, Value> = serde_json::from_str(&content)?;
    let mut out = BTreeMap::new();
    for (name, value) in raw {
        out.insert(
            name,
            ProfileConfig {
                iterations: value
                    .get("iterations")
                    .and_then(Value::as_u64)
                    .unwrap_or(50) as usize,
                warmup: value.get("warmup").and_then(Value::as_u64).unwrap_or(5) as usize,
                fixture_events: value
                    .get("fixture_events")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize,
            },
        );
    }
    Ok(out)
}

fn load_thresholds(
    mode: &str,
) -> Result<BTreeMap<String, ThresholdConfig>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(data_path(threshold_file_name(mode)))?;
    let raw: BTreeMap<String, Value> = serde_json::from_str(&content)?;
    let mut out = BTreeMap::new();
    for (name, value) in raw {
        out.insert(
            name,
            ThresholdConfig {
                p95_ms: value.get("p95_ms").and_then(Value::as_f64),
                max_ms: value.get("max_ms").and_then(Value::as_f64),
            },
        );
    }
    Ok(out)
}

fn default_profile(name: &str) -> ProfileConfig {
    match name {
        "restart_recovery" => ProfileConfig {
            iterations: 1,
            warmup: 0,
            fixture_events: 1000,
        },
        _ => ProfileConfig {
            iterations: 50,
            warmup: 5,
            fixture_events: 0,
        },
    }
}

fn default_suite_profiles(mode: &str) -> Vec<&'static str> {
    match mode {
        "synthetic" => vec![
            "rpc_health",
            "session_lifecycle",
            "terminal_interactive",
            "browser_navigation",
            "browser_fanout",
            "restart_recovery",
        ],
        "real-runtime" => vec![
            "terminal_spawn_latency",
            "terminal_interactive",
            "browser_create_latency",
            "browser_navigation_latency",
            "browser_screenshot_latency",
            "agent_worker_start_latency",
        ],
        _ => Vec::new(),
    }
}

fn profile_file_name(mode: &str) -> &'static str {
    match mode {
        "synthetic" => "perf-profiles.json",
        "real-runtime" => "perf-profiles-real.json",
        _ => "perf-profiles.json",
    }
}

fn threshold_file_name(mode: &str) -> &'static str {
    match mode {
        "synthetic" => "perf-baseline.json",
        "real-runtime" => "perf-baseline-real.json",
        _ => "perf-baseline.json",
    }
}

fn resolve_browser_executable(config: &BackendConfig) -> Option<String> {
    let configured = config.browser_executable_or_channel.trim();
    if configured.eq_ignore_ascii_case("__synthetic__") {
        return None;
    }
    if configured.eq_ignore_ascii_case("webview2") {
        return resolve_webview2_executable();
    }
    if !configured.is_empty()
        && PathBuf::from(configured).exists()
        && is_webview2_executable_path(configured)
    {
        return None;
    }
    if !configured.is_empty() && PathBuf::from(configured).exists() {
        return Some(configured.to_string());
    }
    let candidates: &[&str] = match configured {
        "chrome" | "chromium" | "" => &[
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
            "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
        ],
        "edge" | "msedge" => &[
            "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
            "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
        ],
        other => &[other],
    };
    candidates
        .iter()
        .find(|candidate| PathBuf::from(candidate).exists())
        .map(|candidate| (*candidate).to_string())
}

fn resolve_webview2_executable() -> Option<String> {
    #[cfg(not(windows))]
    {
        None
    }
    #[cfg(windows)]
    {
        let roots = [
            PathBuf::from("C:\\Program Files (x86)\\Microsoft\\EdgeWebView\\Application"),
            PathBuf::from("C:\\Program Files\\Microsoft\\EdgeWebView\\Application"),
        ];
        for root in roots {
            if !root.exists() {
                continue;
            }
            let mut versions = fs::read_dir(&root)
                .ok()?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            versions.sort();
            versions.reverse();
            for version_dir in versions {
                let candidate = version_dir.join("msedgewebview2.exe");
                if candidate.exists() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
        None
    }
}

fn browser_launch_targets(config: &BackendConfig) -> Vec<BrowserLaunchTarget> {
    if config
        .browser_executable_or_channel
        .trim()
        .eq_ignore_ascii_case("__synthetic__")
    {
        return Vec::new();
    }
    if config
        .browser_executable_or_channel
        .trim()
        .eq_ignore_ascii_case("webview2")
    {
        return resolve_webview2_executable()
            .map(|executable| {
                vec![BrowserLaunchTarget {
                    executable,
                    config_value: "webview2".to_string(),
                    runtime: "webview2".to_string(),
                }]
            })
            .unwrap_or_default();
    }
    let configured = config.browser_executable_or_channel.trim();
    if !configured.is_empty()
        && PathBuf::from(configured).exists()
        && is_webview2_executable_path(configured)
    {
        return vec![BrowserLaunchTarget {
            executable: configured.to_string(),
            config_value: "webview2".to_string(),
            runtime: "webview2".to_string(),
        }];
    }

    let mut targets = Vec::new();
    if let Some(executable) = resolve_browser_executable(config) {
        targets.push(BrowserLaunchTarget {
            config_value: executable.clone(),
            executable,
            runtime: "chromium-cdp".to_string(),
        });
    }
    if let Some(executable) = resolve_webview2_executable() {
        if !targets
            .iter()
            .any(|target| target.executable.eq_ignore_ascii_case(executable.as_str()))
        {
            targets.push(BrowserLaunchTarget {
                executable,
                config_value: "webview2".to_string(),
                runtime: "webview2".to_string(),
            });
        }
    }
    targets
}

fn pin_real_browser_target(config: &mut BackendConfig) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(target) = probe_real_browser_target(config) {
        config.browser_executable_or_channel = target.config_value;
        return Ok(());
    }
    Err("real-runtime browser benchmarks require a launchable local Chromium, Edge, or WebView2 runtime".into())
}

fn probe_real_browser_target(config: &BackendConfig) -> Option<BrowserLaunchTarget> {
    browser_launch_targets(config)
        .into_iter()
        .find(browser_target_launchable)
}

async fn probe_real_browser_runtime(
    config: &BackendConfig,
) -> Result<Option<BrowserLaunchTarget>, Box<dyn std::error::Error>> {
    let Some(target) = probe_real_browser_target(config) else {
        return Ok(None);
    };

    let event_dir = temp_event_dir("real-browser-probe");
    let mut probe_config = config.clone();
    probe_config.event_dir = event_dir.to_string_lossy().to_string();
    probe_config.browser_executable_or_channel = target.config_value.clone();

    let result = async {
        let server = RpcServer::new(probe_config)?;
        let token = create_token(&server).await?;
        let response = issue(
            &server,
            0,
            json!({
                "id": 1,
                "method": "browser.create",
                "params": {
                    "command_id":"cmd-probe-browser-create",
                    "workspace_id":"ws-probe",
                    "surface_id":"sf-probe",
                    "auth":{"token": token}
                }
            }),
        )
        .await?;
        let runtime = response["result"]["runtime"]
            .as_str()
            .unwrap_or("browser-simulated");
        if runtime == "browser-simulated" {
            Ok(None)
        } else {
            Ok(Some(BrowserLaunchTarget {
                executable: target.executable.clone(),
                config_value: target.config_value.clone(),
                runtime: runtime.to_string(),
            }))
        }
    }
    .await;

    let _ = fs::remove_dir_all(event_dir);
    result
}

fn browser_target_launchable(target: &BrowserLaunchTarget) -> bool {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis();
    let probe_dir = std::env::temp_dir().join(format!("maxc-perf-browser-probe-{millis}"));
    if fs::create_dir_all(&probe_dir).is_err() {
        return false;
    }

    let mut command = StdCommand::new(&target.executable);
    command
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", probe_dir.display()))
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--disable-background-networking")
        .arg("--disable-sync")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-popup-blocking")
        .arg("about:blank")
        .stdin(StdStdio::null())
        .stdout(StdStdio::null())
        .stderr(StdStdio::null());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            let _ = fs::remove_dir_all(&probe_dir);
            return false;
        }
    };

    let devtools_file = probe_dir.join("DevToolsActivePort");
    let started = Instant::now();
    let launched = loop {
        if let Ok(content) = fs::read_to_string(&devtools_file) {
            if content
                .lines()
                .next()
                .and_then(|line| line.trim().parse::<u16>().ok())
                .is_some()
            {
                break true;
            }
        }
        if started.elapsed() > Duration::from_secs(5) {
            break false;
        }
        thread::sleep(Duration::from_millis(50));
    };

    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(&probe_dir);
    launched
}

fn temp_event_dir(label: &str) -> PathBuf {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("maxc-perf-{label}-{millis}"))
}

fn data_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(file_name)
}

fn benchmark_terminal_shell() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "sh"
    }
}

fn benchmark_terminal_echo(iter: usize) -> String {
    if cfg!(windows) {
        format!("Write-Output {iter}")
    } else {
        format!("printf '{iter}\\n'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaders_and_defaults_work() {
        let profiles = load_profile_configs("synthetic").expect("profiles");
        assert!(profiles.contains_key("rpc_health"));
        let thresholds = load_thresholds("synthetic").expect("thresholds");
        assert_eq!(
            thresholds
                .get("browser_navigation")
                .and_then(|value| value.p95_ms),
            Some(30.0)
        );
        assert_eq!(default_profile("restart_recovery").fixture_events, 1000);
        assert!(temp_event_dir("x")
            .to_string_lossy()
            .contains("maxc-perf-x-"));
        assert_eq!(profile_file_name("real-runtime"), "perf-profiles-real.json");
        assert_eq!(
            threshold_file_name("real-runtime"),
            "perf-baseline-real.json"
        );
    }

    #[test]
    fn summarize_and_percentiles_are_stable() {
        let result = summarize(
            "demo",
            "synthetic",
            4,
            &[1.0, 2.0, 3.0, 4.0],
            2.0,
            &ThresholdConfig {
                p95_ms: Some(5.0),
                max_ms: Some(5.0),
            },
        );
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.5), 3.0);
        assert!(result.pass);
        assert_eq!(result.iterations, 4);
    }

    #[test]
    fn summarize_failures_and_helpers_cover_edge_cases() {
        let failing = summarize(
            "slow",
            "synthetic",
            2,
            &[10.0, 20.0],
            4.0,
            &ThresholdConfig {
                p95_ms: Some(5.0),
                max_ms: Some(15.0),
            },
        );
        assert!(!failing.pass);
        assert_eq!(percentile(&[], 0.5), 0.0);
        assert_eq!(
            data_path("perf-profiles.json")
                .file_name()
                .and_then(|s| s.to_str()),
            Some("perf-profiles.json")
        );
        assert_eq!(
            benchmark_terminal_shell(),
            if cfg!(windows) { "powershell" } else { "sh" }
        );
        assert!(!benchmark_terminal_echo(7).is_empty());
    }

    #[tokio::test]
    async fn cli_helpers_render_text_and_json_output() {
        let compact = run_cli(vec!["--profile".to_string(), "rpc_health".to_string()])
            .await
            .expect("compact");
        assert!(compact.pass);
        assert!(compact
            .output
            .as_deref()
            .expect("output")
            .contains("rpc_health [synthetic]:"));

        let json_result = execute_suite("rpc_health", "synthetic", true)
            .await
            .expect("json");
        assert!(json_result.pass);
        let output = json_result.output.expect("json output");
        assert!(output.contains("\"suite\": \"rpc_health\""));
        assert!(output.contains("\"mode\": \"synthetic\""));
        assert!(output.contains("\"results\""));

        let probe = run_cli(vec![
            "--probe-real-browser-runtime".to_string(),
            "--json".to_string(),
        ])
        .await
        .expect("probe");
        assert!(probe.pass);
        let probe_output = probe.output.expect("probe output");
        assert!(probe_output.contains("\"available\""));
    }

    #[tokio::test]
    async fn run_rpc_health_profile() {
        let result = run_profile(
            "rpc_health",
            "synthetic",
            &ProfileConfig {
                iterations: 3,
                warmup: 1,
                fixture_events: 0,
            },
            &ThresholdConfig {
                p95_ms: Some(50.0),
                max_ms: None,
            },
        )
        .await
        .expect("profile");
        assert!(result.pass);
        assert_eq!(result.profile, "rpc_health");
    }

    #[tokio::test]
    async fn run_terminal_and_recovery_profiles() {
        let terminal = run_profile(
            "terminal_interactive",
            "synthetic",
            &ProfileConfig {
                iterations: 2,
                warmup: 0,
                fixture_events: 0,
            },
            &ThresholdConfig::default(),
        )
        .await
        .expect("terminal profile");
        assert_eq!(terminal.profile, "terminal_interactive");

        let recovery = run_profile(
            "restart_recovery",
            "synthetic",
            &ProfileConfig {
                iterations: 1,
                warmup: 0,
                fixture_events: 10,
            },
            &ThresholdConfig {
                p95_ms: None,
                max_ms: Some(300.0),
            },
        )
        .await
        .expect("recovery profile");
        assert!(recovery.pass);
    }

    #[tokio::test]
    async fn run_session_and_browser_profiles() {
        let session = run_profile(
            "session_lifecycle",
            "synthetic",
            &ProfileConfig {
                iterations: 2,
                warmup: 0,
                fixture_events: 0,
            },
            &ThresholdConfig::default(),
        )
        .await
        .expect("session profile");
        assert_eq!(session.profile, "session_lifecycle");

        let browser_nav = run_profile(
            "browser_navigation",
            "synthetic",
            &ProfileConfig {
                iterations: 2,
                warmup: 0,
                fixture_events: 0,
            },
            &ThresholdConfig::default(),
        )
        .await
        .expect("browser nav profile");
        assert_eq!(browser_nav.profile, "browser_navigation");

        let browser_fanout = run_profile(
            "browser_fanout",
            "synthetic",
            &ProfileConfig {
                iterations: 2,
                warmup: 0,
                fixture_events: 0,
            },
            &ThresholdConfig::default(),
        )
        .await
        .expect("browser fanout profile");
        assert_eq!(browser_fanout.profile, "browser_fanout");
    }

    #[tokio::test]
    async fn issue_and_unknown_profile_errors_are_reported() {
        let event_dir = temp_event_dir("issue-errors");
        let server = RpcServer::new(BackendConfig {
            event_dir: event_dir.to_string_lossy().to_string(),
            ..BackendConfig::default()
        })
        .expect("server");

        let token = create_token(&server).await.expect("token");
        assert!(!token.is_empty());

        let failure = issue(
            &server,
            1,
            json!({
                "id": 9,
                "method": "session.refresh",
                "params": {
                    "command_id":"cmd-bad-refresh",
                    "auth":{"token":"invalid"}
                }
            }),
        )
        .await;
        assert!(failure.is_err());

        let unknown = run_profile(
            "not-a-profile",
            "synthetic",
            &ProfileConfig {
                iterations: 1,
                warmup: 0,
                fixture_events: 0,
            },
            &ThresholdConfig::default(),
        )
        .await;
        assert!(unknown.is_err());

        let failing = execute_suite("does-not-exist", "synthetic", false).await;
        assert!(failing.is_err());

        let _ = fs::remove_dir_all(event_dir);
    }

    #[test]
    fn real_runtime_suite_names_and_browser_detection_work() {
        let names = default_suite_profiles("real-runtime");
        assert!(names.contains(&"terminal_spawn_latency"));
        assert!(names.contains(&"agent_worker_start_latency"));
        let synthetic = BackendConfig {
            browser_executable_or_channel: "__synthetic__".to_string(),
            ..BackendConfig::default()
        };
        assert!(browser_launch_targets(&synthetic).is_empty());

        let temp_dir =
            std::env::temp_dir().join(format!("maxc-webview2-test-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("temp dir");
        let explicit_webview2 = temp_dir.join("msedgewebview2.exe");
        fs::write(&explicit_webview2, b"stub").expect("stub executable");
        let cfg = BackendConfig {
            browser_executable_or_channel: explicit_webview2.to_string_lossy().to_string(),
            ..BackendConfig::default()
        };
        let targets = browser_launch_targets(&cfg);
        assert_eq!(
            targets.first().map(|target| target.config_value.as_str()),
            Some("webview2")
        );
        let _ = fs::remove_dir_all(temp_dir);
    }
}
