use maxc_automation::RpcServer;
use maxc_core::BackendConfig;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::time::Instant;

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
    iterations: usize,
    p50_ms: f64,
    p95_ms: f64,
    max_ms: f64,
    throughput_ops_per_sec: f64,
    pass: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut profile = String::from("ci");
    let mut json_output = false;
    let mut idx = 1;
    while idx < args.len() {
        match args[idx].as_str() {
            "--profile" if idx + 1 < args.len() => {
                profile = args[idx + 1].clone();
                idx += 1;
            }
            "--json" => {
                json_output = true;
            }
            _ => {}
        }
        idx += 1;
    }

    let profiles = load_profile_configs()?;
    let thresholds = load_thresholds()?;
    let names = if profile == "ci" {
        vec![
            "rpc_health",
            "session_lifecycle",
            "terminal_interactive",
            "browser_navigation",
            "browser_fanout",
            "restart_recovery",
        ]
    } else {
        vec![profile.as_str()]
    };

    let mut results = Vec::new();
    for name in names {
        let profile_cfg = profiles
            .get(name)
            .cloned()
            .unwrap_or_else(|| default_profile(name));
        let threshold = thresholds.get(name).cloned().unwrap_or_default();
        let result = run_profile(name, &profile_cfg, &threshold).await?;
        results.push(result);
    }

    let ok = results.iter().all(|result| result.pass);
    if json_output {
        let payload = json!({
            "suite": profile,
            "pass": ok,
            "results": results.iter().map(|result| {
                json!({
                    "profile": result.profile,
                    "iterations": result.iterations,
                    "p50_ms": result.p50_ms,
                    "p95_ms": result.p95_ms,
                    "max_ms": result.max_ms,
                    "throughput_ops_per_sec": result.throughput_ops_per_sec,
                    "pass": result.pass
                })
            }).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        for result in &results {
            println!(
                "{}: p50={:.2}ms p95={:.2}ms max={:.2}ms throughput={:.2}/s pass={}",
                result.profile,
                result.p50_ms,
                result.p95_ms,
                result.max_ms,
                result.throughput_ops_per_sec,
                result.pass
            );
        }
    }

    if ok {
        Ok(())
    } else {
        Err("performance thresholds failed".into())
    }
}

async fn run_profile(
    name: &str,
    profile_cfg: &ProfileConfig,
    threshold: &ThresholdConfig,
) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
    let mut samples = Vec::new();
    let started = Instant::now();
    let event_dir = temp_event_dir(name);
    let server = RpcServer::new(BackendConfig {
        event_dir: event_dir.to_string_lossy().to_string(),
        ..BackendConfig::default()
    })?;

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
                        "auth":{"token": token}
                    }
                }),
            )
            .await?;
            let terminal_session_id = spawned["result"]["terminal_session_id"]
                .as_str()
                .ok_or("missing terminal session id")?
                .to_string();
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
                        "input": format!("echo {iter}")
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
        other => {
            return Err(format!("unknown profile: {other}").into());
        }
    }

    let total_s = started.elapsed().as_secs_f64().max(0.000_001);
    let result = summarize(
        name,
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

fn load_profile_configs() -> Result<BTreeMap<String, ProfileConfig>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(data_path("perf-profiles.json"))?;
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

fn load_thresholds() -> Result<BTreeMap<String, ThresholdConfig>, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(data_path("perf-baseline.json"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaders_and_defaults_work() {
        let profiles = load_profile_configs().expect("profiles");
        assert!(profiles.contains_key("rpc_health"));
        let thresholds = load_thresholds().expect("thresholds");
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
    }

    #[test]
    fn summarize_and_percentiles_are_stable() {
        let result = summarize(
            "demo",
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

    #[tokio::test]
    async fn run_rpc_health_profile() {
        let result = run_profile(
            "rpc_health",
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
            &ProfileConfig {
                iterations: 2,
                warmup: 0,
                fixture_events: 0,
            },
            &ThresholdConfig {
                p95_ms: Some(100.0),
                max_ms: None,
            },
        )
        .await
        .expect("terminal profile");
        assert!(terminal.pass);

        let recovery = run_profile(
            "restart_recovery",
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
}
