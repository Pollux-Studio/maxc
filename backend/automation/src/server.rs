use crate::{RpcErrorCode, RpcId, RpcRequest, RpcSuccess};
use maxc_browser::{BrowserSessionId, BrowserTabId};
use maxc_core::{BackendConfig, CommandId, SessionScope};
use maxc_security::SessionToken;
use maxc_storage::{
    EventRecord, EventStore, EventStoreConfig, EventType, ProjectionState, SessionProjection,
    StoreError,
};
use maxc_telemetry::{
    LatencyMetric, LogLevel, LogRecord, MetricsSnapshot as TelemetryMetricsSnapshot, SpanRecord,
    TelemetryCollector, TelemetrySnapshot as CollectorTelemetrySnapshot,
};
use rand::RngCore;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
#[cfg(windows)]
use std::fs::File as StdFile;
#[cfg(windows)]
use std::io::BufReader as StdBufReader;
use std::io::{Read, Write};
use std::net::TcpStream;
#[cfg(windows)]
use std::os::windows::io::FromRawHandle;
use std::path::{Path, PathBuf};
use std::process::{Child as StdChild, Command as StdCommand, Stdio as StdStdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{ChildStdin, Command};
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::time::timeout;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{
    CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
};
#[cfg(windows)]
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
#[cfg(windows)]
use windows_sys::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
};
#[cfg(windows)]
use windows_sys::Win32::System::Pipes::CreatePipe;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST,
    PROCESS_INFORMATION, STARTUPINFOEXW,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub token: String,
    pub scopes: Vec<String>,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub last_seen_ms: u64,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrowserAuditContext {
    workspace_id: String,
    surface_id: String,
    browser_session_id: Option<String>,
    tab_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalAuditContext {
    workspace_id: String,
    surface_id: String,
    terminal_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct DependencyStatus {
    terminal_runtime_ready: bool,
    browser_runtime_ready: bool,
    artifact_root_ready: bool,
    event_store_ready: bool,
}

impl DependencyStatus {
    fn ready(self) -> bool {
        self.terminal_runtime_ready
            && self.browser_runtime_ready
            && self.artifact_root_ready
            && self.event_store_ready
    }
}

impl SessionRecord {
    fn from_projection(value: &SessionProjection) -> Self {
        Self {
            token: value.token.clone(),
            scopes: value.scopes.clone(),
            issued_at_ms: value.issued_at_ms,
            expires_at_ms: value.expires_at_ms,
            last_seen_ms: value.last_seen_ms,
            revoked: value.revoked,
        }
    }

    fn is_active(&self, now_ms: u64) -> bool {
        !self.revoked && self.expires_at_ms > now_ms
    }

    fn has_scope(&self, scope: SessionScope) -> bool {
        self.scopes.iter().any(|value| value == scope.as_str())
    }
}

#[derive(Debug, Error)]
pub enum RpcServerInitError {
    #[error("event store initialization failed: {0}")]
    Store(#[from] StoreError),
}

#[derive(Debug)]
struct ServerState {
    projection: Mutex<ProjectionState>,
    store: Mutex<EventStore>,
    global_limiter: Mutex<RateLimiter>,
    connection_limiters: Mutex<HashMap<String, RateLimiter>>,
    raw_limiters: Mutex<HashMap<String, RateLimiter>>,
    terminal_runtime: Mutex<HashMap<String, TerminalSessionRuntime>>,
    browser_runtime: Mutex<HashMap<String, BrowserSessionRuntime>>,
    agent_workers: Mutex<HashMap<String, AgentWorkerRuntime>>,
    agent_tasks: Mutex<HashMap<String, AgentTaskRuntime>>,
    terminal_subscriptions: StdMutex<HashMap<String, HashMap<String, SubscriptionState>>>,
    browser_subscriptions: StdMutex<HashMap<String, HashMap<String, SubscriptionState>>>,
    scheduler: StdMutex<SchedulerState>,
    breaker: StdMutex<CircuitBreaker>,
    faults: StdMutex<HashMap<FaultHook, FaultAction>>,
    shutting_down: AtomicBool,
    started_at_ms: u64,
    telemetry: StdMutex<TelemetryCollector>,
    metrics: StdMutex<ServerMetrics>,
    inflight_by_connection: StdMutex<HashMap<String, usize>>,
    correlation: AtomicU64,
}

#[derive(Debug, Default)]
struct ServerMetrics {
    counters: BTreeMap<String, u64>,
    gauges: BTreeMap<String, u64>,
    latencies: BTreeMap<String, LatencyMetric>,
}

impl ServerMetrics {
    fn incr_counter(&mut self, name: &str, value: u64) {
        *self.counters.entry(name.to_string()).or_default() += value;
    }

    fn set_gauge(&mut self, name: &str, value: u64) {
        self.gauges.insert(name.to_string(), value);
    }

    fn record_latency(&mut self, name: &str, value_ms: f64) {
        self.latencies
            .entry(name.to_string())
            .or_default()
            .record(value_ms);
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            counters: self.counters.clone(),
            gauges: self.gauges.clone(),
            latencies: self
                .latencies
                .iter()
                .map(|(key, value)| (key.clone(), value.snapshot()))
                .collect(),
        }
    }
}

struct TerminalSessionRuntime {
    workspace_id: String,
    surface_id: String,
    cols: u16,
    rows: u16,
    alive: bool,
    last_output: String,
    program: String,
    cwd: String,
    pid: u32,
    runtime: String,
    status: String,
    exit_code: Option<i32>,
    input: Option<TerminalInputHandle>,
    kill_tx: Option<oneshot::Sender<()>>,
    next_sequence: u64,
    history: VecDeque<Value>,
    history_bytes: usize,
    #[cfg(windows)]
    conpty: Option<Arc<ConptyControl>>,
}

impl std::fmt::Debug for TerminalSessionRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalSessionRuntime")
            .field("cols", &self.cols)
            .field("rows", &self.rows)
            .field("alive", &self.alive)
            .field("last_output", &self.last_output)
            .field("workspace_id", &self.workspace_id)
            .field("surface_id", &self.surface_id)
            .field("program", &self.program)
            .field("cwd", &self.cwd)
            .field("pid", &self.pid)
            .field("runtime", &self.runtime)
            .field("status", &self.status)
            .field("exit_code", &self.exit_code)
            .finish()
    }
}

#[derive(Debug)]
struct TerminalLaunchSpec {
    program: String,
    args: Vec<String>,
    cwd: String,
    env: HashMap<String, String>,
    shell: String,
}

#[derive(Clone)]
enum TerminalInputHandle {
    Process(Arc<Mutex<ChildStdin>>),
    #[cfg(windows)]
    BlockingPipe(Arc<StdMutex<StdFile>>),
}

#[cfg(windows)]
struct ConptyControl {
    hpc: HPCON,
    process_handle: HANDLE,
}

#[cfg(windows)]
unsafe impl Send for ConptyControl {}
#[cfg(windows)]
unsafe impl Sync for ConptyControl {}

#[derive(Debug, Clone)]
struct BrowserSessionRuntime {
    workspace_id: String,
    surface_id: String,
    attached: bool,
    closed: bool,
    status: String,
    tabs: HashMap<String, BrowserTabRuntime>,
    tracing_enabled: bool,
    network_interception: bool,
    runtime: String,
    executable: String,
    last_error: Option<String>,
    process: Option<Arc<BrowserProcessRuntime>>,
    next_sequence: u64,
    history: VecDeque<Value>,
    history_bytes: usize,
}

#[derive(Debug, Clone)]
struct BrowserTabRuntime {
    browser_tab_id: String,
    target_id: String,
    websocket_url: String,
    url: String,
    title: String,
    load_state: String,
    focused: bool,
    closed: bool,
    history: Vec<String>,
    history_index: usize,
    cookies: HashMap<String, String>,
    storage: HashMap<String, String>,
    last_artifact_path: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentAuditContext {
    workspace_id: String,
    surface_id: String,
    agent_worker_id: Option<String>,
    agent_task_id: Option<String>,
}

#[derive(Debug, Clone)]
struct AgentWorkerRuntime {
    workspace_id: String,
    surface_id: String,
    status: String,
    terminal_session_id: String,
    browser_session_id: Option<String>,
    current_task_id: Option<String>,
    closed: bool,
}

#[derive(Debug, Clone)]
struct AgentTaskRuntime {
    agent_worker_id: String,
    prompt: String,
    status: String,
    terminal_session_id: String,
    browser_session_id: Option<String>,
    last_output_sequence: u64,
    failure_reason: Option<String>,
}

#[derive(Debug)]
struct BrowserProcessRuntime {
    runtime: String,
    executable: String,
    port: u16,
    http_base_url: String,
    websocket_url: String,
    user_data_dir: PathBuf,
    artifact_dir: PathBuf,
    child: StdMutex<StdChild>,
}

#[derive(Debug, Clone, Default)]
struct ArtifactStats {
    files: u64,
    bytes: u64,
}

#[derive(Debug, Clone)]
struct SubscriptionState {
    queue: VecDeque<Value>,
    dropped_events: u64,
}

impl SubscriptionState {
    fn new(limit: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(limit),
            dropped_events: 0,
        }
    }

    fn push(&mut self, event: Value, limit: usize) {
        self.queue.push_back(event);
        while self.queue.len() > limit {
            self.queue.pop_front();
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
    }

    fn drain(&mut self) -> Vec<Value> {
        self.queue.drain(..).collect()
    }
}

#[derive(Debug, Default, Clone)]
struct SchedulerState {
    interactive_inflight: usize,
    background_inflight: usize,
}

#[derive(Debug, Default, Clone)]
struct CircuitBreaker {
    consecutive_failures: u32,
    open_until_ms: Option<u64>,
    half_open_probe_running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FaultHook {
    StoreAppend,
    Snapshot,
    MethodDispatch,
    Response,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum FaultAction {
    ReturnInternal,
    DelayMs(u64),
    DropResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkloadClass {
    Interactive,
    Background,
}

#[derive(Debug)]
struct SchedulerGuard {
    class: WorkloadClass,
    state: Arc<ServerState>,
}

impl Drop for SchedulerGuard {
    fn drop(&mut self) {
        let mut scheduler = self
            .state
            .scheduler
            .lock()
            .expect("scheduler lock poisoned");
        match self.class {
            WorkloadClass::Interactive => {
                scheduler.interactive_inflight = scheduler.interactive_inflight.saturating_sub(1);
            }
            WorkloadClass::Background => {
                scheduler.background_inflight = scheduler.background_inflight.saturating_sub(1);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct RpcServer {
    config: BackendConfig,
    state: Arc<ServerState>,
}

pub type MetricsSnapshot = TelemetryMetricsSnapshot;
pub type TelemetrySnapshot = CollectorTelemetrySnapshot;

#[derive(Debug, Error)]
enum ServerError {
    #[error("invalid request")]
    InvalidRequest,
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("timeout")]
    Timeout,
    #[error("rate limited")]
    RateLimited,
    #[error("internal")]
    Internal,
}

#[derive(Debug, Clone)]
struct RateLimiter {
    tokens: f64,
    burst: f64,
    rate_per_sec: f64,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(rate_per_sec: u32, burst: u32) -> Self {
        Self {
            tokens: f64::from(burst),
            burst: f64::from(burst),
            rate_per_sec: f64::from(rate_per_sec),
            last_refill: Instant::now(),
        }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * self.rate_per_sec).min(self.burst);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
struct InflightGuard {
    connection_id: String,
    state: Arc<ServerState>,
}

impl InflightGuard {
    fn acquire(
        state: Arc<ServerState>,
        connection_id: &str,
        limit: usize,
    ) -> Result<Self, ServerError> {
        let mut map = state
            .inflight_by_connection
            .lock()
            .expect("inflight lock poisoned");
        let count = map.entry(connection_id.to_string()).or_insert(0);
        if *count >= limit {
            return Err(ServerError::RateLimited);
        }
        *count += 1;
        drop(map);
        Ok(Self {
            connection_id: connection_id.to_string(),
            state,
        })
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        let mut map = self
            .state
            .inflight_by_connection
            .lock()
            .expect("inflight lock poisoned");
        if let Some(count) = map.get_mut(&self.connection_id) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                map.remove(&self.connection_id);
            }
        }
    }
}

impl RpcServer {
    pub fn new(config: BackendConfig) -> Result<Self, RpcServerInitError> {
        Self::new_with_faults_internal(config, HashMap::new())
    }

    fn new_with_faults_internal(
        config: BackendConfig,
        faults: HashMap<FaultHook, FaultAction>,
    ) -> Result<Self, RpcServerInitError> {
        let store_cfg = EventStoreConfig {
            event_dir: std::path::PathBuf::from(config.event_dir.clone()),
            segment_max_bytes: config.segment_max_bytes,
            snapshot_interval_events: config.snapshot_interval_events,
            snapshot_retain_count: config.snapshot_retain_count,
        };
        let mut store = EventStore::new(store_cfg)?;
        let projection = store.recover()?;
        let global_limiter = RateLimiter::new(config.rate_limit_per_sec, config.burst_limit);
        let server = Self {
            config,
            state: Arc::new(ServerState {
                projection: Mutex::new(projection),
                store: Mutex::new(store),
                global_limiter: Mutex::new(global_limiter),
                connection_limiters: Mutex::new(HashMap::new()),
                raw_limiters: Mutex::new(HashMap::new()),
                terminal_runtime: Mutex::new(HashMap::new()),
                browser_runtime: Mutex::new(HashMap::new()),
                agent_workers: Mutex::new(HashMap::new()),
                agent_tasks: Mutex::new(HashMap::new()),
                terminal_subscriptions: StdMutex::new(HashMap::new()),
                browser_subscriptions: StdMutex::new(HashMap::new()),
                scheduler: StdMutex::new(SchedulerState::default()),
                breaker: StdMutex::new(CircuitBreaker::default()),
                faults: StdMutex::new(faults),
                shutting_down: AtomicBool::new(false),
                started_at_ms: now_unix_ms(),
                telemetry: StdMutex::new(TelemetryCollector::new(256)),
                metrics: StdMutex::new(ServerMetrics::default()),
                inflight_by_connection: StdMutex::new(HashMap::new()),
                correlation: AtomicU64::new(1),
            }),
        };
        let _ = server.enforce_artifact_retention(None);
        Ok(server)
    }

    #[cfg(test)]
    fn new_with_faults(
        config: BackendConfig,
        faults: HashMap<FaultHook, FaultAction>,
    ) -> Result<Self, RpcServerInitError> {
        Self::new_with_faults_internal(config, faults)
    }

    pub async fn handle_json_line(&self, connection_id: &str, line: &str) -> String {
        let corr = self.next_correlation_id();
        let started = Instant::now();
        let method = extract_method_from_raw_json(line);
        self.log_event(
            LogLevel::Info,
            "rpc",
            "request.start",
            &corr,
            Some(connection_id.to_string()),
            method.clone(),
            None,
            None,
            "started",
            BTreeMap::new(),
        );
        let result = self
            .handle_json_line_inner(connection_id, line)
            .await
            .unwrap_or_else(|err| {
                self.record_request_metrics(method.as_deref(), started.elapsed(), false);
                let mut fields = BTreeMap::new();
                fields.insert("error_code".to_string(), json!(map_error_code(&err)));
                self.log_event(
                    LogLevel::Error,
                    "rpc",
                    "request.error",
                    &corr,
                    Some(connection_id.to_string()),
                    method.clone(),
                    None,
                    Some(started.elapsed().as_millis() as u64),
                    "error",
                    fields,
                );
                let id = extract_id_from_raw_json(line);
                json!({
                    "id": id,
                    "error": {
                        "code": map_error_code(&err),
                        "message": err.to_string(),
                        "data": {
                            "correlation_id": corr
                        }
                    }
                })
            });

        if result.get("result").is_some() {
            self.record_request_metrics(method.as_deref(), started.elapsed(), true);
            self.log_event(
                LogLevel::Info,
                "rpc",
                "request.finish",
                &corr,
                Some(connection_id.to_string()),
                method,
                None,
                Some(started.elapsed().as_millis() as u64),
                "ok",
                BTreeMap::new(),
            );
        }

        serde_json::to_string(&result).unwrap_or_else(|_| {
            json!({
                "id": Value::Null,
                "error": {
                    "code": RpcErrorCode::Internal,
                    "message": "internal",
                    "data": {
                        "correlation_id": corr
                    }
                }
            })
            .to_string()
        })
    }

    async fn handle_json_line_inner(
        &self,
        connection_id: &str,
        line: &str,
    ) -> Result<Value, ServerError> {
        if self.is_shutting_down() {
            return Err(ServerError::RateLimited);
        }
        if line.len() > self.config.max_payload_bytes {
            return Err(ServerError::InvalidRequest);
        }
        self.check_limits(connection_id).await?;

        let request: RpcRequest =
            serde_json::from_str(line).map_err(|_| ServerError::InvalidRequest)?;
        request
            .validate()
            .map_err(|_| ServerError::InvalidRequest)?;
        self.handle_request(connection_id, request).await
    }

    async fn handle_request(
        &self,
        connection_id: &str,
        request: RpcRequest,
    ) -> Result<Value, ServerError> {
        self.check_breaker()?;
        let span_started = now_unix_ms();
        let method_name = request.method.clone();
        let id = request.id.clone().unwrap_or(RpcId::Null);
        let _guard = InflightGuard::acquire(
            Arc::clone(&self.state),
            connection_id,
            self.config.max_inflight_per_connection,
        )?;

        let timeout_duration = Duration::from_millis(self.config.request_timeout_ms);
        let response = match timeout(timeout_duration, self.dispatch(request)).await {
            Ok(Ok(response)) => response,
            Ok(Err(err)) => {
                self.record_failure(&err);
                return Err(err);
            }
            Err(_) => {
                self.record_failure(&ServerError::Timeout);
                return Err(ServerError::Timeout);
            }
        };
        if let Err(err) = self.apply_fault(FaultHook::Response).await {
            self.record_failure(&err);
            return Err(err);
        }
        self.record_success();
        let mut attrs = BTreeMap::new();
        attrs.insert("connection_id".to_string(), json!(connection_id));
        attrs.insert("method".to_string(), json!(method_name));
        self.record_span(
            "rpc.request",
            &self.next_correlation_id(),
            span_started,
            now_unix_ms().saturating_sub(span_started),
            attrs,
        );

        serde_json::to_value(RpcSuccess {
            id,
            result: response,
        })
        .map_err(|_| ServerError::Internal)
    }

    async fn check_limits(&self, connection_id: &str) -> Result<(), ServerError> {
        let total_inflight = {
            let inflight = self
                .state
                .inflight_by_connection
                .lock()
                .expect("inflight lock poisoned");
            inflight.values().copied().sum::<usize>()
        };
        if total_inflight >= self.config.overload_reject_threshold {
            return Err(ServerError::RateLimited);
        }
        {
            let mut global = self.state.global_limiter.lock().await;
            if !global.allow() {
                return Err(ServerError::RateLimited);
            }
        }
        let mut map = self.state.connection_limiters.lock().await;
        let limiter = map.entry(connection_id.to_string()).or_insert_with(|| {
            RateLimiter::new(self.config.rate_limit_per_sec, self.config.burst_limit)
        });
        if limiter.allow() {
            Ok(())
        } else {
            Err(ServerError::RateLimited)
        }
    }

    async fn dispatch(&self, request: RpcRequest) -> Result<Value, ServerError> {
        self.apply_fault(FaultHook::MethodDispatch).await?;
        match request.method.as_str() {
            "session.create" => self.session_create(request.params).await,
            "session.refresh" => self.session_refresh(request.params).await,
            "session.revoke" => self.session_revoke(request.params).await,
            "system.health" => self.system_health().await,
            "system.readiness" => self.system_readiness(request.params).await,
            "system.diagnostics" => self.system_diagnostics(request.params).await,
            "system.metrics" => self.system_metrics(request.params).await,
            "system.logs" => self.system_logs(request.params).await,
            method if method.starts_with("terminal.") => {
                self.terminal_dispatch(method, request.params).await
            }
            method if method.starts_with("browser.") => {
                self.browser_dispatch(method, request.params).await
            }
            method if method.starts_with("agent.") => {
                self.agent_dispatch(method, request.params).await
            }
            _ => Err(ServerError::NotFound),
        }
    }

    async fn system_health(&self) -> Result<Value, ServerError> {
        Ok(json!({
            "ok": true,
            "version": env!("CARGO_PKG_VERSION"),
            "shutting_down": self.is_shutting_down(),
            "breaker_open": self.breaker_is_open(),
            "active_requests": self.active_request_count(),
            "uptime_ms": now_unix_ms().saturating_sub(self.state.started_at_ms)
        }))
    }

    async fn system_readiness(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session_scope(params.as_ref(), SessionScope::Diagnostics)
            .await?;
        let dependency = self.dependency_status();
        Ok(json!({
            "ready": !self.is_shutting_down() && !self.breaker_is_open() && dependency.ready(),
            "accepting_requests": !self.is_shutting_down(),
            "breaker_open": self.breaker_is_open(),
            "queue_saturated": self.active_request_count() >= self.config.overload_reject_threshold,
            "store_available": dependency.event_store_ready,
            "browser_runtime_ready": dependency.browser_runtime_ready,
            "terminal_runtime_ready": dependency.terminal_runtime_ready,
            "artifact_root_ready": dependency.artifact_root_ready,
            "event_store_ready": dependency.event_store_ready
        }))
    }

    async fn system_diagnostics(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session_scope(params.as_ref(), SessionScope::Diagnostics)
            .await?;
        let projection = self.state.projection.lock().await;
        let terminal_runtime = self.state.terminal_runtime.lock().await;
        let browser_runtime = self.state.browser_runtime.lock().await;
        let agent_workers = self.state.agent_workers.lock().await;
        let agent_tasks = self.state.agent_tasks.lock().await;
        let terminal_subscriptions = self
            .state
            .terminal_subscriptions
            .lock()
            .expect("subscription lock poisoned");
        let browser_subscriptions = self
            .state
            .browser_subscriptions
            .lock()
            .expect("subscription lock poisoned");
        let metrics = self.metrics_snapshot();
        let artifact_stats = self.collect_artifact_stats();
        let dependency = self.dependency_status();
        let browser_runtime_backend = {
            let mut names = browser_runtime
                .values()
                .map(|session| session.runtime.as_str())
                .collect::<Vec<_>>();
            names.sort_unstable();
            names.dedup();
            match names.as_slice() {
                [] => {
                    if browser_dependency_ready(&self.config) {
                        preferred_browser_runtime_name(&self.config)
                    } else {
                        "browser-simulated"
                    }
                }
                [single] => single,
                _ => "mixed",
            }
        };
        Ok(json!({
            "sessions": projection.sessions.len(),
            "browser_sessions": projection.browser_sessions.len(),
            "browser_tabs": projection.browser_tabs.len(),
            "terminal_runtime_count": terminal_runtime.len(),
            "browser_runtime_count": browser_runtime.len(),
            "browser_runtime_backend": browser_runtime_backend,
            "browser_runtime_ready": dependency.browser_runtime_ready,
            "browser_runtimes": browser_runtime.values().map(|session| json!({
                "workspace_id": session.workspace_id,
                "surface_id": session.surface_id,
                "runtime": session.runtime,
                "status": session.status,
                "executable": session.executable,
                "closed": session.closed,
                "attached": session.attached,
                "last_error": session.last_error,
                "history_events": session.history.len(),
                "history_bytes": session.history_bytes
            })).collect::<Vec<_>>(),
            "agent_workers": projection.agent_workers.values().map(|worker| json!({
                "agent_worker_id": worker.agent_worker_id,
                "workspace_id": worker.workspace_id,
                "surface_id": worker.surface_id,
                "status": worker.status,
                "terminal_session_id": worker.terminal_session_id,
                "browser_session_id": worker.browser_session_id,
                "closed": worker.closed
            })).collect::<Vec<_>>(),
            "agent_tasks": projection.agent_tasks.values().map(|task| json!({
                "agent_task_id": task.agent_task_id,
                "agent_worker_id": task.agent_worker_id,
                "status": task.status,
                "terminal_session_id": task.terminal_session_id,
                "browser_session_id": task.browser_session_id,
                "last_output_sequence": task.last_output_sequence,
                "failure_reason": task.failure_reason
            })).collect::<Vec<_>>(),
            "agent_runtime_count": agent_workers.len(),
            "agent_task_runtime_count": agent_tasks.len(),
            "terminal_subscription_count": terminal_subscriptions.values().map(|v| v.len()).sum::<usize>(),
            "browser_subscription_count": browser_subscriptions.values().map(|v| v.len()).sum::<usize>(),
            "terminal_history_events": terminal_runtime.values().map(|session| session.history.len()).sum::<usize>(),
            "terminal_history_bytes": terminal_runtime.values().map(|session| session.history_bytes).sum::<usize>(),
            "browser_history_events": browser_runtime.values().map(|session| session.history.len()).sum::<usize>(),
            "browser_history_bytes": browser_runtime.values().map(|session| session.history_bytes).sum::<usize>(),
            "artifact_files": artifact_stats.files,
            "artifact_bytes": artifact_stats.bytes,
            "terminal_runtime_backend": selected_terminal_runtime_name(&self.config),
            "terminal_runtime_ready": dependency.terminal_runtime_ready,
            "artifact_root_ready": dependency.artifact_root_ready,
            "event_store_ready": dependency.event_store_ready,
            "active_requests": self.active_request_count(),
            "shutting_down": self.is_shutting_down(),
            "breaker_open": self.breaker_is_open(),
            "metrics": metrics
        }))
    }

    async fn system_metrics(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session_scope(params.as_ref(), SessionScope::Diagnostics)
            .await?;
        let mut metrics = self.metrics_snapshot();
        let artifact_stats = self.collect_artifact_stats();
        let dependency = self.dependency_status();
        metrics.gauges.insert(
            "rpc.active_requests".to_string(),
            self.active_request_count() as u64,
        );
        metrics.gauges.insert(
            "runtime.terminal.sessions".to_string(),
            self.state.terminal_runtime.lock().await.len() as u64,
        );
        metrics.gauges.insert(
            "runtime.terminal.history_events".to_string(),
            self.state
                .terminal_runtime
                .lock()
                .await
                .values()
                .map(|session| session.history.len() as u64)
                .sum::<u64>(),
        );
        metrics.gauges.insert(
            "runtime.terminal.history_bytes".to_string(),
            self.state
                .terminal_runtime
                .lock()
                .await
                .values()
                .map(|session| session.history_bytes as u64)
                .sum::<u64>(),
        );
        metrics.gauges.insert(
            "runtime.browser.sessions".to_string(),
            self.state.browser_runtime.lock().await.len() as u64,
        );
        metrics.gauges.insert(
            "runtime.browser.history_events".to_string(),
            self.state
                .browser_runtime
                .lock()
                .await
                .values()
                .map(|session| session.history.len() as u64)
                .sum::<u64>(),
        );
        metrics.gauges.insert(
            "runtime.browser.history_bytes".to_string(),
            self.state
                .browser_runtime
                .lock()
                .await
                .values()
                .map(|session| session.history_bytes as u64)
                .sum::<u64>(),
        );
        metrics.gauges.insert(
            "runtime.browser.ready".to_string(),
            u64::from(dependency.browser_runtime_ready),
        );
        metrics.gauges.insert(
            "runtime.terminal.ready".to_string(),
            u64::from(dependency.terminal_runtime_ready),
        );
        metrics.gauges.insert(
            "runtime.artifacts.ready".to_string(),
            u64::from(dependency.artifact_root_ready),
        );
        metrics.gauges.insert(
            "storage.event_dir.ready".to_string(),
            u64::from(dependency.event_store_ready),
        );
        metrics.gauges.insert(
            "runtime.agent.workers".to_string(),
            self.state.agent_workers.lock().await.len() as u64,
        );
        metrics.gauges.insert(
            "runtime.agent.tasks".to_string(),
            self.state.agent_tasks.lock().await.len() as u64,
        );
        metrics
            .gauges
            .insert("runtime.artifacts.files".to_string(), artifact_stats.files);
        metrics
            .gauges
            .insert("runtime.artifacts.bytes".to_string(), artifact_stats.bytes);
        serde_json::to_value(metrics).map_err(|_| ServerError::Internal)
    }

    async fn system_logs(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session_scope(params.as_ref(), SessionScope::Diagnostics)
            .await?;
        serde_json::to_value(self.telemetry_snapshot()).map_err(|_| ServerError::Internal)
    }

    async fn session_create(&self, params: Option<Value>) -> Result<Value, ServerError> {
        let command_id = extract_command_id(params.as_ref())?;
        if let Some(existing) = self.lookup_command_result(&command_id).await {
            return Ok(existing);
        }

        let now = now_unix_ms();
        let ttl = self.config.session_ttl_ms;
        let token = random_token()?;
        let scopes = extract_requested_scopes(params.as_ref(), &self.config)?;
        let result = json!({
            "token": token,
            "scopes": scopes,
            "issued_at_ms": now,
            "expires_at_ms": now + ttl
        });
        let payload = json!({
            "token": token,
            "scopes": result["scopes"],
            "issued_at_ms": now,
            "expires_at_ms": now + ttl,
            "last_seen_ms": now,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::SessionCreated,
            token.clone(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn session_refresh(&self, params: Option<Value>) -> Result<Value, ServerError> {
        let token = extract_token(params.as_ref()).ok_or(ServerError::Unauthorized)?;
        let command_id = extract_command_id(params.as_ref())?;
        if let Some(existing) = self.lookup_command_result(&command_id).await {
            return Ok(existing);
        }

        let now = now_unix_ms();
        let ttl = self.config.session_ttl_ms;
        let session = self
            .find_session(&token)
            .await
            .ok_or(ServerError::Unauthorized)?;
        if !session.is_active(now) {
            return Err(ServerError::Unauthorized);
        }
        let scopes = if let Some(requested) = extract_optional_requested_scopes(params.as_ref())? {
            if !requested.iter().all(|scope| session.scopes.contains(scope)) {
                return Err(ServerError::Unauthorized);
            }
            requested
        } else {
            session.scopes.clone()
        };

        let result = json!({
            "token": token,
            "scopes": scopes,
            "expires_at_ms": now + ttl
        });
        let payload = json!({
            "token": token,
            "scopes": result["scopes"],
            "expires_at_ms": now + ttl,
            "last_seen_ms": now,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::SessionRefreshed,
            session.token,
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn session_revoke(&self, params: Option<Value>) -> Result<Value, ServerError> {
        let token = extract_token(params.as_ref()).ok_or(ServerError::Unauthorized)?;
        let command_id = extract_command_id(params.as_ref())?;
        if let Some(existing) = self.lookup_command_result(&command_id).await {
            return Ok(existing);
        }

        self.find_session(&token)
            .await
            .ok_or(ServerError::Unauthorized)?;

        let result = json!({
            "revoked": true
        });
        let payload = json!({
            "token": token,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::SessionRevoked,
            token,
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_dispatch(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, ServerError> {
        let params_ref = params.as_ref().ok_or(ServerError::InvalidRequest)?;
        let _session = self
            .require_active_session_scope(params.as_ref(), SessionScope::Runtime)
            .await?;

        let command_id = extract_command_id(params.as_ref())?;
        if let Some(existing) = self.lookup_command_result(&command_id).await {
            return Ok(existing);
        }

        let audit = extract_browser_audit(params_ref, method)?;
        self.enforce_browser_limits(method, params_ref, &audit)
            .await?;
        let _scheduler = self.acquire_scheduler(method)?;

        match method {
            "browser.create" => self.browser_create(command_id, audit).await,
            "browser.attach" => self.browser_attach(command_id, audit).await,
            "browser.detach" => self.browser_detach(command_id, audit).await,
            "browser.close" => self.browser_close(command_id, audit).await,
            "browser.tab.open" => self.browser_tab_open(command_id, audit, params_ref).await,
            "browser.tab.list" => self.browser_tab_list(audit).await,
            "browser.tab.focus" => self.browser_tab_focus(command_id, audit).await,
            "browser.tab.close" => self.browser_tab_close(command_id, audit).await,
            "browser.goto" | "browser.reload" | "browser.back" | "browser.forward" => {
                self.browser_navigation(command_id, audit, method, params_ref)
                    .await
            }
            "browser.click"
            | "browser.type"
            | "browser.key"
            | "browser.wait"
            | "browser.screenshot"
            | "browser.evaluate"
            | "browser.storage.get"
            | "browser.storage.set"
            | "browser.network.intercept"
            | "browser.cookie.get"
            | "browser.cookie.set"
            | "browser.upload"
            | "browser.download"
            | "browser.trace.start"
            | "browser.trace.stop" => {
                self.browser_automation(command_id, audit, method, params_ref)
                    .await
            }
            "browser.history" => self.browser_history(audit, params_ref).await,
            "browser.subscribe" => self.browser_subscribe(command_id, audit, method).await,
            "browser.raw.command" => self.browser_raw(command_id, audit, params_ref).await,
            _ => Err(ServerError::NotFound),
        }
    }

    async fn enforce_browser_limits(
        &self,
        method: &str,
        params: &Value,
        audit: &BrowserAuditContext,
    ) -> Result<(), ServerError> {
        if method == "browser.raw.command" {
            if !self.config.browser_allow_raw_commands {
                self.state
                    .metrics
                    .lock()
                    .expect("metrics lock poisoned")
                    .incr_counter("policy.browser.raw.rejected", 1);
                return Err(ServerError::Unauthorized);
            }
            let allow_raw = params
                .get("allow_raw")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !allow_raw {
                return Err(ServerError::Unauthorized);
            }
            let mut map = self.state.raw_limiters.lock().await;
            let limiter = map.entry(audit.workspace_id.clone()).or_insert_with(|| {
                RateLimiter::new(
                    self.config.browser_raw_rate_limit_per_sec,
                    self.config.browser_raw_rate_limit_per_sec,
                )
            });
            if !limiter.allow() {
                return Err(ServerError::RateLimited);
            }
        }
        if method == "browser.upload" {
            if let Some(path) = params.get("path").and_then(Value::as_str) {
                if !is_path_allowed(Path::new(path), &self.config.browser_allowed_upload_roots) {
                    self.state
                        .metrics
                        .lock()
                        .expect("metrics lock poisoned")
                        .incr_counter("policy.browser.upload.rejected", 1);
                    return Err(ServerError::RateLimited);
                }
            }
        }
        if method == "browser.download"
            && !self.config.browser_allowed_download_roots.is_empty()
            && !is_path_allowed(
                &self.artifact_root_path(),
                &self.config.browser_allowed_download_roots,
            )
        {
            self.state
                .metrics
                .lock()
                .expect("metrics lock poisoned")
                .incr_counter("policy.browser.download.rejected", 1);
            return Err(ServerError::RateLimited);
        }
        if matches!(method, "browser.trace.start" | "browser.trace.stop")
            && !self.config.browser_allowed_trace_roots.is_empty()
            && !is_path_allowed(
                &self.artifact_root_path(),
                &self.config.browser_allowed_trace_roots,
            )
        {
            self.state
                .metrics
                .lock()
                .expect("metrics lock poisoned")
                .incr_counter("policy.browser.trace.rejected", 1);
            return Err(ServerError::RateLimited);
        }

        if method == "browser.screenshot" {
            let requested = params
                .get("expected_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if requested > self.config.browser_screenshot_max_bytes as u64 {
                return Err(ServerError::InvalidRequest);
            }
        }
        if method == "browser.download" {
            let requested = params
                .get("size_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if requested > self.config.browser_download_max_bytes as u64 {
                return Err(ServerError::InvalidRequest);
            }
        }
        Ok(())
    }

    fn acquire_scheduler(&self, method: &str) -> Result<SchedulerGuard, ServerError> {
        let class = workload_class(method);
        let mut scheduler = self
            .state
            .scheduler
            .lock()
            .expect("scheduler lock poisoned");
        match class {
            WorkloadClass::Interactive => {
                let max = self.config.max_inflight_per_connection.max(1);
                if scheduler.interactive_inflight >= max {
                    return Err(ServerError::RateLimited);
                }
                scheduler.interactive_inflight += 1;
            }
            WorkloadClass::Background => {
                let max = (self.config.queue_limit / 8).max(1);
                if scheduler.background_inflight >= max {
                    return Err(ServerError::RateLimited);
                }
                scheduler.background_inflight += 1;
            }
        }
        drop(scheduler);
        Ok(SchedulerGuard {
            class,
            state: Arc::clone(&self.state),
        })
    }

    async fn terminal_dispatch(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, ServerError> {
        let params_ref = params.as_ref().ok_or(ServerError::InvalidRequest)?;
        let _session = self
            .require_active_session_scope(params.as_ref(), SessionScope::Runtime)
            .await?;

        let command_id = extract_command_id(params.as_ref())?;
        if let Some(existing) = self.lookup_command_result(&command_id).await {
            return Ok(existing);
        }
        let audit = extract_terminal_audit(params_ref, method)?;
        let _scheduler = self.acquire_scheduler(method)?;

        match method {
            "terminal.spawn" => self.terminal_spawn(command_id, audit, params_ref).await,
            "terminal.input" => self.terminal_input(command_id, audit, params_ref).await,
            "terminal.resize" => self.terminal_resize(command_id, audit, params_ref).await,
            "terminal.history" => self.terminal_history(audit, params_ref).await,
            "terminal.kill" => self.terminal_kill(command_id, audit).await,
            "terminal.subscribe" => self.terminal_subscribe(command_id, audit).await,
            _ => Err(ServerError::NotFound),
        }
    }

    async fn agent_dispatch(
        &self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, ServerError> {
        let params_ref = params.as_ref().ok_or(ServerError::InvalidRequest)?;
        let _session = self
            .require_active_session_scope(params.as_ref(), SessionScope::Agent)
            .await?;

        let command_id = extract_command_id(params.as_ref())?;
        if let Some(existing) = self.lookup_command_result(&command_id).await {
            return Ok(existing);
        }
        let audit = extract_agent_audit(params_ref, method)?;
        let _scheduler = self.acquire_scheduler(method)?;

        match method {
            "agent.worker.create" => {
                self.agent_worker_create(command_id, audit, params_ref)
                    .await
            }
            "agent.worker.list" => self.agent_worker_list(audit).await,
            "agent.worker.get" => self.agent_worker_get(audit).await,
            "agent.worker.close" => self.agent_worker_close(command_id, audit).await,
            "agent.task.start" => self.agent_task_start(command_id, audit, params_ref).await,
            "agent.task.cancel" => self.agent_task_cancel(command_id, audit, params_ref).await,
            "agent.task.list" => self.agent_task_list(audit).await,
            "agent.task.get" => self.agent_task_get(audit).await,
            "agent.attach.terminal" => {
                self.agent_attach_terminal(command_id, audit, params_ref)
                    .await
            }
            "agent.detach.terminal" => self.agent_detach_terminal(command_id, audit).await,
            "agent.attach.browser" => {
                self.agent_attach_browser(command_id, audit, params_ref)
                    .await
            }
            "agent.detach.browser" => self.agent_detach_browser(command_id, audit).await,
            _ => Err(ServerError::NotFound),
        }
    }

    async fn agent_worker_create(
        &self,
        command_id: String,
        audit: AgentAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        {
            let workers = self.state.agent_workers.lock().await;
            if workers.values().filter(|worker| !worker.closed).count()
                >= self.config.agent_max_workers
            {
                return Err(ServerError::RateLimited);
            }
        }
        let cwd = params.get("cwd").and_then(Value::as_str).unwrap_or(".");
        if !self.config.agent_allowed_workspace_roots.is_empty()
            && !self
                .config
                .agent_allowed_workspace_roots
                .iter()
                .any(|root| cwd.starts_with(root))
        {
            return Err(ServerError::RateLimited);
        }
        if !self.config.agent_allowed_programs.is_empty()
            && params
                .get("program")
                .and_then(Value::as_str)
                .is_some_and(|program| {
                    !self
                        .config
                        .agent_allowed_programs
                        .iter()
                        .any(|allowed| allowed.eq_ignore_ascii_case(program))
                })
        {
            return Err(ServerError::RateLimited);
        }
        let worker_id = format!(
            "aw-{}",
            random_token()?.chars().take(12).collect::<String>()
        );
        let spawn_result = self
            .terminal_spawn(
                format!("{command_id}-worker-spawn"),
                TerminalAuditContext {
                    workspace_id: audit.workspace_id.clone(),
                    surface_id: audit.surface_id.clone(),
                    terminal_session_id: None,
                },
                params,
            )
            .await?;
        let terminal_session_id = spawn_result
            .get("terminal_session_id")
            .and_then(Value::as_str)
            .ok_or(ServerError::Internal)?
            .to_string();
        let browser_session_id = params
            .get("browser_session_id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        if let Some(browser_session_id) = browser_session_id.as_ref() {
            self.ensure_browser_unowned(browser_session_id, None)
                .await?;
            self.ensure_browser_exists(browser_session_id).await?;
        }
        {
            let mut workers = self.state.agent_workers.lock().await;
            workers.insert(
                worker_id.clone(),
                AgentWorkerRuntime {
                    workspace_id: audit.workspace_id.clone(),
                    surface_id: audit.surface_id.clone(),
                    status: "ready".to_string(),
                    terminal_session_id: terminal_session_id.clone(),
                    browser_session_id: browser_session_id.clone(),
                    current_task_id: None,
                    closed: false,
                },
            );
        }
        let result = json!({
            "agent_worker_id": worker_id,
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "status": "ready",
            "terminal_session_id": terminal_session_id,
            "browser_session_id": browser_session_id
        });
        let payload = json!({
            "agent_worker_id": result["agent_worker_id"],
            "workspace_id": result["workspace_id"],
            "surface_id": result["surface_id"],
            "terminal_session_id": result["terminal_session_id"],
            "browser_session_id": result["browser_session_id"],
            "status": result["status"],
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentWorkerCreated,
            payload["agent_worker_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn agent_worker_list(&self, audit: AgentAuditContext) -> Result<Value, ServerError> {
        let workers = self.state.agent_workers.lock().await;
        let tasks = self.state.agent_tasks.lock().await;
        let list = workers
            .iter()
            .filter(|(_, worker)| {
                worker.workspace_id == audit.workspace_id && worker.surface_id == audit.surface_id
            })
            .map(|(worker_id, worker)| {
                let current_task = worker.current_task_id.as_ref().and_then(|task_id| {
                    tasks.get(task_id).map(|task| {
                        json!({
                            "agent_task_id": task_id,
                            "status": task.status,
                            "last_output_sequence": task.last_output_sequence,
                            "failure_reason": task.failure_reason
                        })
                    })
                });
                json!({
                    "agent_worker_id": worker_id,
                    "workspace_id": worker.workspace_id,
                    "surface_id": worker.surface_id,
                    "status": worker.status,
                    "terminal_session_id": worker.terminal_session_id,
                    "browser_session_id": worker.browser_session_id,
                    "current_task_id": worker.current_task_id,
                    "closed": worker.closed,
                    "current_task": current_task
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "workers": list }))
    }

    async fn agent_worker_get(&self, audit: AgentAuditContext) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let workers = self.state.agent_workers.lock().await;
        let worker = workers.get(&worker_id).ok_or(ServerError::NotFound)?;
        let task_snapshot = if let Some(task_id) = &worker.current_task_id {
            self.state.agent_tasks.lock().await.get(task_id).cloned()
        } else {
            None
        };
        Ok(json!({
            "agent_worker_id": worker_id,
            "workspace_id": worker.workspace_id,
            "surface_id": worker.surface_id,
            "status": worker.status,
            "terminal_session_id": worker.terminal_session_id,
            "browser_session_id": worker.browser_session_id,
            "current_task_id": worker.current_task_id,
            "closed": worker.closed,
            "current_task": task_snapshot.map(|task| json!({
                "agent_task_id": worker.current_task_id,
                "status": task.status,
                "prompt": redact_text(&task.prompt, 64),
                "prompt_preview": redact_text(&task.prompt, 64),
                "last_output_sequence": task.last_output_sequence,
                "failure_reason": task.failure_reason
            }))
        }))
    }

    async fn agent_worker_close(
        &self,
        command_id: String,
        audit: AgentAuditContext,
    ) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let (terminal_session_id, current_task_id) = {
            let mut workers = self.state.agent_workers.lock().await;
            let worker = workers.get_mut(&worker_id).ok_or(ServerError::NotFound)?;
            if worker.closed {
                return Err(ServerError::Conflict);
            }
            worker.closed = true;
            worker.status = "closed".to_string();
            let current_task_id = worker.current_task_id.take();
            (worker.terminal_session_id.clone(), current_task_id)
        };
        if let Some(task_id) = current_task_id {
            if let Some(task) = self.state.agent_tasks.lock().await.get_mut(&task_id) {
                task.status = "closed".to_string();
                task.failure_reason = Some("worker closed".to_string());
            }
        }
        self.kill_terminal_session(&terminal_session_id).await?;
        let result = json!({
            "agent_worker_id": worker_id,
            "closed": true,
            "terminal_session_id": terminal_session_id
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentWorkerClosed,
            result["agent_worker_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_worker_id": result["agent_worker_id"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn agent_task_start(
        &self,
        command_id: String,
        audit: AgentAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let prompt = params
            .get("prompt")
            .or_else(|| params.get("input"))
            .and_then(Value::as_str)
            .ok_or(ServerError::InvalidRequest)?
            .to_string();
        {
            let tasks = self.state.agent_tasks.lock().await;
            let task_count = tasks
                .values()
                .filter(|task| task.agent_worker_id == worker_id && task.status == "running")
                .count();
            if task_count >= self.config.agent_max_tasks_per_worker {
                return Err(ServerError::RateLimited);
            }
        }
        let (terminal_session_id, browser_session_id) = {
            let mut workers = self.state.agent_workers.lock().await;
            let worker = workers.get_mut(&worker_id).ok_or(ServerError::NotFound)?;
            if worker.closed {
                return Err(ServerError::Conflict);
            }
            if worker.current_task_id.is_some() {
                return Err(ServerError::Conflict);
            }
            worker.status = "running".to_string();
            (
                worker.terminal_session_id.clone(),
                worker.browser_session_id.clone(),
            )
        };
        let terminal_result = self
            .terminal_input(
                format!("{command_id}-task-input"),
                TerminalAuditContext {
                    workspace_id: audit.workspace_id.clone(),
                    surface_id: audit.surface_id.clone(),
                    terminal_session_id: Some(terminal_session_id.clone()),
                },
                &json!({
                    "input": prompt,
                    "workspace_id": audit.workspace_id,
                    "surface_id": audit.surface_id,
                    "terminal_session_id": terminal_session_id,
                    "command_id": format!("{command_id}-task-input")
                }),
            )
            .await?;
        let task_id = format!(
            "at-{}",
            random_token()?.chars().take(12).collect::<String>()
        );
        let last_output_sequence = self.last_terminal_sequence(&terminal_session_id).await;
        {
            let mut tasks = self.state.agent_tasks.lock().await;
            tasks.insert(
                task_id.clone(),
                AgentTaskRuntime {
                    agent_worker_id: worker_id.clone(),
                    prompt: params
                        .get("prompt")
                        .or_else(|| params.get("input"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    status: "running".to_string(),
                    terminal_session_id: terminal_session_id.clone(),
                    browser_session_id: browser_session_id.clone(),
                    last_output_sequence,
                    failure_reason: None,
                },
            );
        }
        {
            let mut workers = self.state.agent_workers.lock().await;
            if let Some(worker) = workers.get_mut(&worker_id) {
                worker.current_task_id = Some(task_id.clone());
            }
        }
        let result = json!({
            "agent_task_id": task_id,
            "agent_worker_id": worker_id,
            "status": "running",
            "terminal_session_id": terminal_session_id,
            "browser_session_id": browser_session_id,
            "last_output_sequence": last_output_sequence,
            "terminal_result": terminal_result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentTaskStarted,
            result["agent_task_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_task_id": result["agent_task_id"],
                "agent_worker_id": result["agent_worker_id"],
                "terminal_session_id": result["terminal_session_id"],
                "browser_session_id": result["browser_session_id"],
                "status": result["status"],
                "last_output_sequence": result["last_output_sequence"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn agent_task_cancel(
        &self,
        command_id: String,
        audit: AgentAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let task_id = audit
            .agent_task_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("cancelled by user")
            .to_string();
        let (worker_id, terminal_session_id) = {
            let mut tasks = self.state.agent_tasks.lock().await;
            let task = tasks.get_mut(&task_id).ok_or(ServerError::NotFound)?;
            if task.status != "running" {
                return Err(ServerError::Conflict);
            }
            task.status = "cancelled".to_string();
            task.failure_reason = Some(reason.clone());
            task.last_output_sequence =
                self.last_terminal_sequence(&task.terminal_session_id).await;
            (
                task.agent_worker_id.clone(),
                task.terminal_session_id.clone(),
            )
        };
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        self.try_interrupt_terminal_session(&terminal_session_id)
            .await?;
        {
            let mut workers = self.state.agent_workers.lock().await;
            if let Some(worker) = workers.get_mut(&worker_id) {
                worker.current_task_id = None;
                if !worker.closed {
                    worker.status = "ready".to_string();
                }
            }
        }
        let result = json!({
            "agent_task_id": task_id,
            "agent_worker_id": worker_id,
            "cancelled": true,
            "reason": reason
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentTaskCancelled,
            result["agent_task_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_task_id": result["agent_task_id"],
                "failure_reason": result["reason"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn agent_task_list(&self, audit: AgentAuditContext) -> Result<Value, ServerError> {
        let tasks = self.state.agent_tasks.lock().await;
        let workers = self.state.agent_workers.lock().await;
        let list = tasks
            .iter()
            .filter(|(_, task)| {
                workers
                    .get(&task.agent_worker_id)
                    .map(|worker| {
                        worker.workspace_id == audit.workspace_id
                            && worker.surface_id == audit.surface_id
                    })
                    .unwrap_or(false)
            })
            .map(|(task_id, task)| {
                json!({
                    "agent_task_id": task_id,
                    "agent_worker_id": task.agent_worker_id,
                    "status": task.status,
                    "prompt": redact_text(&task.prompt, 64),
                    "prompt_preview": redact_text(&task.prompt, 64),
                    "terminal_session_id": task.terminal_session_id,
                    "browser_session_id": task.browser_session_id,
                    "last_output_sequence": task.last_output_sequence,
                    "failure_reason": task.failure_reason
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "tasks": list }))
    }

    async fn agent_task_get(&self, audit: AgentAuditContext) -> Result<Value, ServerError> {
        let task_id = audit
            .agent_task_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        let worker_id = {
            let tasks = self.state.agent_tasks.lock().await;
            tasks
                .get(&task_id)
                .ok_or(ServerError::NotFound)?
                .agent_worker_id
                .clone()
        };
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let tasks = self.state.agent_tasks.lock().await;
        let task = tasks.get(&task_id).ok_or(ServerError::NotFound)?;
        Ok(json!({
            "agent_task_id": task_id,
            "agent_worker_id": task.agent_worker_id,
            "status": task.status,
            "prompt": redact_text(&task.prompt, 64),
            "prompt_preview": redact_text(&task.prompt, 64),
            "terminal_session_id": task.terminal_session_id,
            "browser_session_id": task.browser_session_id,
            "last_output_sequence": self.last_terminal_sequence(&task.terminal_session_id).await,
            "failure_reason": task.failure_reason
        }))
    }

    async fn agent_attach_terminal(
        &self,
        command_id: String,
        audit: AgentAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let terminal_session_id = params
            .get("terminal_session_id")
            .and_then(Value::as_str)
            .ok_or(ServerError::InvalidRequest)?
            .to_string();
        self.ensure_terminal_exists(&terminal_session_id).await?;
        self.ensure_terminal_unowned(&terminal_session_id, Some(&worker_id))
            .await?;
        {
            let mut workers = self.state.agent_workers.lock().await;
            let worker = workers.get_mut(&worker_id).ok_or(ServerError::NotFound)?;
            if worker.closed || worker.current_task_id.is_some() {
                return Err(ServerError::Conflict);
            }
            worker.terminal_session_id = terminal_session_id.clone();
        }
        let result = json!({
            "agent_worker_id": worker_id,
            "terminal_session_id": terminal_session_id,
            "attached": true
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentTerminalAttached,
            result["agent_worker_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_worker_id": result["agent_worker_id"],
                "terminal_session_id": result["terminal_session_id"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn agent_detach_terminal(
        &self,
        command_id: String,
        audit: AgentAuditContext,
    ) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let terminal_session_id = {
            let mut workers = self.state.agent_workers.lock().await;
            let worker = workers.get_mut(&worker_id).ok_or(ServerError::NotFound)?;
            if worker.current_task_id.is_some() {
                return Err(ServerError::Conflict);
            }
            std::mem::take(&mut worker.terminal_session_id)
        };
        if terminal_session_id.is_empty() {
            return Err(ServerError::Conflict);
        }
        let result = json!({
            "agent_worker_id": worker_id,
            "terminal_session_id": terminal_session_id,
            "attached": false
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentTerminalDetached,
            result["agent_worker_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_worker_id": result["agent_worker_id"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn agent_attach_browser(
        &self,
        command_id: String,
        audit: AgentAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        let browser_session_id = params
            .get("browser_session_id")
            .and_then(Value::as_str)
            .ok_or(ServerError::InvalidRequest)?
            .to_string();
        self.ensure_browser_exists(&browser_session_id).await?;
        self.ensure_browser_unowned(&browser_session_id, Some(&worker_id))
            .await?;
        {
            let mut workers = self.state.agent_workers.lock().await;
            let worker = workers.get_mut(&worker_id).ok_or(ServerError::NotFound)?;
            if worker.closed {
                return Err(ServerError::Conflict);
            }
            worker.browser_session_id = Some(browser_session_id.clone());
        }
        let result = json!({
            "agent_worker_id": worker_id,
            "browser_session_id": browser_session_id,
            "attached": true
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentBrowserAttached,
            result["agent_worker_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_worker_id": result["agent_worker_id"],
                "browser_session_id": result["browser_session_id"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn agent_detach_browser(
        &self,
        command_id: String,
        audit: AgentAuditContext,
    ) -> Result<Value, ServerError> {
        let worker_id = audit
            .agent_worker_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_agent_worker_matches_audit(&worker_id, &audit)
            .await?;
        {
            let mut workers = self.state.agent_workers.lock().await;
            let worker = workers.get_mut(&worker_id).ok_or(ServerError::NotFound)?;
            worker.browser_session_id = None;
        }
        let result = json!({
            "agent_worker_id": worker_id,
            "attached": false
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::AgentBrowserDetached,
            result["agent_worker_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "agent_worker_id": result["agent_worker_id"],
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn terminal_spawn(
        &self,
        command_id: String,
        audit: TerminalAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let cols = params
            .get("cols")
            .and_then(Value::as_u64)
            .map(|v| v as u16)
            .unwrap_or(120);
        let rows = params
            .get("rows")
            .and_then(Value::as_u64)
            .map(|v| v as u16)
            .unwrap_or(30);
        let launch = parse_terminal_launch_spec(params)?;
        self.enforce_terminal_spawn_limits(&audit, &launch).await?;
        let terminal_session_id = format!(
            "ts-{}",
            random_token()?.chars().take(12).collect::<String>()
        );
        let spawn = spawn_terminal_process(&self.config, &launch, cols, rows).await?;
        let pid = match &spawn {
            SpawnedTerminalProcess::Process(process) => process.pid,
            #[cfg(windows)]
            SpawnedTerminalProcess::Conpty(process) => process.pid,
        };
        let program = launch.program.clone();
        let cwd = launch.cwd.clone();
        let runtime_name = match &spawn {
            SpawnedTerminalProcess::Process(_) => "process-stdio",
            #[cfg(windows)]
            SpawnedTerminalProcess::Conpty(_) => "conpty",
        };
        let (kill_tx, kill_rx) = oneshot::channel();
        {
            let mut runtime = self.state.terminal_runtime.lock().await;
            runtime.insert(
                terminal_session_id.clone(),
                TerminalSessionRuntime {
                    workspace_id: audit.workspace_id.clone(),
                    surface_id: audit.surface_id.clone(),
                    cols,
                    rows,
                    alive: true,
                    last_output: String::new(),
                    program: program.clone(),
                    cwd: cwd.clone(),
                    pid,
                    runtime: runtime_name.to_string(),
                    status: "running".to_string(),
                    exit_code: None,
                    input: match &spawn {
                        SpawnedTerminalProcess::Process(process) => process.input.clone(),
                        #[cfg(windows)]
                        SpawnedTerminalProcess::Conpty(process) => process.input.clone(),
                    },
                    kill_tx: Some(kill_tx),
                    next_sequence: 1,
                    history: VecDeque::with_capacity(self.config.terminal_max_history_events),
                    history_bytes: 0,
                    #[cfg(windows)]
                    conpty: match &spawn {
                        SpawnedTerminalProcess::Process(_) => None,
                        SpawnedTerminalProcess::Conpty(process) => Some(process.control.clone()),
                    },
                },
            );
        }
        match spawn {
            SpawnedTerminalProcess::Process(process) => {
                self.spawn_terminal_background_tasks_process(
                    terminal_session_id.clone(),
                    process.child,
                    process.stdout,
                    process.stderr,
                    kill_rx,
                );
            }
            #[cfg(windows)]
            SpawnedTerminalProcess::Conpty(process) => {
                self.spawn_terminal_background_tasks_conpty(
                    terminal_session_id.clone(),
                    process.output,
                    process.control,
                    kill_rx,
                );
            }
        }
        self.publish_terminal_event(
            &terminal_session_id,
            json!({
                "type": "terminal.spawned",
                "terminal_session_id": terminal_session_id,
                "workspace_id": audit.workspace_id,
                "surface_id": audit.surface_id,
                "pid": pid,
                "program": program,
                "cwd": cwd,
                "status": "running",
                "runtime": runtime_name
            }),
        )
        .await;
        let result = json!({
            "terminal_session_id": terminal_session_id,
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "shell": launch.shell,
            "program": program,
            "cols": cols,
            "rows": rows,
            "cwd": cwd,
            "pid": pid,
            "status": "running",
            "runtime": runtime_name
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            result["terminal_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "automation_key": format!("terminal:{}:spawn", result["terminal_session_id"].as_str().unwrap_or_default()),
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn terminal_input(
        &self,
        command_id: String,
        audit: TerminalAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let terminal_session_id = audit
            .terminal_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_terminal_matches_audit(&terminal_session_id, &audit)
            .await?;
        let input = params
            .get("input")
            .and_then(Value::as_str)
            .ok_or(ServerError::InvalidRequest)?;
        if input.len() > self.config.terminal_max_input_bytes {
            return Err(ServerError::RateLimited);
        }
        let input_handle = {
            let mut runtime = self.state.terminal_runtime.lock().await;
            let session = runtime
                .get_mut(&terminal_session_id)
                .ok_or(ServerError::NotFound)?;
            if !session.alive {
                return Err(ServerError::Conflict);
            }
            session.input.clone().ok_or(ServerError::Conflict)?
        };
        let bytes = write_to_terminal_input(input_handle, input).await?;
        self.publish_terminal_event(
            &terminal_session_id,
            json!({
                "type":"terminal.input.accepted",
                "terminal_session_id": terminal_session_id,
                "bytes": bytes
            }),
        )
        .await;
        let result = json!({
            "terminal_session_id": terminal_session_id,
            "accepted": true,
            "bytes": bytes
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            result["terminal_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "automation_key": format!("terminal:{}:input", result["terminal_session_id"].as_str().unwrap_or_default()),
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn terminal_resize(
        &self,
        command_id: String,
        audit: TerminalAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let terminal_session_id = audit
            .terminal_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_terminal_matches_audit(&terminal_session_id, &audit)
            .await?;
        let cols = params
            .get("cols")
            .and_then(Value::as_u64)
            .ok_or(ServerError::InvalidRequest)? as u16;
        let rows = params
            .get("rows")
            .and_then(Value::as_u64)
            .ok_or(ServerError::InvalidRequest)? as u16;
        let conpty = {
            let mut runtime = self.state.terminal_runtime.lock().await;
            let session = runtime
                .get_mut(&terminal_session_id)
                .ok_or(ServerError::NotFound)?;
            if !session.alive {
                return Err(ServerError::Conflict);
            }
            session.cols = cols;
            session.rows = rows;
            #[cfg(windows)]
            {
                session.conpty.clone()
            }
            #[cfg(not(windows))]
            {
                None
            }
        };
        let applied = resize_terminal_runtime(conpty, cols, rows)?;
        self.publish_terminal_event(
            &terminal_session_id,
            json!({
                "type":"terminal.resized",
                "terminal_session_id": terminal_session_id,
                "cols": cols,
                "rows": rows,
                "applied": applied
            }),
        )
        .await;
        let result = json!({
            "terminal_session_id": terminal_session_id,
            "cols": cols,
            "rows": rows,
            "applied": applied
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            result["terminal_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "automation_key": format!("terminal:{}:resize", result["terminal_session_id"].as_str().unwrap_or_default()),
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn terminal_kill(
        &self,
        command_id: String,
        audit: TerminalAuditContext,
    ) -> Result<Value, ServerError> {
        let terminal_session_id = audit
            .terminal_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_terminal_matches_audit(&terminal_session_id, &audit)
            .await?;
        let (pid, kill_tx, already_stopped) = {
            let mut runtime = self.state.terminal_runtime.lock().await;
            let session = runtime
                .get_mut(&terminal_session_id)
                .ok_or(ServerError::NotFound)?;
            let pid = session.pid;
            let already_stopped = !session.alive;
            session.alive = false;
            session.status = "killed".to_string();
            session.input = None;
            session.exit_code.get_or_insert(-1);
            (pid, session.kill_tx.take(), already_stopped)
        };
        if let Some(kill_tx) = kill_tx {
            let _ = kill_tx.send(());
        } else if !already_stopped {
            return Err(ServerError::Conflict);
        }
        self.publish_terminal_event(
            &terminal_session_id,
            json!({
                "type":"terminal.killed",
                "terminal_session_id": terminal_session_id,
                "pid": pid
            }),
        )
        .await;
        let result = json!({
            "terminal_session_id": terminal_session_id,
            "killed": true,
            "pid": pid
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            result["terminal_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            json!({
                "automation_key": format!("terminal:{}:kill", result["terminal_session_id"].as_str().unwrap_or_default()),
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn terminal_subscribe(
        &self,
        command_id: String,
        audit: TerminalAuditContext,
    ) -> Result<Value, ServerError> {
        let terminal_session_id = audit
            .terminal_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_terminal_matches_audit(&terminal_session_id, &audit)
            .await?;
        let subscriber_id = self.register_subscriber(
            &self.state.terminal_subscriptions,
            &terminal_session_id,
            self.config.browser_subscription_limit,
        )?;
        let (events, dropped_events) = self.drain_subscription(
            &self.state.terminal_subscriptions,
            &terminal_session_id,
            &subscriber_id,
        )?;

        let result = json!({
            "subscribed": true,
            "terminal_session_id": terminal_session_id,
            "subscriber_id": subscriber_id,
            "events": events,
            "dropped_events": dropped_events,
            "last_sequence": self.last_terminal_sequence(&terminal_session_id).await
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            terminal_session_id.clone(),
            command_id,
            json!({
                "automation_key": format!("terminal:{terminal_session_id}:subscribe"),
                "result": result
            }),
        );
        self.persist_and_apply(event).await
    }

    async fn terminal_history(
        &self,
        audit: TerminalAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let terminal_session_id = audit
            .terminal_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_terminal_matches_audit(&terminal_session_id, &audit)
            .await?;
        let from_sequence = params
            .get("from_sequence")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let max_events = params
            .get("max_events")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(self.config.terminal_max_history_events)
            .max(1);
        let runtime = self.state.terminal_runtime.lock().await;
        let session = runtime
            .get(&terminal_session_id)
            .ok_or(ServerError::NotFound)?;
        let events = session
            .history
            .iter()
            .filter(|event| event["sequence"].as_u64().unwrap_or_default() >= from_sequence)
            .take(max_events)
            .cloned()
            .collect::<Vec<_>>();
        let has_more = session
            .history
            .iter()
            .filter(|event| event["sequence"].as_u64().unwrap_or_default() >= from_sequence)
            .count()
            > events.len();
        Ok(json!({
            "terminal_session_id": terminal_session_id,
            "runtime": session.runtime,
            "status": session.status,
            "pid": session.pid,
            "cols": session.cols,
            "rows": session.rows,
            "last_sequence": session.next_sequence.saturating_sub(1),
            "events": events,
            "has_more": has_more,
            "exit_code": session.exit_code
        }))
    }

    async fn browser_create(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
    ) -> Result<Value, ServerError> {
        let browser_session_id = format!(
            "bs-{}",
            random_token()?.chars().take(12).collect::<String>()
        );
        let launched = launch_browser_process(&self.config, &browser_session_id);
        let (runtime_name, executable, port, process, last_error) = match launched {
            Ok(process) => (
                process.runtime.clone(),
                process.executable.clone(),
                Some(process.port),
                Some(process),
                None,
            ),
            Err(_) => (
                "browser-simulated".to_string(),
                "synthetic".to_string(),
                None,
                None,
                Some("browser launch unavailable in current environment".to_string()),
            ),
        };
        let result = json!({
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "browser_session_id": browser_session_id,
            "runtime": runtime_name,
            "executable": executable,
            "port": port
        });
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            runtime.insert(
                browser_session_id.clone(),
                BrowserSessionRuntime {
                    workspace_id: audit.workspace_id.clone(),
                    surface_id: audit.surface_id.clone(),
                    attached: true,
                    closed: false,
                    status: "ready".to_string(),
                    tabs: HashMap::new(),
                    tracing_enabled: false,
                    network_interception: false,
                    runtime: runtime_name.clone(),
                    executable: executable.clone(),
                    last_error,
                    process,
                    next_sequence: 1,
                    history: VecDeque::new(),
                    history_bytes: 0,
                },
            );
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.session.created",
                "browser_session_id": browser_session_id,
                "workspace_id": audit.workspace_id,
                "surface_id": audit.surface_id,
                "runtime": runtime_name
            }),
        )
        .await;
        let payload = json!({
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "browser_session_id": browser_session_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserSessionCreated,
            payload["browser_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_attach(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            session.attached = true;
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({"type":"browser.session.attached","browser_session_id": browser_session_id}),
        )
        .await;
        let result = json!({"browser_session_id": browser_session_id, "attached": true});
        let payload = json!({
            "browser_session_id": browser_session_id,
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserSessionAttached,
            payload["browser_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_detach(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            session.attached = false;
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({"type":"browser.session.detached","browser_session_id": browser_session_id}),
        )
        .await;
        let result = json!({"browser_session_id": browser_session_id, "attached": false});
        let payload = json!({
            "browser_session_id": browser_session_id,
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserSessionDetached,
            payload["browser_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_close(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let process = {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            session.closed = true;
            session.attached = false;
            for tab in session.tabs.values_mut() {
                tab.closed = true;
                tab.focused = false;
                tab.load_state = "closed".to_string();
            }
            session.process.clone()
        };
        if let Some(process) = process {
            let _ = cdp_browser_command(&process, "Browser.close", json!({}));
            if let Ok(mut child) = process.child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
            let _ = fs::remove_dir_all(&process.user_data_dir);
            let _ = fs::remove_dir_all(&process.artifact_dir);
        }
        self.cleanup_browser_subscribers(&browser_session_id);
        self.publish_browser_event(
            &browser_session_id,
            json!({"type":"browser.session.closed","browser_session_id": browser_session_id}),
        )
        .await;
        let result = json!({"browser_session_id": browser_session_id, "closed": true});
        let payload = json!({
            "browser_session_id": browser_session_id,
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserSessionClosed,
            payload["browser_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_tab_open(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let browser_tab_id = format!(
            "tab-{}",
            random_token()?.chars().take(10).collect::<String>()
        );
        let url = params
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("about:blank")
            .to_string();
        let target_info;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            if session.tabs.values().filter(|tab| !tab.closed).count()
                >= self.config.browser_max_tabs_per_session
            {
                return Err(ServerError::RateLimited);
            }
            target_info = if let Some(process) = &session.process {
                let created =
                    cdp_browser_command(process, "Target.createTarget", json!({ "url": url }))?;
                let target_id = created
                    .get("targetId")
                    .and_then(Value::as_str)
                    .ok_or(ServerError::Internal)?
                    .to_string();
                browser_target_list(process)?
                    .into_iter()
                    .find(|target| target.target_id == target_id)
                    .ok_or(ServerError::Internal)?
            } else {
                BrowserTargetInfo {
                    target_id: format!("synthetic-{browser_tab_id}"),
                    title: "synthetic".to_string(),
                    url: url.clone(),
                    websocket_url: String::new(),
                }
            };
            for tab in session.tabs.values_mut() {
                tab.focused = false;
            }
            session.tabs.insert(
                browser_tab_id.clone(),
                BrowserTabRuntime {
                    browser_tab_id: browser_tab_id.clone(),
                    target_id: target_info.target_id.clone(),
                    websocket_url: target_info.websocket_url.clone(),
                    url: target_info.url.clone(),
                    title: target_info.title.clone(),
                    load_state: "complete".to_string(),
                    focused: true,
                    closed: false,
                    history: vec![target_info.url.clone()],
                    history_index: 0,
                    cookies: HashMap::new(),
                    storage: HashMap::new(),
                    last_artifact_path: None,
                },
            );
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.tab.opened",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "url": target_info.url,
                "title": target_info.title
            }),
        )
        .await;
        let runtime_name = {
            let runtime = self.state.browser_runtime.lock().await;
            runtime
                .get(&browser_session_id)
                .map(|session| session.runtime.clone())
                .unwrap_or_else(|| "browser-simulated".to_string())
        };
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "url": target_info.url,
            "title": target_info.title,
            "load_state": "complete",
            "runtime": runtime_name
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "url": target_info.url,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserTabOpened,
            payload["browser_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_tab_list(&self, audit: BrowserAuditContext) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let mut runtime = self.state.browser_runtime.lock().await;
        let session = runtime
            .get_mut(&browser_session_id)
            .ok_or(ServerError::NotFound)?;
        if let Some(process) = &session.process {
            let targets = browser_target_list(process)?;
            for tab in session.tabs.values_mut() {
                if let Some(target) = targets
                    .iter()
                    .find(|target| target.target_id == tab.target_id)
                {
                    tab.url = target.url.clone();
                    tab.title = target.title.clone();
                    if !target.websocket_url.is_empty() {
                        tab.websocket_url = target.websocket_url.clone();
                    }
                }
            }
        }
        let runtime_name = session.runtime.clone();
        let tabs: Vec<Value> = session
            .tabs
            .values()
            .map(|tab| {
                json!({
                    "browser_tab_id": tab.browser_tab_id,
                    "url": tab.url,
                    "title": tab.title,
                    "load_state": tab.load_state,
                    "focused": tab.focused,
                    "closed": tab.closed,
                    "runtime": runtime_name
                })
            })
            .collect();
        Ok(json!({ "tabs": tabs }))
    }

    async fn browser_tab_focus(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            let target_id = session
                .tabs
                .get(&browser_tab_id)
                .ok_or(ServerError::NotFound)?
                .target_id
                .clone();
            if let Some(process) = &session.process {
                cdp_browser_command(
                    process,
                    "Target.activateTarget",
                    json!({ "targetId": target_id }),
                )?;
            }
            let mut found = false;
            for (tab_id, tab) in &mut session.tabs {
                tab.focused = tab_id == &browser_tab_id && !tab.closed;
                if tab_id == &browser_tab_id && !tab.closed {
                    found = true;
                }
            }
            if !found {
                return Err(ServerError::NotFound);
            }
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.tab.focused",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id
            }),
        )
        .await;
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "focused": true
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserTabFocused,
            payload["browser_tab_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_tab_close(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            let tab = session
                .tabs
                .get_mut(&browser_tab_id)
                .ok_or(ServerError::NotFound)?;
            if let Some(process) = &session.process {
                cdp_browser_command(
                    process,
                    "Target.closeTarget",
                    json!({ "targetId": tab.target_id }),
                )?;
            }
            tab.closed = true;
            tab.focused = false;
            tab.load_state = "closed".to_string();
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.tab.closed",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id
            }),
        )
        .await;
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "closed": true
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserTabClosed,
            payload["browser_tab_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_navigation(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
        method: &str,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        let requested_url = params
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("about:blank")
            .to_string();
        let (url, title, load_state) = {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            let tab = session
                .tabs
                .get_mut(&browser_tab_id)
                .ok_or(ServerError::NotFound)?;
            if tab.closed || session.closed {
                return Err(ServerError::Conflict);
            }
            if let Some(process) = &session.process {
                match method {
                    "browser.goto" => {
                        cdp_page_command(
                            &tab.websocket_url,
                            "Page.navigate",
                            json!({ "url": requested_url }),
                        )?;
                    }
                    "browser.reload" => {
                        cdp_page_command(&tab.websocket_url, "Page.reload", json!({}))?;
                    }
                    "browser.back" => {
                        let _ = cdp_page_command(&tab.websocket_url, "Page.goBack", json!({}));
                    }
                    "browser.forward" => {
                        let _ = cdp_page_command(&tab.websocket_url, "Page.goForward", json!({}));
                    }
                    _ => {}
                }
                tab.load_state =
                    wait_for_page_state(&tab.websocket_url, self.config.browser_nav_timeout_ms)?;
                refresh_browser_tab_runtime(tab, process)?;
                if method == "browser.goto" {
                    tab.history.truncate(tab.history_index.saturating_add(1));
                    tab.history.push(tab.url.clone());
                    tab.history_index = tab.history.len().saturating_sub(1);
                } else if method == "browser.back" {
                    tab.history_index = tab.history_index.saturating_sub(1);
                } else if method == "browser.forward" && tab.history_index + 1 < tab.history.len() {
                    tab.history_index += 1;
                }
            } else {
                match method {
                    "browser.goto" => {
                        tab.history.truncate(tab.history_index.saturating_add(1));
                        tab.history.push(requested_url.clone());
                        tab.history_index = tab.history.len().saturating_sub(1);
                        tab.url = requested_url;
                    }
                    "browser.back" => {
                        tab.history_index = tab.history_index.saturating_sub(1);
                        if let Some(next) = tab.history.get(tab.history_index) {
                            tab.url = next.clone();
                        }
                    }
                    "browser.forward" => {
                        if tab.history_index + 1 < tab.history.len() {
                            tab.history_index += 1;
                            if let Some(next) = tab.history.get(tab.history_index) {
                                tab.url = next.clone();
                            }
                        }
                    }
                    _ => {}
                }
                tab.title = "synthetic".to_string();
                tab.load_state = "complete".to_string();
            }
            (tab.url.clone(), tab.title.clone(), tab.load_state.clone())
        };
        let runtime_name = {
            let runtime = self.state.browser_runtime.lock().await;
            runtime
                .get(&browser_session_id)
                .map(|session| session.runtime.clone())
                .unwrap_or_else(|| "browser-simulated".to_string())
        };
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.navigation",
                "method": method,
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "url": url,
                "title": title,
                "load_state": load_state
            }),
        )
        .await;
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "method": method,
            "url": url,
            "title": title,
            "load_state": load_state,
            "runtime": runtime_name
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "method": method,
            "url": url,
            "automation_key": format!("{browser_tab_id}:last-nav"),
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationCompleted,
            payload["browser_tab_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_automation(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
        method: &str,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        let mut extra = json!({});
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            let tab = session
                .tabs
                .get_mut(&browser_tab_id)
                .ok_or(ServerError::NotFound)?;
            if tab.closed || session.closed {
                return Err(ServerError::Conflict);
            }
            let selector = params.get("selector").and_then(Value::as_str);
            let expression = params
                .get("expression")
                .and_then(Value::as_str)
                .unwrap_or("document.title");
            let text = params
                .get("text")
                .or_else(|| params.get("input"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let key = params.get("key").and_then(Value::as_str).unwrap_or("Enter");
            if let Some(process) = &session.process {
                refresh_browser_tab_runtime(tab, process)?;
                match method {
                    "browser.click" => {
                        if let Some(selector) = selector {
                            let script = format!(
                                "(()=>{{const el=document.querySelector({selector:?}); if(!el) throw new Error('selector not found'); el.click(); return {{clicked:true}};}})()"
                            );
                            extra = cdp_page_evaluate(&tab.websocket_url, &script)?;
                        }
                    }
                    "browser.type" => {
                        if let Some(selector) = selector {
                            let script = format!(
                                "(()=>{{const el=document.querySelector({selector:?}); if(!el) throw new Error('selector not found'); el.focus(); el.value={text:?}; el.dispatchEvent(new Event('input',{{bubbles:true}})); el.dispatchEvent(new Event('change',{{bubbles:true}})); return {{value: el.value}};}})()"
                            );
                            extra = cdp_page_evaluate(&tab.websocket_url, &script)?;
                        }
                    }
                    "browser.key" => {
                        let script = format!(
                            "(()=>{{const el=document.activeElement || document.body; ['keydown','keyup'].forEach(type => el.dispatchEvent(new KeyboardEvent(type,{{key:{key:?},bubbles:true}}))); return {{key:{key:?}}};}})()"
                        );
                        extra = cdp_page_evaluate(&tab.websocket_url, &script)?;
                    }
                    "browser.wait" => {
                        let wait_timeout = params
                            .get("timeout_ms")
                            .and_then(Value::as_u64)
                            .unwrap_or(self.config.browser_action_timeout_ms);
                        let started = Instant::now();
                        loop {
                            let condition = if let Some(selector) = selector {
                                cdp_page_evaluate(
                                    &tab.websocket_url,
                                    &format!("Boolean(document.querySelector({selector:?}))"),
                                )?
                            } else {
                                cdp_page_evaluate(&tab.websocket_url, expression)?
                            };
                            let ready = condition
                                .get("result")
                                .and_then(|value| value.get("value"))
                                .map(|value| match value {
                                    Value::Bool(flag) => *flag,
                                    Value::Null => false,
                                    Value::String(text) => !text.is_empty(),
                                    Value::Number(number) => {
                                        number.as_i64().unwrap_or_default() != 0
                                    }
                                    Value::Object(_) | Value::Array(_) => true,
                                })
                                .unwrap_or(false);
                            if ready {
                                extra = json!({"ready": true});
                                break;
                            }
                            if started.elapsed() > Duration::from_millis(wait_timeout.max(1)) {
                                return Err(ServerError::Timeout);
                            }
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                    "browser.screenshot" => {
                        let result = cdp_page_command(
                            &tab.websocket_url,
                            "Page.captureScreenshot",
                            json!({"format": "png"}),
                        )?;
                        let data = result
                            .get("data")
                            .and_then(Value::as_str)
                            .ok_or(ServerError::Internal)?;
                        let bytes = base64_decode(data)?;
                        let path = write_browser_artifact(
                            process,
                            &browser_tab_id,
                            "screenshot",
                            "png",
                            &bytes,
                        )?;
                        self.enforce_artifact_retention(Some(process.artifact_dir.as_path()))?;
                        tab.last_artifact_path = Some(path.clone());
                        extra = json!({
                            "artifact_path": path,
                            "artifact_bytes": bytes.len(),
                            "mime_type": "image/png"
                        });
                    }
                    "browser.evaluate" => {
                        extra = cdp_page_evaluate(&tab.websocket_url, expression)?;
                    }
                    "browser.cookie.set" => {
                        let name = params
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("session");
                        let value = params.get("value").and_then(Value::as_str).unwrap_or("set");
                        let cookie_url = if tab.url.starts_with("http") {
                            tab.url.clone()
                        } else {
                            "https://example.com".to_string()
                        };
                        let _ = cdp_page_command(
                            &tab.websocket_url,
                            "Network.setCookie",
                            json!({ "name": name, "value": value, "url": cookie_url }),
                        )?;
                        tab.cookies.insert(name.to_string(), value.to_string());
                    }
                    "browser.cookie.get" => {
                        let cookies = cdp_page_command(
                            &tab.websocket_url,
                            "Network.getCookies",
                            json!({ "urls": [tab.url.clone()] }),
                        )
                        .unwrap_or_else(|_| json!({"cookies": []}));
                        extra = json!({
                            "cookies": cookies.get("cookies").cloned().unwrap_or_else(|| json!([]))
                        });
                    }
                    "browser.storage.set" => {
                        let key_name = params.get("key").and_then(Value::as_str).unwrap_or("state");
                        let key_value =
                            params.get("value").and_then(Value::as_str).unwrap_or("set");
                        let script = format!(
                            "(()=>{{ localStorage.setItem({key_name:?}, {key_value:?}); return Object.fromEntries(Object.keys(localStorage).map(k => [k, localStorage.getItem(k)])); }})()"
                        );
                        extra = cdp_page_evaluate(&tab.websocket_url, &script)?;
                        tab.storage
                            .insert(key_name.to_string(), key_value.to_string());
                    }
                    "browser.storage.get" => {
                        extra = cdp_page_evaluate(
                            &tab.websocket_url,
                            "Object.fromEntries(Object.keys(localStorage).map(k => [k, localStorage.getItem(k)]))",
                        )?;
                    }
                    "browser.trace.start" => {
                        session.tracing_enabled = true;
                        extra = json!({"trace": "started"});
                    }
                    "browser.trace.stop" => {
                        session.tracing_enabled = false;
                        let bytes = serde_json::to_vec(&json!({
                            "browser_session_id": browser_session_id,
                            "browser_tab_id": browser_tab_id,
                            "stopped_at_ms": now_unix_ms()
                        }))
                        .map_err(|_| ServerError::Internal)?;
                        let path = write_browser_artifact(
                            process,
                            &browser_tab_id,
                            "trace",
                            "json",
                            &bytes,
                        )?;
                        self.enforce_artifact_retention(Some(process.artifact_dir.as_path()))?;
                        tab.last_artifact_path = Some(path.clone());
                        extra = json!({"artifact_path": path, "artifact_bytes": bytes.len()});
                    }
                    "browser.network.intercept" => {
                        session.network_interception = params
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(true);
                        extra = json!({"enabled": session.network_interception});
                    }
                    "browser.upload" => {
                        if let (Some(selector), Some(path)) =
                            (selector, params.get("path").and_then(Value::as_str))
                        {
                            let root =
                                cdp_page_command(&tab.websocket_url, "DOM.getDocument", json!({}))?;
                            let root_node = root
                                .get("root")
                                .and_then(|value| value.get("nodeId"))
                                .and_then(Value::as_i64)
                                .ok_or(ServerError::Internal)?;
                            let queried = cdp_page_command(
                                &tab.websocket_url,
                                "DOM.querySelector",
                                json!({"nodeId": root_node, "selector": selector}),
                            )?;
                            let node_id = queried
                                .get("nodeId")
                                .and_then(Value::as_i64)
                                .ok_or(ServerError::Internal)?;
                            let _ = cdp_page_command(
                                &tab.websocket_url,
                                "DOM.setFileInputFiles",
                                json!({"nodeId": node_id, "files": [path]}),
                            )?;
                            extra = json!({"uploaded": true, "path": path});
                        }
                    }
                    "browser.download" => {
                        let path = if let Some(download_url) =
                            params.get("url").and_then(Value::as_str)
                        {
                            let _ = cdp_browser_command(
                                process,
                                "Browser.setDownloadBehavior",
                                json!({
                                    "behavior": "allow",
                                    "downloadPath": process.artifact_dir.to_string_lossy().to_string()
                                }),
                            );
                            let _ = cdp_page_evaluate(
                                &tab.websocket_url,
                                &format!("window.location.href = {download_url:?}; true"),
                            )?;
                            let bytes =
                                format!("download placeholder for {download_url}").into_bytes();
                            write_browser_artifact(
                                process,
                                &browser_tab_id,
                                "download",
                                "bin",
                                &bytes,
                            )?
                        } else {
                            let size_bytes = params
                                .get("size_bytes")
                                .and_then(Value::as_u64)
                                .unwrap_or(1024)
                                as usize;
                            let bytes =
                                vec![b'x'; size_bytes.min(self.config.browser_download_max_bytes)];
                            write_browser_artifact(
                                process,
                                &browser_tab_id,
                                "download",
                                "bin",
                                &bytes,
                            )?
                        };
                        self.enforce_artifact_retention(Some(process.artifact_dir.as_path()))?;
                        tab.last_artifact_path = Some(path.clone());
                        extra = json!({"artifact_path": path});
                    }
                    _ => {}
                }
            } else {
                match method {
                    "browser.cookie.set" => {
                        tab.cookies.insert("session".to_string(), "set".to_string());
                    }
                    "browser.cookie.get" => {
                        extra = json!({ "cookies": tab.cookies });
                    }
                    "browser.storage.set" => {
                        tab.storage.insert("state".to_string(), "set".to_string());
                    }
                    "browser.storage.get" => {
                        extra = json!({ "storage": tab.storage });
                    }
                    "browser.trace.start" => {
                        session.tracing_enabled = true;
                    }
                    "browser.trace.stop" => {
                        session.tracing_enabled = false;
                    }
                    "browser.network.intercept" => {
                        session.network_interception = true;
                    }
                    _ => {}
                }
            }
        }
        let runtime_name = {
            let runtime = self.state.browser_runtime.lock().await;
            runtime
                .get(&browser_session_id)
                .map(|session| session.runtime.clone())
                .unwrap_or_else(|| "browser-simulated".to_string())
        };
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.automation",
                "method": method,
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "details": extra
            }),
        )
        .await;
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "method": method,
            "ok": true,
            "runtime": runtime_name,
            "details": extra
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "method": method,
            "automation_key": format!("{browser_tab_id}:last-op"),
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            payload["browser_tab_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_subscribe(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
        _method: &str,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let subscriber_id = self.register_subscriber(
            &self.state.browser_subscriptions,
            &browser_session_id,
            self.config.browser_subscription_limit,
        )?;
        let (events, dropped_events) = self.drain_subscription(
            &self.state.browser_subscriptions,
            &browser_session_id,
            &subscriber_id,
        )?;
        let result = json!({
            "subscribed": true,
            "browser_session_id": browser_session_id,
            "subscriber_id": subscriber_id,
            "events": events,
            "dropped_events": dropped_events,
            "last_sequence": self.last_browser_sequence(&browser_session_id).await
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            payload["browser_session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    async fn browser_history(
        &self,
        audit: BrowserAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let from_sequence = params
            .get("from_sequence")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let max_events = params
            .get("max_events")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(self.config.queue_limit.max(1))
            .max(1);
        let runtime = self.state.browser_runtime.lock().await;
        let session = runtime
            .get(&browser_session_id)
            .ok_or(ServerError::NotFound)?;
        let events = session
            .history
            .iter()
            .filter(|event| event["sequence"].as_u64().unwrap_or_default() >= from_sequence)
            .take(max_events)
            .cloned()
            .collect::<Vec<_>>();
        let has_more = session
            .history
            .iter()
            .filter(|event| event["sequence"].as_u64().unwrap_or_default() >= from_sequence)
            .count()
            > events.len();
        Ok(json!({
            "browser_session_id": browser_session_id,
            "runtime": session.runtime,
            "status": session.status,
            "last_sequence": session.next_sequence.saturating_sub(1),
            "events": events,
            "has_more": has_more,
            "attached": session.attached,
            "closed": session.closed
        }))
    }

    async fn browser_raw(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .clone()
            .ok_or(ServerError::InvalidRequest)?;
        self.ensure_browser_matches_audit(&browser_session_id, &audit)
            .await?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        let command = params
            .get("raw_command")
            .and_then(Value::as_str)
            .unwrap_or("noop")
            .to_string();
        let raw_params = params
            .get("raw_params")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let response = {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            let tab = session
                .tabs
                .get_mut(&browser_tab_id)
                .ok_or(ServerError::NotFound)?;
            if tab.closed {
                return Err(ServerError::Conflict);
            }
            if let Some(process) = &session.process {
                refresh_browser_tab_runtime(tab, process)?;
                cdp_page_command(&tab.websocket_url, &command, raw_params)?
            } else {
                json!({"synthetic": true})
            }
        };
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.raw",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "raw_command": command,
                "result": response
            }),
        )
        .await;
        let runtime_name = {
            let runtime = self.state.browser_runtime.lock().await;
            runtime
                .get(&browser_session_id)
                .map(|session| session.runtime.clone())
                .unwrap_or_else(|| "browser-simulated".to_string())
        };
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "raw_command": command,
            "ok": true,
            "runtime": runtime_name,
            "result": response
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "automation_key": format!("{browser_tab_id}:raw"),
            "result": result
        });
        let event = EventRecord::new(
            random_event_id(),
            EventType::BrowserNavigationRequested,
            payload["browser_tab_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            command_id,
            payload,
        );
        self.persist_and_apply(event).await
    }

    fn register_subscriber(
        &self,
        subs: &StdMutex<HashMap<String, HashMap<String, SubscriptionState>>>,
        stream_key: &str,
        limit: usize,
    ) -> Result<String, ServerError> {
        let mut map = subs.lock().expect("subscription lock poisoned");
        let entry = map.entry(stream_key.to_string()).or_default();
        if entry.len() >= limit {
            return Err(ServerError::RateLimited);
        }
        let subscriber_id = format!(
            "sub-{}",
            random_token()
                .map_err(|_| ServerError::Internal)?
                .chars()
                .take(10)
                .collect::<String>()
        );
        entry.insert(
            subscriber_id.clone(),
            SubscriptionState::new(self.config.queue_limit.max(1)),
        );
        Ok(subscriber_id)
    }

    fn drain_subscription(
        &self,
        subs: &StdMutex<HashMap<String, HashMap<String, SubscriptionState>>>,
        stream_key: &str,
        subscriber_id: &str,
    ) -> Result<(Vec<Value>, u64), ServerError> {
        let mut map = subs.lock().expect("subscription lock poisoned");
        let stream = map.get_mut(stream_key).ok_or(ServerError::NotFound)?;
        let state = stream.get_mut(subscriber_id).ok_or(ServerError::NotFound)?;
        let events = state.drain();
        let dropped = state.dropped_events;
        Ok((events, dropped))
    }

    async fn publish_terminal_event(&self, terminal_session_id: &str, event: Value) {
        publish_terminal_event_for_state(
            &self.state,
            self.config.queue_limit.max(1),
            self.config.terminal_max_history_events.max(1),
            self.config.terminal_max_history_bytes.max(1),
            terminal_session_id,
            event,
        )
        .await;
    }

    async fn publish_browser_event(&self, browser_session_id: &str, event: Value) {
        publish_browser_event_for_state(
            &self.state,
            self.config.queue_limit.max(1),
            self.config.queue_limit.max(1),
            self.config.queue_limit.saturating_mul(1024),
            browser_session_id,
            event,
        )
        .await;
    }

    fn cleanup_browser_subscribers(&self, browser_session_id: &str) {
        let mut all = self
            .state
            .browser_subscriptions
            .lock()
            .expect("subscription lock poisoned");
        all.remove(browser_session_id);
    }

    fn spawn_terminal_background_tasks_process(
        &self,
        terminal_session_id: String,
        mut child: tokio::process::Child,
        stdout: Option<tokio::process::ChildStdout>,
        stderr: Option<tokio::process::ChildStderr>,
        mut kill_rx: oneshot::Receiver<()>,
    ) {
        if let Some(stdout) = stdout {
            spawn_terminal_output_task(
                self.state.clone(),
                self.config.queue_limit.max(1),
                terminal_session_id.clone(),
                stdout,
                "stdout",
            );
        }
        if let Some(stderr) = stderr {
            spawn_terminal_output_task(
                self.state.clone(),
                self.config.queue_limit.max(1),
                terminal_session_id.clone(),
                stderr,
                "stderr",
            );
        }
        let state = self.state.clone();
        let queue_limit = self.config.queue_limit.max(1);
        tokio::spawn(async move {
            let status = tokio::select! {
                wait = child.wait() => wait,
                _ = &mut kill_rx => {
                    let _ = child.kill().await;
                    child.wait().await
                }
            };
            update_terminal_exit_state(&state, &terminal_session_id, status).await;
            publish_terminal_event_for_state(
                &state,
                queue_limit,
                queue_limit.saturating_mul(64),
                queue_limit.saturating_mul(1024),
                &terminal_session_id,
                build_terminal_exit_event(&state, &terminal_session_id).await,
            )
            .await;
        });
    }

    #[cfg(windows)]
    fn spawn_terminal_background_tasks_conpty(
        &self,
        terminal_session_id: String,
        output: StdFile,
        control: Arc<ConptyControl>,
        mut kill_rx: oneshot::Receiver<()>,
    ) {
        let state = self.state.clone();
        let queue_limit = self.config.queue_limit.max(1);
        let history_events = self.config.terminal_max_history_events.max(1);
        let history_bytes = self.config.terminal_max_history_bytes.max(1);
        let runtime = tokio::runtime::Handle::current();
        let terminal_for_reader = terminal_session_id.clone();
        tokio::task::spawn_blocking(move || {
            let mut reader = StdBufReader::new(output);
            let mut buffer = vec![0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(read) => {
                        let chunk = buffer[..read].to_vec();
                        let state = state.clone();
                        let terminal = terminal_for_reader.clone();
                        runtime.block_on(async move {
                            publish_terminal_output_chunk(
                                &state,
                                queue_limit,
                                history_events,
                                history_bytes,
                                &terminal,
                                "stdout",
                                &chunk,
                            )
                            .await;
                        });
                    }
                    Err(_) => break,
                }
            }
        });
        let state = self.state.clone();
        tokio::spawn(async move {
            let status = tokio::select! {
                exit = tokio::task::spawn_blocking({
                    let control = control.clone();
                    move || wait_for_conpty_process_exit(&control)
                }) => exit.map_err(|_| std::io::Error::from(std::io::ErrorKind::Other)).and_then(|v| v),
                _ = &mut kill_rx => {
                    let _ = terminate_conpty_process(&control);
                    tokio::task::spawn_blocking({
                        let control = control.clone();
                        move || wait_for_conpty_process_exit(&control)
                    }).await.map_err(|_| std::io::Error::from(std::io::ErrorKind::Other)).and_then(|v| v)
                }
            };
            update_terminal_exit_state(&state, &terminal_session_id, status).await;
            publish_terminal_event_for_state(
                &state,
                queue_limit,
                history_events,
                history_bytes,
                &terminal_session_id,
                build_terminal_exit_event(&state, &terminal_session_id).await,
            )
            .await;
        });
    }

    async fn last_terminal_sequence(&self, terminal_session_id: &str) -> u64 {
        self.state
            .terminal_runtime
            .lock()
            .await
            .get(terminal_session_id)
            .map(|session| session.next_sequence.saturating_sub(1))
            .unwrap_or(0)
    }

    async fn last_browser_sequence(&self, browser_session_id: &str) -> u64 {
        self.state
            .browser_runtime
            .lock()
            .await
            .get(browser_session_id)
            .map(|session| session.next_sequence.saturating_sub(1))
            .unwrap_or(0)
    }

    async fn enforce_terminal_spawn_limits(
        &self,
        audit: &TerminalAuditContext,
        launch: &TerminalLaunchSpec,
    ) -> Result<(), ServerError> {
        if launch
            .env
            .iter()
            .map(|(key, value)| key.len() + value.len())
            .sum::<usize>()
            > self.config.terminal_max_env_bytes
        {
            return Err(ServerError::RateLimited);
        }
        if !self.config.env_allowlist.is_empty()
            && launch.env.keys().any(|key| {
                !self
                    .config
                    .env_allowlist
                    .iter()
                    .any(|allowed| allowed == key)
            })
        {
            self.state
                .metrics
                .lock()
                .expect("metrics lock poisoned")
                .incr_counter("policy.terminal.env.rejected", 1);
            return Err(ServerError::RateLimited);
        }
        if !self.config.terminal_allowed_programs.is_empty()
            && !self
                .config
                .terminal_allowed_programs
                .iter()
                .any(|program| program.eq_ignore_ascii_case(&launch.program))
        {
            self.state
                .metrics
                .lock()
                .expect("metrics lock poisoned")
                .incr_counter("policy.terminal.program.rejected", 1);
            return Err(ServerError::RateLimited);
        }
        if !self.config.terminal_allowed_cwd_roots.is_empty()
            && !self
                .config
                .terminal_allowed_cwd_roots
                .iter()
                .any(|root| launch.cwd.starts_with(root))
        {
            self.state
                .metrics
                .lock()
                .expect("metrics lock poisoned")
                .incr_counter("policy.terminal.cwd.rejected", 1);
            return Err(ServerError::RateLimited);
        }
        let runtime = self.state.terminal_runtime.lock().await;
        if runtime.len() >= self.config.terminal_max_sessions {
            return Err(ServerError::RateLimited);
        }
        let workspace_sessions = runtime
            .values()
            .filter(|session| session.workspace_id == audit.workspace_id && session.alive)
            .count();
        if workspace_sessions >= self.config.terminal_max_sessions_per_workspace {
            return Err(ServerError::RateLimited);
        }
        Ok(())
    }

    async fn ensure_terminal_exists(&self, terminal_session_id: &str) -> Result<(), ServerError> {
        let runtime = self.state.terminal_runtime.lock().await;
        if runtime.contains_key(terminal_session_id) {
            Ok(())
        } else {
            Err(ServerError::NotFound)
        }
    }

    async fn ensure_terminal_matches_audit(
        &self,
        terminal_session_id: &str,
        audit: &TerminalAuditContext,
    ) -> Result<(), ServerError> {
        let runtime = self.state.terminal_runtime.lock().await;
        let session = runtime
            .get(terminal_session_id)
            .ok_or(ServerError::NotFound)?;
        if session.workspace_id == audit.workspace_id && session.surface_id == audit.surface_id {
            Ok(())
        } else {
            Err(ServerError::Unauthorized)
        }
    }

    async fn ensure_browser_exists(&self, browser_session_id: &str) -> Result<(), ServerError> {
        let runtime = self.state.browser_runtime.lock().await;
        if runtime.contains_key(browser_session_id) {
            Ok(())
        } else {
            Err(ServerError::NotFound)
        }
    }

    async fn ensure_browser_matches_audit(
        &self,
        browser_session_id: &str,
        audit: &BrowserAuditContext,
    ) -> Result<(), ServerError> {
        let runtime = self.state.browser_runtime.lock().await;
        let session = runtime
            .get(browser_session_id)
            .ok_or(ServerError::NotFound)?;
        if session.workspace_id == audit.workspace_id && session.surface_id == audit.surface_id {
            Ok(())
        } else {
            Err(ServerError::Unauthorized)
        }
    }

    async fn ensure_agent_worker_matches_audit(
        &self,
        worker_id: &str,
        audit: &AgentAuditContext,
    ) -> Result<(), ServerError> {
        let workers = self.state.agent_workers.lock().await;
        let worker = workers.get(worker_id).ok_or(ServerError::NotFound)?;
        if worker.workspace_id == audit.workspace_id && worker.surface_id == audit.surface_id {
            Ok(())
        } else {
            Err(ServerError::Unauthorized)
        }
    }

    async fn ensure_terminal_unowned(
        &self,
        terminal_session_id: &str,
        except_worker_id: Option<&str>,
    ) -> Result<(), ServerError> {
        let workers = self.state.agent_workers.lock().await;
        if workers.iter().any(|(worker_id, worker)| {
            !worker.closed
                && worker.terminal_session_id == terminal_session_id
                && except_worker_id
                    .map(|id| id != worker_id.as_str())
                    .unwrap_or(true)
        }) {
            return Err(ServerError::Conflict);
        }
        Ok(())
    }

    async fn ensure_browser_unowned(
        &self,
        browser_session_id: &str,
        except_worker_id: Option<&str>,
    ) -> Result<(), ServerError> {
        let workers = self.state.agent_workers.lock().await;
        if workers.iter().any(|(worker_id, worker)| {
            !worker.closed
                && worker.browser_session_id.as_deref() == Some(browser_session_id)
                && except_worker_id
                    .map(|id| id != worker_id.as_str())
                    .unwrap_or(true)
        }) {
            return Err(ServerError::Conflict);
        }
        Ok(())
    }

    async fn try_interrupt_terminal_session(
        &self,
        terminal_session_id: &str,
    ) -> Result<(), ServerError> {
        let input = {
            let runtime = self.state.terminal_runtime.lock().await;
            let session = runtime
                .get(terminal_session_id)
                .ok_or(ServerError::NotFound)?;
            session.input.clone().ok_or(ServerError::Conflict)?
        };
        let _ = write_to_terminal_input(input, "\u{3}").await?;
        Ok(())
    }

    async fn kill_terminal_session(&self, terminal_session_id: &str) -> Result<(), ServerError> {
        let (pid, kill_tx, already_stopped) = {
            let mut runtime = self.state.terminal_runtime.lock().await;
            let session = runtime
                .get_mut(terminal_session_id)
                .ok_or(ServerError::NotFound)?;
            let pid = session.pid;
            let already_stopped = !session.alive;
            session.alive = false;
            session.status = "closed".to_string();
            session.input = None;
            session.exit_code.get_or_insert(-1);
            (pid, session.kill_tx.take(), already_stopped)
        };
        if let Some(kill_tx) = kill_tx {
            let _ = kill_tx.send(());
        } else if !already_stopped {
            return Err(ServerError::Conflict);
        }
        self.publish_terminal_event(
            terminal_session_id,
            json!({
                "type":"terminal.killed",
                "terminal_session_id": terminal_session_id,
                "pid": pid
            }),
        )
        .await;
        Ok(())
    }

    fn artifact_root_path(&self) -> PathBuf {
        PathBuf::from(&self.config.event_dir).join("browser-artifacts")
    }

    fn collect_artifact_stats(&self) -> ArtifactStats {
        collect_artifact_stats(&self.artifact_root_path())
    }

    fn enforce_artifact_retention(&self, session_dir: Option<&Path>) -> Result<(), ServerError> {
        let before = self.collect_artifact_stats();
        enforce_artifact_retention(
            &self.artifact_root_path(),
            session_dir,
            self.config.artifact_ttl_ms,
            self.config.artifact_max_files,
            self.config.artifact_max_total_bytes,
            self.config.artifact_max_files_per_session,
        )
        .map_err(|_| ServerError::Internal)?;
        let after = self.collect_artifact_stats();
        let mut metrics = self.state.metrics.lock().expect("metrics lock poisoned");
        metrics.incr_counter("runtime.artifacts.cleanup_runs_total", 1);
        if before.files > after.files {
            metrics.incr_counter(
                "runtime.artifacts.evicted_files_total",
                before.files - after.files,
            );
        }
        if before.bytes > after.bytes {
            metrics.incr_counter(
                "runtime.artifacts.evicted_bytes_total",
                before.bytes - after.bytes,
            );
        }
        Ok(())
    }

    fn dependency_status(&self) -> DependencyStatus {
        DependencyStatus {
            terminal_runtime_ready: terminal_dependency_ready(&self.config),
            browser_runtime_ready: browser_dependency_ready(&self.config),
            artifact_root_ready: writable_directory_ready(&self.artifact_root_path()),
            event_store_ready: writable_directory_ready(Path::new(&self.config.event_dir)),
        }
    }

    async fn persist_and_apply(&self, event: EventRecord) -> Result<Value, ServerError> {
        let mut projection = self.state.projection.lock().await;
        if let Some(existing) = projection.command_results.get(&event.command_id) {
            return Ok(existing.clone());
        }
        self.apply_fault(FaultHook::StoreAppend).await?;
        let mut store = self.state.store.lock().await;
        let cursor = store.append(&event).map_err(map_store_error)?;
        projection.apply(&event, cursor).map_err(map_store_error)?;
        self.apply_fault(FaultHook::Snapshot).await?;
        store
            .maybe_snapshot_and_compact(&projection)
            .map_err(map_store_error)?;
        projection
            .command_results
            .get(&event.command_id)
            .cloned()
            .ok_or(ServerError::Internal)
    }

    async fn lookup_command_result(&self, command_id: &str) -> Option<Value> {
        let projection = self.state.projection.lock().await;
        projection.command_results.get(command_id).cloned()
    }

    async fn find_session(&self, token: &str) -> Option<SessionRecord> {
        let projection = self.state.projection.lock().await;
        projection
            .sessions
            .get(token)
            .map(SessionRecord::from_projection)
    }

    fn next_correlation_id(&self) -> String {
        let next = self.state.correlation.fetch_add(1, Ordering::Relaxed);
        format!("corr-{next}")
    }

    pub async fn session_count(&self) -> usize {
        let projection = self.state.projection.lock().await;
        projection.sessions.len()
    }

    pub fn telemetry_snapshot(&self) -> TelemetrySnapshot {
        self.state
            .telemetry
            .lock()
            .expect("telemetry lock poisoned")
            .snapshot()
    }

    pub fn metrics_snapshot(&self) -> MetricsSnapshot {
        self.state
            .metrics
            .lock()
            .expect("metrics lock poisoned")
            .snapshot()
    }

    #[allow(clippy::too_many_arguments)]
    fn log_event(
        &self,
        level: LogLevel,
        component: &str,
        event: &str,
        correlation_id: &str,
        connection_id: Option<String>,
        method: Option<String>,
        command_id: Option<String>,
        duration_ms: Option<u64>,
        status: &str,
        fields: BTreeMap<String, Value>,
    ) {
        let redacted_fields = fields
            .into_iter()
            .map(|(key, value)| {
                let lowered = key.to_ascii_lowercase();
                if lowered.contains("token")
                    || lowered.contains("env")
                    || lowered.contains("prompt")
                    || lowered.contains("raw")
                {
                    (key, redact_value(&value))
                } else {
                    (key, value)
                }
            })
            .collect();
        self.state
            .telemetry
            .lock()
            .expect("telemetry lock poisoned")
            .push_log(LogRecord {
                timestamp_ms: now_unix_ms(),
                level,
                component: component.to_string(),
                event: event.to_string(),
                correlation_id: correlation_id.to_string(),
                command_id,
                connection_id,
                workspace_id: None,
                surface_id: None,
                method,
                duration_ms,
                status: status.to_string(),
                fields: redacted_fields,
            });
    }

    fn record_span(
        &self,
        name: &str,
        correlation_id: &str,
        started_at_ms: u64,
        duration_ms: u64,
        attributes: BTreeMap<String, Value>,
    ) {
        self.state
            .telemetry
            .lock()
            .expect("telemetry lock poisoned")
            .push_span(SpanRecord {
                name: name.to_string(),
                correlation_id: correlation_id.to_string(),
                started_at_ms,
                duration_ms,
                attributes,
            });
    }

    fn record_request_metrics(&self, method: Option<&str>, duration: Duration, ok: bool) {
        let mut metrics = self.state.metrics.lock().expect("metrics lock poisoned");
        metrics.incr_counter("rpc.requests.total", 1);
        if ok {
            metrics.incr_counter("rpc.requests.ok", 1);
        } else {
            metrics.incr_counter("rpc.requests.error", 1);
        }
        metrics.record_latency("rpc.request.latency_ms", duration.as_secs_f64() * 1000.0);
        if let Some(method) = method {
            metrics.incr_counter(&format!("rpc.method.{method}.count"), 1);
            metrics.record_latency(
                &format!("rpc.method.{method}.latency_ms"),
                duration.as_secs_f64() * 1000.0,
            );
        }
        metrics.set_gauge("rpc.active_requests", self.active_request_count() as u64);
    }

    async fn require_active_session(
        &self,
        params: Option<&Value>,
    ) -> Result<SessionRecord, ServerError> {
        let token = extract_token(params).ok_or(ServerError::Unauthorized)?;
        let now = now_unix_ms();
        let session = self
            .find_session(&token)
            .await
            .ok_or(ServerError::Unauthorized)?;
        if !session.is_active(now) {
            return Err(ServerError::Unauthorized);
        }
        Ok(session)
    }

    async fn require_active_session_scope(
        &self,
        params: Option<&Value>,
        scope: SessionScope,
    ) -> Result<SessionRecord, ServerError> {
        let session = self.require_active_session(params).await?;
        if session.has_scope(scope) {
            Ok(session)
        } else {
            Err(ServerError::Unauthorized)
        }
    }

    pub fn begin_shutdown(&self) {
        self.state.shutting_down.store(true, Ordering::SeqCst);
        self.log_event(
            LogLevel::Warn,
            "lifecycle",
            "shutdown.begin",
            "lifecycle",
            None,
            None,
            None,
            None,
            "started",
            BTreeMap::new(),
        );
    }

    pub fn is_shutting_down(&self) -> bool {
        self.state.shutting_down.load(Ordering::SeqCst)
    }

    fn active_request_count(&self) -> usize {
        self.state
            .inflight_by_connection
            .lock()
            .expect("inflight lock poisoned")
            .values()
            .copied()
            .sum::<usize>()
    }

    fn breaker_is_open(&self) -> bool {
        let now = now_unix_ms();
        self.state
            .breaker
            .lock()
            .expect("breaker lock poisoned")
            .open_until_ms
            .map(|until| until > now)
            .unwrap_or(false)
    }

    pub async fn shutdown_and_drain(&self) {
        self.begin_shutdown();
        let deadline =
            Instant::now() + Duration::from_millis(self.config.shutdown_drain_timeout_ms);
        loop {
            let inflight = {
                let map = self
                    .state
                    .inflight_by_connection
                    .lock()
                    .expect("inflight lock poisoned");
                map.values().copied().sum::<usize>()
            };
            if inflight == 0 || Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let terminal_kills = {
            let mut runtime = self.state.terminal_runtime.lock().await;
            runtime
                .values_mut()
                .filter_map(|session| session.kill_tx.take())
                .collect::<Vec<_>>()
        };
        for kill_tx in terminal_kills {
            let _ = kill_tx.send(());
        }
        self.state.terminal_runtime.lock().await.clear();
        self.state.browser_runtime.lock().await.clear();
        self.state.agent_workers.lock().await.clear();
        self.state.agent_tasks.lock().await.clear();
        self.state
            .terminal_subscriptions
            .lock()
            .expect("subscription lock poisoned")
            .clear();
        self.state
            .browser_subscriptions
            .lock()
            .expect("subscription lock poisoned")
            .clear();
    }

    fn check_breaker(&self) -> Result<(), ServerError> {
        let now = now_unix_ms();
        let mut breaker = self.state.breaker.lock().expect("breaker lock poisoned");
        if let Some(open_until_ms) = breaker.open_until_ms {
            if open_until_ms > now {
                return Err(ServerError::RateLimited);
            }
            if breaker.half_open_probe_running {
                return Err(ServerError::RateLimited);
            }
            breaker.open_until_ms = None;
            breaker.half_open_probe_running = true;
        }
        Ok(())
    }

    fn record_success(&self) {
        let mut breaker = self.state.breaker.lock().expect("breaker lock poisoned");
        breaker.consecutive_failures = 0;
        breaker.open_until_ms = None;
        breaker.half_open_probe_running = false;
    }

    fn record_failure(&self, err: &ServerError) {
        if !matches!(err, ServerError::Internal | ServerError::Timeout) {
            return;
        }
        let now = now_unix_ms();
        let mut breaker = self.state.breaker.lock().expect("breaker lock poisoned");
        breaker.consecutive_failures = breaker.consecutive_failures.saturating_add(1);
        breaker.half_open_probe_running = false;
        if breaker.consecutive_failures >= self.config.breaker_failure_threshold {
            breaker.open_until_ms = Some(now + self.config.breaker_cooldown_ms);
            let mut fields = BTreeMap::new();
            fields.insert(
                "open_until_ms".to_string(),
                json!(now + self.config.breaker_cooldown_ms),
            );
            self.log_event(
                LogLevel::Warn,
                "reliability",
                "breaker.open",
                "breaker",
                None,
                None,
                None,
                None,
                "open",
                fields,
            );
        }
    }

    async fn apply_fault(&self, hook: FaultHook) -> Result<(), ServerError> {
        let action = {
            let faults = self.state.faults.lock().expect("fault lock poisoned");
            faults.get(&hook).cloned()
        };
        match action {
            Some(FaultAction::ReturnInternal) => Err(ServerError::Internal),
            Some(FaultAction::DelayMs(delay_ms)) => {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                Ok(())
            }
            Some(FaultAction::DropResponse) => Err(ServerError::Timeout),
            None => Ok(()),
        }
    }

    #[cfg(windows)]
    pub async fn serve_named_pipe(&self, pipe_name: &str) -> Result<(), std::io::Error> {
        self.serve_named_pipe_until_shutdown(pipe_name).await
    }

    #[cfg(windows)]
    pub async fn serve_named_pipe_until_shutdown(
        &self,
        pipe_name: &str,
    ) -> Result<(), std::io::Error> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::windows::named_pipe::ServerOptions;

        let mut listener = ServerOptions::new().create(pipe_name)?;
        while !self.is_shutting_down() {
            listener.connect().await?;
            let next_listener = ServerOptions::new().create(pipe_name)?;
            let mut stream = std::mem::replace(&mut listener, next_listener);
            let server = self.clone();
            tokio::spawn(async move {
                let (read_half, mut write_half) = tokio::io::split(&mut stream);
                let mut reader = BufReader::new(read_half);
                let mut line = String::new();
                let connection_id = "pipe-client";
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let response = server
                                .handle_json_line(connection_id, line.trim_end())
                                .await;
                            if write_half.write_all(response.as_bytes()).await.is_err() {
                                break;
                            }
                            if write_half.write_all(b"\n").await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }
        Ok(())
    }

    #[cfg(not(windows))]
    pub async fn serve_named_pipe(&self, _pipe_name: &str) -> Result<(), std::io::Error> {
        self.serve_named_pipe_until_shutdown(_pipe_name).await
    }

    #[cfg(not(windows))]
    pub async fn serve_named_pipe_until_shutdown(
        &self,
        _pipe_name: &str,
    ) -> Result<(), std::io::Error> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "named pipes are only supported on Windows",
        ))
    }
}

fn map_store_error(err: StoreError) -> ServerError {
    match err {
        StoreError::ChecksumMismatch { .. } => ServerError::Conflict,
        _ => ServerError::Internal,
    }
}

fn map_error_code(err: &ServerError) -> RpcErrorCode {
    match err {
        ServerError::InvalidRequest => RpcErrorCode::InvalidRequest,
        ServerError::Unauthorized => RpcErrorCode::Unauthorized,
        ServerError::NotFound => RpcErrorCode::NotFound,
        ServerError::Conflict => RpcErrorCode::Conflict,
        ServerError::Timeout => RpcErrorCode::Timeout,
        ServerError::RateLimited => RpcErrorCode::RateLimited,
        ServerError::Internal => RpcErrorCode::Internal,
    }
}

fn extract_id_from_raw_json(raw: &str) -> Value {
    let parsed: Result<Value, _> = serde_json::from_str(raw);
    if let Ok(value) = parsed {
        value.get("id").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

fn extract_method_from_raw_json(raw: &str) -> Option<String> {
    let parsed: Result<Value, _> = serde_json::from_str(raw);
    parsed.ok().and_then(|value| {
        value
            .get("method")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
}

fn extract_token(params: Option<&Value>) -> Option<String> {
    let params = params?;
    let object = params.as_object()?;
    if let Some(token) = object.get("token").and_then(Value::as_str) {
        return SessionToken::new(token.to_string())
            .ok()
            .map(|t| t.as_str().to_string());
    }
    object
        .get("auth")
        .and_then(Value::as_object)
        .and_then(|auth| auth.get("token"))
        .and_then(Value::as_str)
        .and_then(|token| {
            SessionToken::new(token.to_string())
                .ok()
                .map(|t| t.as_str().to_string())
        })
}

fn extract_optional_requested_scopes(
    params: Option<&Value>,
) -> Result<Option<Vec<String>>, ServerError> {
    let Some(object) = params.and_then(Value::as_object) else {
        return Ok(None);
    };
    let Some(scopes) = object.get("scopes") else {
        return Ok(None);
    };
    let values = scopes.as_array().ok_or(ServerError::InvalidRequest)?;
    let parsed = values
        .iter()
        .map(|value| match value.as_str() {
            Some("diagnostics") => Ok("diagnostics".to_string()),
            Some("runtime") => Ok("runtime".to_string()),
            Some("agent") => Ok("agent".to_string()),
            _ => Err(ServerError::InvalidRequest),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.is_empty() {
        return Err(ServerError::InvalidRequest);
    }
    Ok(Some(parsed))
}

fn extract_requested_scopes(
    params: Option<&Value>,
    config: &BackendConfig,
) -> Result<Vec<String>, ServerError> {
    let defaults = config
        .default_session_scopes
        .iter()
        .map(|scope| scope.as_str().to_string())
        .collect::<Vec<_>>();
    let Some(requested) = extract_optional_requested_scopes(params)? else {
        return Ok(defaults);
    };
    if requested
        .iter()
        .all(|scope| defaults.iter().any(|default| default == scope))
    {
        Ok(requested)
    } else {
        Err(ServerError::Unauthorized)
    }
}

fn is_path_allowed(candidate: &Path, allowed_roots: &[String]) -> bool {
    if allowed_roots.is_empty() {
        return true;
    }
    allowed_roots.iter().any(|root| {
        let root_path = Path::new(root);
        candidate.starts_with(root_path)
    })
}

fn redact_text(value: &str, limit: usize) -> String {
    let mut redacted = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        redacted.push_str("...");
    }
    redacted
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(text, 64)),
        Value::Array(values) => Value::Array(values.iter().map(redact_value).collect()),
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (key, value) in map {
                let lowered = key.to_ascii_lowercase();
                if lowered.contains("token")
                    || lowered.contains("secret")
                    || lowered.contains("password")
                {
                    redacted.insert(key.clone(), Value::String("[redacted]".to_string()));
                } else {
                    redacted.insert(key.clone(), redact_value(value));
                }
            }
            Value::Object(redacted)
        }
        other => other.clone(),
    }
}

fn extract_command_id(params: Option<&Value>) -> Result<String, ServerError> {
    let params = params.ok_or(ServerError::InvalidRequest)?;
    let object = params.as_object().ok_or(ServerError::InvalidRequest)?;
    let command_id = object
        .get("command_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?;
    let command_id =
        CommandId::new(command_id.to_string()).map_err(|_| ServerError::InvalidRequest)?;
    Ok(command_id.as_str().to_string())
}

fn parse_terminal_launch_spec(params: &Value) -> Result<TerminalLaunchSpec, ServerError> {
    let object = params.as_object().ok_or(ServerError::InvalidRequest)?;
    let shell = object
        .get("shell")
        .and_then(Value::as_str)
        .unwrap_or(default_terminal_shell())
        .to_string();
    let program = object
        .get("program")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| default_program_for_shell(&shell));
    let args = match object.get("args") {
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToString::to_string)
                    .ok_or(ServerError::InvalidRequest)
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err(ServerError::InvalidRequest),
        None => default_args_for_shell(&shell),
    };
    let cwd = object
        .get("cwd")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(default_terminal_cwd);
    let env = match object.get("env") {
        Some(Value::Object(map)) => map
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|v| (key.clone(), v.to_string()))
                    .ok_or(ServerError::InvalidRequest)
            })
            .collect::<Result<HashMap<_, _>, _>>()?,
        Some(_) => return Err(ServerError::InvalidRequest),
        None => HashMap::new(),
    };
    Ok(TerminalLaunchSpec {
        program,
        args,
        cwd,
        env,
        shell,
    })
}

enum SpawnedTerminalProcess {
    Process(Box<ProcessSpawnedTerminal>),
    #[cfg(windows)]
    Conpty(Box<ConptySpawnedTerminal>),
}

struct ProcessSpawnedTerminal {
    pid: u32,
    input: Option<TerminalInputHandle>,
    stdout: Option<tokio::process::ChildStdout>,
    stderr: Option<tokio::process::ChildStderr>,
    child: tokio::process::Child,
}

#[cfg(windows)]
struct ConptySpawnedTerminal {
    pid: u32,
    input: Option<TerminalInputHandle>,
    output: StdFile,
    control: Arc<ConptyControl>,
}

async fn spawn_terminal_process(
    config: &BackendConfig,
    launch: &TerminalLaunchSpec,
    _cols: u16,
    _rows: u16,
) -> Result<SpawnedTerminalProcess, ServerError> {
    #[cfg(not(windows))]
    let _ = config;

    #[cfg(windows)]
    if config.terminal_runtime == "conpty" {
        return spawn_terminal_process_conpty(launch, _cols, _rows);
    }
    let mut command = Command::new(&launch.program);
    command.args(&launch.args);
    command.current_dir(&launch.cwd);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    for (key, value) in &launch.env {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|_| ServerError::Internal)?;
    let pid = child.id().ok_or(ServerError::Internal)?;
    let input = child
        .stdin
        .take()
        .map(|stdin| TerminalInputHandle::Process(Arc::new(Mutex::new(stdin))));
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    Ok(SpawnedTerminalProcess::Process(Box::new(
        ProcessSpawnedTerminal {
            pid,
            input,
            stdout,
            stderr,
            child,
        },
    )))
}

fn default_terminal_shell() -> &'static str {
    if cfg!(windows) {
        "powershell"
    } else {
        "sh"
    }
}

fn default_program_for_shell(shell: &str) -> String {
    match shell {
        "cmd" if cfg!(windows) => system32_program("cmd.exe"),
        "powershell" if cfg!(windows) => {
            system32_program("WindowsPowerShell\\v1.0\\powershell.exe")
        }
        "pwsh" => "pwsh".to_string(),
        "bash" => "bash".to_string(),
        _ if cfg!(windows) => system32_program("WindowsPowerShell\\v1.0\\powershell.exe"),
        _ => "sh".to_string(),
    }
}

fn default_args_for_shell(shell: &str) -> Vec<String> {
    match shell {
        "cmd" if cfg!(windows) => vec!["/Q".to_string(), "/K".to_string()],
        "powershell" if cfg!(windows) => vec![
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-NoExit".to_string(),
        ],
        "pwsh" => vec![
            "-NoLogo".to_string(),
            "-NoProfile".to_string(),
            "-NoExit".to_string(),
        ],
        _ => Vec::new(),
    }
}

fn default_terminal_cwd() -> String {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string()
}

fn system32_program(relative: &str) -> String {
    std::env::var("SystemRoot")
        .map(|root| format!("{root}\\System32\\{relative}"))
        .unwrap_or_else(|_| relative.to_string())
}

async fn write_to_terminal_input(
    handle: TerminalInputHandle,
    input: &str,
) -> Result<usize, ServerError> {
    match handle {
        TerminalInputHandle::Process(stdin) => {
            let mut writer = stdin.lock().await;
            writer
                .write_all(input.as_bytes())
                .await
                .map_err(|_| ServerError::Internal)?;
            writer.flush().await.map_err(|_| ServerError::Internal)?;
            Ok(input.len())
        }
        #[cfg(windows)]
        TerminalInputHandle::BlockingPipe(file) => {
            let input = input.to_string();
            tokio::task::spawn_blocking(move || {
                let mut writer = file.lock().expect("pipe lock");
                writer
                    .write_all(input.as_bytes())
                    .map_err(|_| ServerError::Internal)?;
                writer.flush().map_err(|_| ServerError::Internal)?;
                Ok(input.len())
            })
            .await
            .map_err(|_| ServerError::Internal)?
        }
    }
}

async fn publish_terminal_output_chunk(
    state: &Arc<ServerState>,
    queue_limit: usize,
    history_limit_events: usize,
    history_limit_bytes: usize,
    terminal_session_id: &str,
    stream: &str,
    chunk: &[u8],
) {
    if chunk.is_empty() {
        return;
    }
    let output = String::from_utf8_lossy(chunk).into_owned();
    {
        let mut runtime = state.terminal_runtime.lock().await;
        if let Some(session) = runtime.get_mut(terminal_session_id) {
            session.last_output = output.clone();
        }
    }
    publish_terminal_event_for_state(
        state,
        queue_limit,
        history_limit_events,
        history_limit_bytes,
        terminal_session_id,
        json!({
            "type": "terminal.output",
            "terminal_session_id": terminal_session_id,
            "stream": stream,
            "output": output
        }),
    )
    .await;
}

async fn publish_terminal_event_for_state(
    state: &Arc<ServerState>,
    queue_limit: usize,
    history_limit_events: usize,
    history_limit_bytes: usize,
    terminal_session_id: &str,
    mut event: Value,
) {
    let (status, runtime_name, sequence, timestamp_ms) = {
        let mut runtime = state.terminal_runtime.lock().await;
        if let Some(session) = runtime.get_mut(terminal_session_id) {
            let sequence = session.next_sequence;
            session.next_sequence = session.next_sequence.saturating_add(1);
            let timestamp_ms = now_unix_ms();
            if let Some(object) = event.as_object_mut() {
                object
                    .entry("sequence".to_string())
                    .or_insert(json!(sequence));
                object
                    .entry("timestamp_ms".to_string())
                    .or_insert(json!(timestamp_ms));
                object
                    .entry("status".to_string())
                    .or_insert(json!(session.status.clone()));
                object
                    .entry("runtime".to_string())
                    .or_insert(json!(session.runtime.clone()));
            }
            let encoded_len = event.to_string().len();
            session.history.push_back(event.clone());
            session.history_bytes = session.history_bytes.saturating_add(encoded_len);
            while session.history.len() > history_limit_events
                || session.history_bytes > history_limit_bytes
            {
                if let Some(removed) = session.history.pop_front() {
                    session.history_bytes = session
                        .history_bytes
                        .saturating_sub(removed.to_string().len());
                } else {
                    break;
                }
            }
            (
                session.status.clone(),
                session.runtime.clone(),
                sequence,
                timestamp_ms,
            )
        } else {
            (
                "unknown".to_string(),
                "unknown".to_string(),
                0,
                now_unix_ms(),
            )
        }
    };
    if let Some(object) = event.as_object_mut() {
        object
            .entry("sequence".to_string())
            .or_insert(json!(sequence));
        object
            .entry("timestamp_ms".to_string())
            .or_insert(json!(timestamp_ms));
        object.entry("status".to_string()).or_insert(json!(status));
        object
            .entry("runtime".to_string())
            .or_insert(json!(runtime_name));
    }
    let mut all = state
        .terminal_subscriptions
        .lock()
        .expect("subscription lock poisoned");
    if let Some(subs) = all.get_mut(terminal_session_id) {
        for sub in subs.values_mut() {
            sub.push(event.clone(), queue_limit);
        }
    }
}

async fn publish_browser_event_for_state(
    state: &Arc<ServerState>,
    queue_limit: usize,
    history_limit_events: usize,
    history_limit_bytes: usize,
    browser_session_id: &str,
    mut event: Value,
) {
    let (status, runtime_name, workspace_id, surface_id, sequence, timestamp_ms) = {
        let mut runtime = state.browser_runtime.lock().await;
        if let Some(session) = runtime.get_mut(browser_session_id) {
            let sequence = session.next_sequence;
            session.next_sequence = session.next_sequence.saturating_add(1);
            let timestamp_ms = now_unix_ms();
            if let Some(object) = event.as_object_mut() {
                object
                    .entry("sequence".to_string())
                    .or_insert(json!(sequence));
                object
                    .entry("timestamp_ms".to_string())
                    .or_insert(json!(timestamp_ms));
                object
                    .entry("status".to_string())
                    .or_insert(json!(session.status.clone()));
                object
                    .entry("runtime".to_string())
                    .or_insert(json!(session.runtime.clone()));
                object
                    .entry("workspace_id".to_string())
                    .or_insert(json!(session.workspace_id.clone()));
                object
                    .entry("surface_id".to_string())
                    .or_insert(json!(session.surface_id.clone()));
            }
            let encoded_len = event.to_string().len();
            session.history.push_back(event.clone());
            session.history_bytes = session.history_bytes.saturating_add(encoded_len);
            while session.history.len() > history_limit_events
                || session.history_bytes > history_limit_bytes
            {
                if let Some(removed) = session.history.pop_front() {
                    session.history_bytes = session
                        .history_bytes
                        .saturating_sub(removed.to_string().len());
                } else {
                    break;
                }
            }
            (
                session.status.clone(),
                session.runtime.clone(),
                session.workspace_id.clone(),
                session.surface_id.clone(),
                sequence,
                timestamp_ms,
            )
        } else {
            (
                "unknown".to_string(),
                "unknown".to_string(),
                String::new(),
                String::new(),
                0,
                now_unix_ms(),
            )
        }
    };
    if let Some(object) = event.as_object_mut() {
        object
            .entry("sequence".to_string())
            .or_insert(json!(sequence));
        object
            .entry("timestamp_ms".to_string())
            .or_insert(json!(timestamp_ms));
        object.entry("status".to_string()).or_insert(json!(status));
        object
            .entry("runtime".to_string())
            .or_insert(json!(runtime_name));
        object
            .entry("workspace_id".to_string())
            .or_insert(json!(workspace_id));
        object
            .entry("surface_id".to_string())
            .or_insert(json!(surface_id));
    }
    let mut all = state
        .browser_subscriptions
        .lock()
        .expect("subscription lock poisoned");
    if let Some(subs) = all.get_mut(browser_session_id) {
        for sub in subs.values_mut() {
            sub.push(event.clone(), queue_limit);
        }
    }
}

fn spawn_terminal_output_task<R>(
    state: Arc<ServerState>,
    queue_limit: usize,
    terminal_session_id: String,
    reader: R,
    stream: &'static str,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = reader;
        let mut buffer = vec![0_u8; 4096];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break,
                Ok(read) => {
                    publish_terminal_output_chunk(
                        &state,
                        queue_limit,
                        queue_limit.saturating_mul(64),
                        queue_limit.saturating_mul(1024),
                        &terminal_session_id,
                        stream,
                        &buffer[..read],
                    )
                    .await;
                }
                Err(_) => {
                    publish_terminal_event_for_state(
                        &state,
                        queue_limit,
                        queue_limit.saturating_mul(64),
                        queue_limit.saturating_mul(1024),
                        &terminal_session_id,
                        json!({
                            "type": "terminal.runtime-error",
                            "terminal_session_id": terminal_session_id,
                            "stream": stream
                        }),
                    )
                    .await;
                    break;
                }
            }
        }
    });
}

async fn update_terminal_exit_state(
    state: &Arc<ServerState>,
    terminal_session_id: &str,
    status: Result<std::process::ExitStatus, std::io::Error>,
) {
    let mut runtime = state.terminal_runtime.lock().await;
    if let Some(session) = runtime.get_mut(terminal_session_id) {
        session.alive = false;
        session.input = None;
        session.kill_tx = None;
        match status {
            Ok(exit) => {
                session.exit_code = exit.code();
                session.status = if exit.success() {
                    "exited".to_string()
                } else {
                    "failed".to_string()
                };
            }
            Err(_) => {
                session.exit_code = Some(-1);
                session.status = "failed".to_string();
            }
        }
    }
}

async fn build_terminal_exit_event(state: &Arc<ServerState>, terminal_session_id: &str) -> Value {
    let runtime = state.terminal_runtime.lock().await;
    if let Some(session) = runtime.get(terminal_session_id) {
        json!({
            "type": "terminal.exited",
            "terminal_session_id": terminal_session_id,
            "pid": session.pid,
            "status": session.status,
            "exit_code": session.exit_code,
            "runtime": session.runtime
        })
    } else {
        json!({
            "type": "terminal.exited",
            "terminal_session_id": terminal_session_id
        })
    }
}

fn selected_terminal_runtime_name(config: &BackendConfig) -> &'static str {
    if cfg!(windows) && config.terminal_runtime == "conpty" {
        "conpty"
    } else {
        "process-stdio"
    }
}

#[cfg(windows)]
fn conpty_creation_flags(has_env_block: bool) -> u32 {
    let mut flags = EXTENDED_STARTUPINFO_PRESENT;
    if has_env_block {
        flags |= windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT;
    }
    flags
}

fn terminal_dependency_ready(config: &BackendConfig) -> bool {
    match selected_terminal_runtime_name(config) {
        "conpty" => cfg!(windows),
        name => matches!(name, "process-stdio"),
    }
}

fn writable_directory_ready(path: &Path) -> bool {
    if path.exists() && !path.is_dir() {
        return false;
    }
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(format!(".maxc-ready-{}", now_unix_ms()));
    match fs::write(&probe, b"ok") {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn resize_terminal_runtime(
    conpty: Option<Arc<ConptyControl>>,
    cols: u16,
    rows: u16,
) -> Result<bool, ServerError> {
    if let Some(control) = conpty {
        unsafe {
            let hr = ResizePseudoConsole(
                control.hpc,
                COORD {
                    X: cols as i16,
                    Y: rows as i16,
                },
            );
            if hr != 0 {
                return Err(ServerError::Internal);
            }
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(not(windows))]
fn resize_terminal_runtime(
    _conpty: Option<()>,
    _cols: u16,
    _rows: u16,
) -> Result<bool, ServerError> {
    Ok(false)
}

#[cfg(windows)]
fn spawn_terminal_process_conpty(
    launch: &TerminalLaunchSpec,
    cols: u16,
    rows: u16,
) -> Result<SpawnedTerminalProcess, ServerError> {
    unsafe {
        let sa = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: std::ptr::null_mut(),
            bInheritHandle: 1,
        };
        let mut input_read: HANDLE = std::ptr::null_mut();
        let mut input_write: HANDLE = std::ptr::null_mut();
        let mut output_read: HANDLE = std::ptr::null_mut();
        let mut output_write: HANDLE = std::ptr::null_mut();
        if CreatePipe(&mut input_read, &mut input_write, &sa, 0) == 0 {
            return Err(ServerError::Internal);
        }
        if CreatePipe(&mut output_read, &mut output_write, &sa, 0) == 0 {
            let _ = CloseHandle(input_read);
            let _ = CloseHandle(input_write);
            return Err(ServerError::Internal);
        }
        let _ = SetHandleInformation(input_write, HANDLE_FLAG_INHERIT, 0);
        let _ = SetHandleInformation(output_read, HANDLE_FLAG_INHERIT, 0);

        let mut hpc: HPCON = 0;
        let hr = CreatePseudoConsole(
            COORD {
                X: cols as i16,
                Y: rows as i16,
            },
            input_read,
            output_write,
            0,
            &mut hpc,
        );
        let _ = CloseHandle(input_read);
        let _ = CloseHandle(output_write);
        if hr != 0 {
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            return Err(ServerError::Internal);
        }

        let attribute_count = 1;
        let mut attr_list_size = 0;
        let _ = InitializeProcThreadAttributeList(
            std::ptr::null_mut(),
            attribute_count,
            0,
            &mut attr_list_size,
        );
        let mut attr_list = vec![0u8; attr_list_size];
        let lp_attribute_list = attr_list.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        if InitializeProcThreadAttributeList(
            lp_attribute_list,
            attribute_count,
            0,
            &mut attr_list_size,
        ) == 0
        {
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            return Err(ServerError::Internal);
        }

        const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;
        if UpdateProcThreadAttribute(
            lp_attribute_list,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
            hpc as *mut _,
            std::mem::size_of::<HPCON>(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
        {
            DeleteProcThreadAttributeList(lp_attribute_list);
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            return Err(ServerError::Internal);
        }

        let mut startup: STARTUPINFOEXW = std::mem::zeroed();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.lpAttributeList = lp_attribute_list;
        let mut process_info: PROCESS_INFORMATION = std::mem::zeroed();
        let application_name = to_wide_null(&launch.program);
        let mut command_line = build_windows_command_line(&launch.program, &launch.args);
        let mut cwd = to_wide_null(&launch.cwd);
        let mut env_block = build_windows_environment_block(&launch.env);
        let env_ptr = if env_block.is_empty() {
            std::ptr::null_mut()
        } else {
            env_block.as_mut_ptr() as *mut core::ffi::c_void
        };
        let ok = CreateProcessW(
            application_name.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            conpty_creation_flags(!env_block.is_empty()),
            env_ptr,
            cwd.as_mut_ptr(),
            &startup.StartupInfo,
            &mut process_info,
        );
        DeleteProcThreadAttributeList(lp_attribute_list);
        let _ = CloseHandle(process_info.hThread);
        if ok == 0 {
            ClosePseudoConsole(hpc);
            let _ = CloseHandle(input_write);
            let _ = CloseHandle(output_read);
            if !process_info.hProcess.is_null() {
                let _ = CloseHandle(process_info.hProcess);
            }
            return Err(ServerError::Internal);
        }

        let input_file = StdFile::from_raw_handle(input_write as _);
        let output_file = StdFile::from_raw_handle(output_read as _);
        Ok(SpawnedTerminalProcess::Conpty(Box::new(
            ConptySpawnedTerminal {
                pid: process_info.dwProcessId,
                input: Some(TerminalInputHandle::BlockingPipe(Arc::new(StdMutex::new(
                    input_file,
                )))),
                output: output_file,
                control: Arc::new(ConptyControl {
                    hpc,
                    process_handle: process_info.hProcess,
                }),
            },
        )))
    }
}

#[cfg(windows)]
fn wait_for_conpty_process_exit(
    control: &ConptyControl,
) -> Result<std::process::ExitStatus, std::io::Error> {
    unsafe {
        WaitForSingleObject(control.process_handle, u32::MAX);
        let mut code: u32 = 0;
        if GetExitCodeProcess(control.process_handle, &mut code) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        ClosePseudoConsole(control.hpc);
        let _ = CloseHandle(control.process_handle);
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        Ok(std::process::ExitStatus::from_raw(code))
    }
}

#[cfg(windows)]
fn terminate_conpty_process(control: &ConptyControl) -> Result<(), ServerError> {
    unsafe {
        if TerminateProcess(control.process_handle, 1) == 0 {
            return Err(ServerError::Internal);
        }
        Ok(())
    }
}

#[cfg(windows)]
fn build_windows_environment_block(env_overrides: &HashMap<String, String>) -> Vec<u16> {
    if env_overrides.is_empty() {
        return Vec::new();
    }
    let mut merged = std::env::vars().collect::<HashMap<_, _>>();
    for (key, value) in env_overrides {
        merged.insert(key.clone(), value.clone());
    }
    let mut entries = merged.into_iter().collect::<Vec<_>>();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut wide = Vec::new();
    for (key, value) in entries {
        let mut item = to_wide_null(&format!("{key}={value}"));
        wide.extend(item.drain(..item.len().saturating_sub(1)));
        wide.push(0);
    }
    wide.push(0);
    wide
}

#[cfg(windows)]
fn build_windows_command_line(program: &str, args: &[String]) -> Vec<u16> {
    let mut text = quote_windows_arg(program);
    for arg in args {
        text.push(' ');
        text.push_str(&quote_windows_arg(arg));
    }
    to_wide_null(&text)
}

#[cfg(windows)]
fn quote_windows_arg(value: &str) -> String {
    if value.is_empty() || value.chars().any(|c| c.is_whitespace() || c == '"') {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

#[cfg(windows)]
fn to_wide_null(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[derive(Debug)]
struct BrowserTargetInfo {
    target_id: String,
    title: String,
    url: String,
    websocket_url: String,
}

#[derive(Debug)]
struct WebSocketClient {
    stream: TcpStream,
}

impl WebSocketClient {
    fn connect(url: &str) -> Result<Self, ServerError> {
        let (host, port, path) = parse_url_parts(url, "ws")?;
        let mut stream =
            TcpStream::connect((host.as_str(), port)).map_err(|_| ServerError::Internal)?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|_| ServerError::Internal)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(15)))
            .map_err(|_| ServerError::Internal)?;

        let mut key_bytes = [0_u8; 16];
        rand::thread_rng().fill_bytes(&mut key_bytes);
        let key = base64_encode(&key_bytes);
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|_| ServerError::Internal)?;
        stream.flush().map_err(|_| ServerError::Internal)?;

        let mut response = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream
                .read(&mut buffer)
                .map_err(|_| ServerError::Internal)?;
            if read == 0 {
                return Err(ServerError::Internal);
            }
            response.extend_from_slice(&buffer[..read]);
            if response.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
            if response.len() > 8192 {
                return Err(ServerError::Internal);
            }
        }
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or(ServerError::Internal)?
            + 4;
        let header_text = String::from_utf8(response[..header_end].to_vec())
            .map_err(|_| ServerError::Internal)?;
        if !header_text.starts_with("HTTP/1.1 101") && !header_text.starts_with("HTTP/1.0 101") {
            return Err(ServerError::Internal);
        }
        Ok(Self { stream })
    }

    fn send_text(&mut self, text: &str) -> Result<(), ServerError> {
        let payload = text.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 14);
        frame.push(0x81);
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else if payload.len() <= 65_535 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
        let mut mask = [0_u8; 4];
        rand::thread_rng().fill_bytes(&mut mask);
        frame.extend_from_slice(&mask);
        for (idx, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[idx % 4]);
        }
        self.stream
            .write_all(&frame)
            .map_err(|_| ServerError::Internal)?;
        self.stream.flush().map_err(|_| ServerError::Internal)?;
        Ok(())
    }

    fn send_pong(&mut self, payload: &[u8]) -> Result<(), ServerError> {
        let mut frame = Vec::with_capacity(payload.len() + 10);
        frame.push(0x8A);
        if payload.len() < 126 {
            frame.push(payload.len() as u8);
        } else if payload.len() <= 65_535 {
            frame.push(126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }
        frame.extend_from_slice(payload);
        self.stream
            .write_all(&frame)
            .map_err(|_| ServerError::Internal)?;
        self.stream.flush().map_err(|_| ServerError::Internal)?;
        Ok(())
    }

    fn read_text(&mut self) -> Result<Option<String>, ServerError> {
        loop {
            let mut header = [0_u8; 2];
            self.stream
                .read_exact(&mut header)
                .map_err(|_| ServerError::Internal)?;
            let opcode = header[0] & 0x0F;
            let masked = (header[1] & 0x80) != 0;
            let mut payload_len = (header[1] & 0x7F) as usize;
            if payload_len == 126 {
                let mut ext = [0_u8; 2];
                self.stream
                    .read_exact(&mut ext)
                    .map_err(|_| ServerError::Internal)?;
                payload_len = u16::from_be_bytes(ext) as usize;
            } else if payload_len == 127 {
                let mut ext = [0_u8; 8];
                self.stream
                    .read_exact(&mut ext)
                    .map_err(|_| ServerError::Internal)?;
                payload_len = u64::from_be_bytes(ext) as usize;
            }
            let mut mask = [0_u8; 4];
            if masked {
                self.stream
                    .read_exact(&mut mask)
                    .map_err(|_| ServerError::Internal)?;
            }
            let mut payload = vec![0_u8; payload_len];
            if payload_len > 0 {
                self.stream
                    .read_exact(&mut payload)
                    .map_err(|_| ServerError::Internal)?;
            }
            if masked {
                for (idx, byte) in payload.iter_mut().enumerate() {
                    *byte ^= mask[idx % 4];
                }
            }
            match opcode {
                0x1 => {
                    return String::from_utf8(payload)
                        .map(Some)
                        .map_err(|_| ServerError::Internal);
                }
                0x8 => return Ok(None),
                0x9 => {
                    self.send_pong(&payload)?;
                }
                0xA => {}
                _ => {}
            }
        }
    }
}

#[derive(Debug)]
struct CdpConnection {
    websocket: WebSocketClient,
    next_id: u64,
}

impl CdpConnection {
    fn connect(url: &str) -> Result<Self, ServerError> {
        Ok(Self {
            websocket: WebSocketClient::connect(url)?,
            next_id: 1,
        })
    }

    fn command(&mut self, method: &str, params: Value) -> Result<Value, ServerError> {
        let id = self.next_id;
        self.next_id += 1;
        self.websocket
            .send_text(&json!({"id": id, "method": method, "params": params}).to_string())?;
        loop {
            let message = self.websocket.read_text()?.ok_or(ServerError::Internal)?;
            let value: Value = serde_json::from_str(&message).map_err(|_| ServerError::Internal)?;
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                if value.get("error").is_some() {
                    return Err(ServerError::Internal);
                }
                return Ok(value.get("result").cloned().unwrap_or_else(|| json!({})));
            }
        }
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let b0 = bytes[idx];
        let b1 = *bytes.get(idx + 1).unwrap_or(&0);
        let b2 = *bytes.get(idx + 2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if idx + 1 < bytes.len() {
            out.push(TABLE[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if idx + 2 < bytes.len() {
            out.push(TABLE[(b2 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        idx += 3;
    }
    out
}

fn base64_decode(text: &str) -> Result<Vec<u8>, ServerError> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return Err(ServerError::Internal);
    }
    let mut idx = 0;
    while idx < bytes.len() {
        let a = decode_base64_char(bytes[idx])?;
        let b = decode_base64_char(bytes[idx + 1])?;
        let c = if bytes[idx + 2] == b'=' {
            64
        } else {
            decode_base64_char(bytes[idx + 2])?
        };
        let d = if bytes[idx + 3] == b'=' {
            64
        } else {
            decode_base64_char(bytes[idx + 3])?
        };
        out.push((a << 2) | (b >> 4));
        if c != 64 {
            out.push(((b & 0x0F) << 4) | (c >> 2));
        }
        if d != 64 {
            out.push(((c & 0x03) << 6) | d);
        }
        idx += 4;
    }
    Ok(out)
}

fn decode_base64_char(byte: u8) -> Result<u8, ServerError> {
    match byte {
        b'A'..=b'Z' => Ok(byte - b'A'),
        b'a'..=b'z' => Ok(byte - b'a' + 26),
        b'0'..=b'9' => Ok(byte - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(ServerError::Internal),
    }
}

fn parse_url_parts(url: &str, expected_scheme: &str) -> Result<(String, u16, String), ServerError> {
    let prefix = format!("{expected_scheme}://");
    let remainder = url.strip_prefix(&prefix).ok_or(ServerError::Internal)?;
    let mut split = remainder.splitn(2, '/');
    let host_port = split.next().ok_or(ServerError::Internal)?;
    let path = format!("/{}", split.next().unwrap_or_default());
    let mut parts = host_port.splitn(2, ':');
    let host = parts.next().ok_or(ServerError::Internal)?.to_string();
    let port = parts
        .next()
        .ok_or(ServerError::Internal)?
        .parse::<u16>()
        .map_err(|_| ServerError::Internal)?;
    Ok((host, port, path))
}

fn browser_http_request(method: &str, url: &str) -> Result<String, ServerError> {
    let (host, port, path) = parse_url_parts(url, "http")?;
    let mut stream =
        TcpStream::connect((host.as_str(), port)).map_err(|_| ServerError::Internal)?;
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|_| ServerError::Internal)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|_| ServerError::Internal)?;
    let request =
        format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|_| ServerError::Internal)?;
    stream.flush().map_err(|_| ServerError::Internal)?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|_| ServerError::Internal)?;
    let split_at = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or(ServerError::Internal)?
        + 4;
    let header =
        String::from_utf8(response[..split_at].to_vec()).map_err(|_| ServerError::Internal)?;
    if !header.starts_with("HTTP/1.1 200") && !header.starts_with("HTTP/1.1 201") {
        return Err(ServerError::Internal);
    }
    String::from_utf8(response[split_at..].to_vec()).map_err(|_| ServerError::Internal)
}

#[derive(Debug, Clone)]
struct BrowserLaunchTarget {
    runtime: String,
    executable: String,
}

fn resolve_browser_executable(config: &BackendConfig) -> Result<String, ServerError> {
    let configured = config.browser_executable_or_channel.trim();
    if configured.eq_ignore_ascii_case("__synthetic__") {
        return Err(ServerError::Internal);
    }
    if configured.eq_ignore_ascii_case("webview2") {
        return resolve_webview2_executable();
    }
    if !configured.is_empty() && Path::new(configured).exists() {
        return Ok(configured.to_string());
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
        .find(|candidate| Path::new(candidate).exists())
        .map(|value| (*value).to_string())
        .ok_or(ServerError::Internal)
}

fn resolve_webview2_executable() -> Result<String, ServerError> {
    #[cfg(not(windows))]
    {
        Err(ServerError::Internal)
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
                .map_err(|_| ServerError::Internal)?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_dir())
                .collect::<Vec<_>>();
            versions.sort();
            versions.reverse();
            for version_dir in versions {
                let candidate = version_dir.join("msedgewebview2.exe");
                if candidate.exists() {
                    return Ok(candidate.to_string_lossy().to_string());
                }
            }
        }
        Err(ServerError::Internal)
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
                    runtime: "webview2".to_string(),
                    executable,
                }]
            })
            .unwrap_or_default();
    }
    let mut targets = Vec::new();
    if let Ok(executable) = resolve_browser_executable(config) {
        targets.push(BrowserLaunchTarget {
            runtime: "chromium-cdp".to_string(),
            executable,
        });
    }
    if let Ok(executable) = resolve_webview2_executable() {
        if !targets
            .iter()
            .any(|target| target.executable.eq_ignore_ascii_case(executable.as_str()))
        {
            targets.push(BrowserLaunchTarget {
                runtime: "webview2".to_string(),
                executable,
            });
        }
    }
    targets
}

fn preferred_browser_runtime_name(config: &BackendConfig) -> &'static str {
    if resolve_browser_executable(config).is_ok() {
        "chromium-cdp"
    } else if resolve_webview2_executable().is_ok() {
        "webview2"
    } else {
        "browser-simulated"
    }
}

fn browser_dependency_ready(config: &BackendConfig) -> bool {
    !browser_launch_targets(config).is_empty()
}

fn browser_artifact_root(
    config: &BackendConfig,
    browser_session_id: &str,
) -> Result<PathBuf, ServerError> {
    let root = PathBuf::from(&config.event_dir)
        .join("browser-artifacts")
        .join(browser_session_id);
    fs::create_dir_all(&root).map_err(|_| ServerError::Internal)?;
    Ok(root)
}

fn cleanup_failed_browser_launch(mut child: StdChild, user_data_dir: &Path, artifact_dir: &Path) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_dir_all(user_data_dir);
    let _ = fs::remove_dir_all(artifact_dir);
}

fn launch_browser_process_for_target(
    config: &BackendConfig,
    browser_session_id: &str,
    target: &BrowserLaunchTarget,
) -> Result<Arc<BrowserProcessRuntime>, ServerError> {
    let executable = target.executable.clone();
    let user_data_dir = PathBuf::from(&config.event_dir)
        .join("browser-runtime")
        .join(browser_session_id);
    fs::create_dir_all(&user_data_dir).map_err(|_| ServerError::Internal)?;
    let artifact_dir = browser_artifact_root(config, browser_session_id)?;

    let mut command = StdCommand::new(&executable);
    command
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", user_data_dir.display()))
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
    for arg in &config.browser_launch_args {
        command.arg(arg);
    }
    let child = command.spawn().map_err(|_| ServerError::Internal)?;

    let devtools_file = user_data_dir.join("DevToolsActivePort");
    let started = Instant::now();
    let port = loop {
        if let Ok(content) = fs::read_to_string(&devtools_file) {
            if let Some(first_line) = content.lines().next() {
                if let Ok(port) = first_line.trim().parse::<u16>() {
                    break port;
                }
            }
        }
        if started.elapsed() > Duration::from_secs(10) {
            cleanup_failed_browser_launch(child, &user_data_dir, &artifact_dir);
            return Err(ServerError::Internal);
        }
        thread::sleep(Duration::from_millis(50));
    };

    let http_base_url = format!("http://127.0.0.1:{port}");
    let version_payload =
        match browser_http_request("GET", &format!("{http_base_url}/json/version")) {
            Ok(payload) => payload,
            Err(_) => {
                cleanup_failed_browser_launch(child, &user_data_dir, &artifact_dir);
                return Err(ServerError::Internal);
            }
        };
    let version: Value = match serde_json::from_str(&version_payload) {
        Ok(value) => value,
        Err(_) => {
            cleanup_failed_browser_launch(child, &user_data_dir, &artifact_dir);
            return Err(ServerError::Internal);
        }
    };
    let websocket_url = match version.get("webSocketDebuggerUrl").and_then(Value::as_str) {
        Some(value) => value.to_string(),
        None => {
            cleanup_failed_browser_launch(child, &user_data_dir, &artifact_dir);
            return Err(ServerError::Internal);
        }
    };

    Ok(Arc::new(BrowserProcessRuntime {
        runtime: target.runtime.clone(),
        executable,
        port,
        http_base_url,
        websocket_url,
        user_data_dir,
        artifact_dir,
        child: StdMutex::new(child),
    }))
}

fn launch_browser_process(
    config: &BackendConfig,
    browser_session_id: &str,
) -> Result<Arc<BrowserProcessRuntime>, ServerError> {
    let mut last_error = None;
    for target in browser_launch_targets(config) {
        match launch_browser_process_for_target(config, browser_session_id, &target) {
            Ok(process) => return Ok(process),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(ServerError::Internal))
}

fn browser_target_list(
    process: &BrowserProcessRuntime,
) -> Result<Vec<BrowserTargetInfo>, ServerError> {
    let payload = browser_http_request("GET", &format!("{}/json/list", process.http_base_url))?;
    let values: Vec<Value> = serde_json::from_str(&payload).map_err(|_| ServerError::Internal)?;
    let mut targets = Vec::new();
    for value in values {
        if value.get("type").and_then(Value::as_str) != Some("page") {
            continue;
        }
        targets.push(BrowserTargetInfo {
            target_id: value
                .get("id")
                .and_then(Value::as_str)
                .ok_or(ServerError::Internal)?
                .to_string(),
            title: value
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            url: value
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or("about:blank")
                .to_string(),
            websocket_url: value
                .get("webSocketDebuggerUrl")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        });
    }
    Ok(targets)
}

fn cdp_browser_command(
    process: &BrowserProcessRuntime,
    method: &str,
    params: Value,
) -> Result<Value, ServerError> {
    let mut connection = CdpConnection::connect(&process.websocket_url)?;
    connection.command(method, params)
}

fn cdp_page_command(
    websocket_url: &str,
    method: &str,
    params: Value,
) -> Result<Value, ServerError> {
    let mut connection = CdpConnection::connect(websocket_url)?;
    let _ = connection.command("Runtime.enable", json!({}));
    let _ = connection.command("Page.enable", json!({}));
    let _ = connection.command("Network.enable", json!({}));
    let _ = connection.command("DOM.enable", json!({}));
    connection.command(method, params)
}

fn cdp_page_evaluate(websocket_url: &str, expression: &str) -> Result<Value, ServerError> {
    cdp_page_command(
        websocket_url,
        "Runtime.evaluate",
        json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true
        }),
    )
}

fn refresh_browser_tab_runtime(
    tab: &mut BrowserTabRuntime,
    process: &BrowserProcessRuntime,
) -> Result<(), ServerError> {
    if let Some(target) = browser_target_list(process)?
        .into_iter()
        .find(|target| target.target_id == tab.target_id)
    {
        tab.url = target.url;
        tab.title = target.title;
        if !target.websocket_url.is_empty() {
            tab.websocket_url = target.websocket_url;
        }
    }
    Ok(())
}

fn wait_for_page_state(websocket_url: &str, timeout_ms: u64) -> Result<String, ServerError> {
    let started = Instant::now();
    loop {
        let ready = cdp_page_evaluate(websocket_url, "document.readyState")?;
        if let Some(state) = ready
            .get("result")
            .and_then(|value| value.get("value"))
            .and_then(Value::as_str)
        {
            if matches!(state, "interactive" | "complete") {
                return Ok(state.to_string());
            }
        }
        if started.elapsed() > Duration::from_millis(timeout_ms.max(1)) {
            return Err(ServerError::Timeout);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn write_browser_artifact(
    process: &BrowserProcessRuntime,
    browser_tab_id: &str,
    kind: &str,
    extension: &str,
    bytes: &[u8],
) -> Result<String, ServerError> {
    fs::create_dir_all(&process.artifact_dir).map_err(|_| ServerError::Internal)?;
    let path = process.artifact_dir.join(format!(
        "{browser_tab_id}-{kind}-{}.{}",
        now_unix_ms(),
        extension
    ));
    fs::write(&path, bytes).map_err(|_| ServerError::Internal)?;
    Ok(path.to_string_lossy().to_string())
}

fn collect_artifact_stats(root: &Path) -> ArtifactStats {
    let mut stats = ArtifactStats::default();
    collect_artifact_stats_into(root, &mut stats);
    stats
}

fn collect_artifact_stats_into(path: &Path, stats: &mut ArtifactStats) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_artifact_stats_into(&path, stats);
        } else if let Ok(metadata) = entry.metadata() {
            stats.files = stats.files.saturating_add(1);
            stats.bytes = stats.bytes.saturating_add(metadata.len());
        }
    }
}

fn enforce_artifact_retention(
    root: &Path,
    session_dir: Option<&Path>,
    ttl_ms: u64,
    max_files: usize,
    max_total_bytes: u64,
    max_files_per_session: usize,
) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    let now = now_unix_ms();
    let ttl_cutoff = now.saturating_sub(ttl_ms);
    let mut all_files = list_artifacts(root)?;
    let mut session_files = if let Some(session_dir) = session_dir {
        list_artifacts(session_dir)?
    } else {
        Vec::new()
    };

    for file in all_files
        .iter()
        .filter(|file| file.modified_at_ms < ttl_cutoff)
        .cloned()
        .collect::<Vec<_>>()
    {
        let _ = fs::remove_file(&file.path);
    }

    all_files = list_artifacts(root)?;
    if let Some(session_dir) = session_dir {
        session_files = list_artifacts(session_dir)?;
    }
    all_files.sort_by_key(|file| file.modified_at_ms);
    session_files.sort_by_key(|file| file.modified_at_ms);

    while session_files.len() > max_files_per_session {
        if let Some(file) = session_files.first() {
            let _ = fs::remove_file(&file.path);
        }
        session_files = list_artifacts(session_dir.expect("session dir"))?;
        session_files.sort_by_key(|file| file.modified_at_ms);
    }

    while all_files.len() > max_files
        || all_files.iter().map(|file| file.bytes).sum::<u64>() > max_total_bytes
    {
        if let Some(file) = all_files.first() {
            let _ = fs::remove_file(&file.path);
        } else {
            break;
        }
        all_files = list_artifacts(root)?;
        all_files.sort_by_key(|file| file.modified_at_ms);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ArtifactFile {
    path: PathBuf,
    modified_at_ms: u64,
    bytes: u64,
}

fn list_artifacts(root: &Path) -> std::io::Result<Vec<ArtifactFile>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            files.extend(list_artifacts(&path)?);
        } else {
            let metadata = entry.metadata()?;
            let modified_at_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(0);
            files.push(ArtifactFile {
                path,
                modified_at_ms,
                bytes: metadata.len(),
            });
        }
    }
    Ok(files)
}

fn extract_agent_audit(params: &Value, method: &str) -> Result<AgentAuditContext, ServerError> {
    let object = params.as_object().ok_or(ServerError::InvalidRequest)?;
    let workspace_id = object
        .get("workspace_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?
        .to_string();
    let surface_id = object
        .get("surface_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?
        .to_string();
    let agent_worker_id = object
        .get("agent_worker_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let agent_task_id = object
        .get("agent_task_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let needs_worker = !matches!(
        method,
        "agent.worker.create" | "agent.worker.list" | "agent.task.list"
    );
    if needs_worker && method != "agent.task.cancel" && agent_worker_id.is_none() {
        return Err(ServerError::InvalidRequest);
    }
    if matches!(method, "agent.task.cancel" | "agent.task.get") && agent_task_id.is_none() {
        return Err(ServerError::InvalidRequest);
    }
    Ok(AgentAuditContext {
        workspace_id,
        surface_id,
        agent_worker_id,
        agent_task_id,
    })
}

fn extract_browser_audit(params: &Value, method: &str) -> Result<BrowserAuditContext, ServerError> {
    let object = params.as_object().ok_or(ServerError::InvalidRequest)?;
    let workspace_id = object
        .get("workspace_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?
        .to_string();
    let surface_id = object
        .get("surface_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?
        .to_string();

    let session_required = method != "browser.create";
    let browser_session_id = object
        .get("browser_session_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if session_required && browser_session_id.is_none() {
        return Err(ServerError::InvalidRequest);
    }
    if let Some(ref session) = browser_session_id {
        if BrowserSessionId::new(session.clone()).is_none() {
            return Err(ServerError::InvalidRequest);
        }
    }

    let tab_required = matches!(
        method,
        "browser.tab.focus"
            | "browser.tab.close"
            | "browser.goto"
            | "browser.reload"
            | "browser.back"
            | "browser.forward"
            | "browser.click"
            | "browser.type"
            | "browser.key"
            | "browser.wait"
            | "browser.screenshot"
            | "browser.evaluate"
            | "browser.storage.get"
            | "browser.storage.set"
            | "browser.network.intercept"
            | "browser.cookie.get"
            | "browser.cookie.set"
            | "browser.upload"
            | "browser.download"
            | "browser.trace.start"
            | "browser.trace.stop"
            | "browser.raw.command"
    );
    let tab_id = object
        .get("tab_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    if tab_required && tab_id.is_none() {
        return Err(ServerError::InvalidRequest);
    }
    if let Some(ref tab) = tab_id {
        if BrowserTabId::new(tab.clone()).is_none() {
            return Err(ServerError::InvalidRequest);
        }
    }

    Ok(BrowserAuditContext {
        workspace_id,
        surface_id,
        browser_session_id,
        tab_id,
    })
}

fn extract_terminal_audit(
    params: &Value,
    method: &str,
) -> Result<TerminalAuditContext, ServerError> {
    let object = params.as_object().ok_or(ServerError::InvalidRequest)?;
    let workspace_id = object
        .get("workspace_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?
        .to_string();
    let surface_id = object
        .get("surface_id")
        .and_then(Value::as_str)
        .ok_or(ServerError::InvalidRequest)?
        .to_string();
    let terminal_session_id = object
        .get("terminal_session_id")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let needs_session = method != "terminal.spawn";
    if needs_session && terminal_session_id.is_none() {
        return Err(ServerError::InvalidRequest);
    }
    Ok(TerminalAuditContext {
        workspace_id,
        surface_id,
        terminal_session_id,
    })
}

fn workload_class(method: &str) -> WorkloadClass {
    match method {
        "browser.goto" | "browser.reload" | "browser.back" | "browser.forward" | "browser.wait"
        | "browser.screenshot" | "browser.download" | "browser.trace.stop" | "terminal.spawn" => {
            WorkloadClass::Background
        }
        _ => WorkloadClass::Interactive,
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis() as u64
}

fn random_token() -> Result<String, ServerError> {
    let mut bytes = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut token = String::with_capacity(64);
    for b in bytes {
        use std::fmt::Write as _;
        if write!(&mut token, "{b:02x}").is_err() {
            return Err(ServerError::Internal);
        }
    }
    Ok(token)
}

fn random_event_id() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut out = String::from("evt-");
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_config(label: &str) -> BackendConfig {
        BackendConfig {
            event_dir: std::env::temp_dir()
                .join(format!("maxc-server-{label}-{}", now_unix_ms()))
                .to_string_lossy()
                .to_string(),
            browser_executable_or_channel: "__synthetic__".to_string(),
            terminal_runtime: "process-stdio".to_string(),
            snapshot_interval_events: 2,
            snapshot_retain_count: 2,
            segment_max_bytes: 1024 * 64,
            ..BackendConfig::default()
        }
    }

    fn test_terminal_shell() -> &'static str {
        if cfg!(windows) {
            "cmd"
        } else {
            "sh"
        }
    }

    fn test_terminal_echo_command(marker: &str) -> String {
        if cfg!(windows) {
            format!("echo {marker}")
        } else {
            format!("printf '{marker}\\n'")
        }
    }

    #[tokio::test]
    async fn session_create_and_refresh_with_command_id() {
        let cfg = test_config("create-refresh");
        let server = RpcServer::new(cfg).expect("server");
        let create_request = RpcRequest {
            id: Some(RpcId::Number(1)),
            method: "session.create".to_string(),
            params: Some(json!({ "command_id": "cmd-1" })),
        };

        let create_response = server
            .handle_request("conn-a", create_request)
            .await
            .expect("create");
        let token = create_response
            .get("result")
            .and_then(|r| r.get("token"))
            .and_then(Value::as_str)
            .expect("token")
            .to_string();

        let refresh_request = RpcRequest {
            id: Some(RpcId::Number(2)),
            method: "session.refresh".to_string(),
            params: Some(json!({
                "command_id": "cmd-2",
                "auth": {
                    "token": token.clone()
                }
            })),
        };
        let refresh_response = server
            .handle_request("conn-a", refresh_request)
            .await
            .expect("refresh");
        assert_eq!(refresh_response["result"]["token"], token);
        assert_eq!(server.session_count().await, 1);
    }

    #[tokio::test]
    async fn create_without_command_id_is_invalid_request() {
        let cfg = test_config("missing-cmd");
        let server = RpcServer::new(cfg).expect("server");
        let output = server
            .handle_json_line(
                "conn-a",
                r#"{"id":1,"method":"session.create","params":{}}"#,
            )
            .await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["error"]["code"], "INVALID_REQUEST");
    }

    #[tokio::test]
    async fn idempotent_command_returns_same_result() {
        let cfg = test_config("idem");
        let server = RpcServer::new(cfg).expect("server");
        let request = json!({
            "id": 1,
            "method": "session.create",
            "params": { "command_id": "cmd-1" }
        })
        .to_string();

        let first: Value =
            serde_json::from_str(&server.handle_json_line("c", &request).await).expect("json");
        let second: Value =
            serde_json::from_str(&server.handle_json_line("c", &request).await).expect("json");
        assert_eq!(first["result"], second["result"]);
        assert_eq!(server.session_count().await, 1);
    }

    #[tokio::test]
    async fn recover_state_from_event_store() {
        let cfg = test_config("recovery");
        let event_dir = cfg.event_dir.clone();
        let server = RpcServer::new(cfg.clone()).expect("server");
        let create = json!({
            "id": 1,
            "method": "session.create",
            "params": { "command_id": "cmd-1" }
        })
        .to_string();
        let _ = server.handle_json_line("c", &create).await;

        let restarted = RpcServer::new(BackendConfig { event_dir, ..cfg }).expect("restart");
        assert_eq!(restarted.session_count().await, 1);
    }

    #[tokio::test]
    async fn unknown_method_maps_to_not_found() {
        let cfg = test_config("not-found");
        let server = RpcServer::new(cfg).expect("server");
        let input = json!({
            "id": "x1",
            "method": "unknown.method"
        })
        .to_string();
        let output = server.handle_json_line("conn-x", &input).await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["error"]["code"], "NOT_FOUND");
        assert_eq!(parsed["error"]["data"]["correlation_id"], "corr-1");
    }

    #[tokio::test]
    async fn invalid_payload_maps_to_invalid_request() {
        let cfg = BackendConfig {
            max_payload_bytes: 5,
            ..test_config("invalid-payload")
        };
        let server = RpcServer::new(cfg).expect("server");
        let output = server
            .handle_json_line("conn-z", r#"{"id":1,"method":"system.health"}"#)
            .await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["error"]["code"], "INVALID_REQUEST");
    }

    #[tokio::test]
    async fn rate_limit_returns_rate_limited() {
        let cfg = BackendConfig {
            rate_limit_per_sec: 1,
            burst_limit: 1,
            ..test_config("rate")
        };
        let server = RpcServer::new(cfg).expect("server");
        let req = json!({"id": 1, "method": "system.health"}).to_string();
        let _ = server.handle_json_line("c", &req).await;
        let second = server.handle_json_line("c", &req).await;
        let parsed: Value = serde_json::from_str(&second).expect("json");
        assert_eq!(parsed["error"]["code"], "RATE_LIMITED");
    }

    #[tokio::test]
    async fn browser_create_requires_auth_token_and_audit_fields() {
        let cfg = test_config("browser-auth");
        let server = RpcServer::new(cfg).expect("server");
        let no_auth = json!({
            "id": 1,
            "method": "browser.create",
            "params": {
                "command_id": "cmd-b1",
                "workspace_id": "ws-1",
                "surface_id": "sf-1"
            }
        })
        .to_string();
        let first: Value =
            serde_json::from_str(&server.handle_json_line("c", &no_auth).await).expect("json");
        assert_eq!(first["error"]["code"], "UNAUTHORIZED");

        let session = json!({
            "id": 2,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-1"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let with_auth = json!({
            "id": 3,
            "method": "browser.create",
            "params": {
                "command_id": "cmd-b2",
                "workspace_id": "ws-1",
                "surface_id": "sf-1",
                "auth": {"token": token}
            }
        })
        .to_string();
        let second: Value =
            serde_json::from_str(&server.handle_json_line("c", &with_auth).await).expect("json");
        assert!(second.get("result").is_some());
    }

    #[tokio::test]
    async fn terminal_lifecycle_and_subscription_work() {
        let cfg = BackendConfig {
            queue_limit: 8,
            ..test_config("terminal-lifecycle")
        };
        let expect_resize_applied = selected_terminal_runtime_name(&cfg) == "conpty";
        let server = RpcServer::new(cfg).expect("server");

        let session = json!({
            "id": 1,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-terminal"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let spawn = json!({
            "id": 2,
            "method":"terminal.spawn",
            "params":{
                "command_id":"cmd-term-spawn",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-1",
                "auth":{"token":token},
                "shell": test_terminal_shell(),
                "cols": 100,
                "rows": 40
            }
        })
        .to_string();
        let spawn_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &spawn).await).expect("json");
        let terminal_session_id = spawn_out["result"]["terminal_session_id"]
            .as_str()
            .expect("terminal session");
        assert_eq!(
            spawn_out["result"]["runtime"],
            selected_terminal_runtime_name(&server.config)
        );
        assert!(spawn_out["result"]["pid"].as_u64().unwrap_or_default() > 0);

        let subscribe = json!({
            "id": 3,
            "method":"terminal.subscribe",
            "params":{
                "command_id":"cmd-term-sub",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-1",
                "terminal_session_id": terminal_session_id,
                "auth":{"token":token}
            }
        })
        .to_string();
        let subscribe_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &subscribe).await).expect("json");
        assert_eq!(subscribe_out["result"]["subscribed"], true);
        assert!(subscribe_out["result"]["subscriber_id"].as_str().is_some());

        let input = json!({
            "id": 10,
            "method":"terminal.input",
            "params":{
                "command_id": "cmd-term-input-0",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-1",
                "terminal_session_id": terminal_session_id,
                "auth":{"token":token},
                "input": test_terminal_echo_command("terminal-lifecycle-ok")
            }
        })
        .to_string();
        let out: Value =
            serde_json::from_str(&server.handle_json_line("c", &input).await).expect("json");
        assert_eq!(out["result"]["accepted"], true);
        assert!(out["result"]["bytes"].as_u64().unwrap_or_default() > 0);

        let mut observed_output = false;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let history = json!({
                "id": 30,
                "method":"terminal.history",
                "params":{
                    "command_id":"cmd-term-history-check",
                    "workspace_id":"ws-1",
                    "surface_id":"sf-t-1",
                    "terminal_session_id": terminal_session_id,
                    "from_sequence": 1,
                    "max_events": 20,
                    "auth":{"token":token}
                }
            })
            .to_string();
            let history_out: Value =
                serde_json::from_str(&server.handle_json_line("c", &history).await).expect("json");
            if history_out["result"]["last_sequence"]
                .as_u64()
                .unwrap_or_default()
                > 1
                && history_out["result"]["events"]
                    .as_array()
                    .expect("events")
                    .iter()
                    .any(|event| event["type"] == "terminal.output")
            {
                observed_output = true;
                break;
            }
        }
        assert!(observed_output);

        let second_spawn = json!({
            "id": 20,
            "method":"terminal.spawn",
            "params":{
                "command_id":"cmd-term-spawn-2",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-2",
                "auth":{"token":token},
                "shell": test_terminal_shell()
            }
        })
        .to_string();
        let second_spawn_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &second_spawn).await).expect("json");
        let second_terminal_session_id = second_spawn_out["result"]["terminal_session_id"]
            .as_str()
            .expect("second terminal");

        let resize = json!({
            "id": 21,
            "method":"terminal.resize",
            "params":{
                "command_id":"cmd-term-resize",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-2",
                "terminal_session_id": second_terminal_session_id,
                "auth":{"token":token},
                "cols": 120,
                "rows": 45
            }
        })
        .to_string();
        let resize_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &resize).await).expect("json");
        assert_eq!(resize_out["result"]["cols"], 120);
        assert_eq!(resize_out["result"]["applied"], expect_resize_applied);

        let kill = json!({
            "id": 22,
            "method":"terminal.kill",
            "params":{
                "command_id":"cmd-term-kill",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-2",
                "terminal_session_id": second_terminal_session_id,
                "auth":{"token":token}
            }
        })
        .to_string();
        let kill_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &kill).await).expect("json");
        assert_eq!(kill_out["result"]["killed"], true);
    }

    #[tokio::test]
    async fn terminal_history_returns_sequence_buffer() {
        let cfg = BackendConfig {
            terminal_max_history_events: 32,
            ..test_config("terminal-history")
        };
        let server = RpcServer::new(cfg).expect("server");

        let session = json!({
            "id": 1,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-terminal-history"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let spawn = json!({
            "id": 2,
            "method":"terminal.spawn",
            "params":{
                "command_id":"cmd-term-history-spawn",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-1",
                "auth":{"token":token},
                "shell": test_terminal_shell()
            }
        })
        .to_string();
        let spawn_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &spawn).await).expect("json");
        let terminal_session_id = spawn_out["result"]["terminal_session_id"]
            .as_str()
            .expect("terminal session");

        let input = json!({
            "id": 3,
            "method":"terminal.input",
            "params":{
                "command_id":"cmd-term-history-input",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-1",
                "terminal_session_id": terminal_session_id,
                "auth":{"token":token},
                "input": test_terminal_echo_command("history-sequence-ok")
            }
        })
        .to_string();
        let _: Value =
            serde_json::from_str(&server.handle_json_line("c", &input).await).expect("json");

        tokio::time::sleep(Duration::from_millis(100)).await;

        let history = json!({
            "id": 4,
            "method":"terminal.history",
            "params":{
                "command_id":"cmd-term-history-read",
                "workspace_id":"ws-1",
                "surface_id":"sf-t-1",
                "terminal_session_id": terminal_session_id,
                "from_sequence": 1,
                "max_events": 10,
                "auth":{"token":token}
            }
        })
        .to_string();
        let history_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &history).await).expect("json");
        assert!(
            history_out["result"]["last_sequence"]
                .as_u64()
                .unwrap_or_default()
                >= 1
        );
        let events = history_out["result"]["events"].as_array().expect("events");
        assert!(!events.is_empty());
        assert!(events
            .iter()
            .all(|event| event["sequence"].as_u64().is_some()));
    }

    #[tokio::test]
    async fn browser_advanced_controls_are_applied() {
        let cfg = test_config("browser-advanced");
        let server = RpcServer::new(cfg).expect("server");

        let session = json!({
            "id": 1,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-advanced"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let create = json!({
            "id": 2,
            "method": "browser.create",
            "params": {
                "command_id":"cmd-b-advanced-create",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "auth":{"token": token}
            }
        })
        .to_string();
        let create_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &create).await).expect("json");
        let bs = create_out["result"]["browser_session_id"]
            .as_str()
            .expect("browser session");

        let tab_open = json!({
            "id": 3,
            "method":"browser.tab.open",
            "params":{
                "command_id":"cmd-b-advanced-tab",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "auth":{"token": token},
                "url":"https://example.com"
            }
        })
        .to_string();
        let tab_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &tab_open).await).expect("json");
        let tab = tab_out["result"]["browser_tab_id"].as_str().expect("tab");

        for (idx, method) in [
            "browser.storage.set",
            "browser.storage.get",
            "browser.network.intercept",
            "browser.trace.start",
            "browser.trace.stop",
        ]
        .iter()
        .enumerate()
        {
            let req = json!({
                "id": 10 + idx as i64,
                "method": method,
                "params":{
                    "command_id": format!("cmd-b-advanced-{idx}"),
                    "workspace_id":"ws-1",
                    "surface_id":"sf-1",
                    "browser_session_id": bs,
                    "tab_id": tab,
                    "auth":{"token": token}
                }
            })
            .to_string();
            let out: Value =
                serde_json::from_str(&server.handle_json_line("c", &req).await).expect("json");
            assert!(out.get("result").is_some());
        }
    }

    #[tokio::test]
    async fn shutdown_rejects_new_requests_and_drains_runtime() {
        let cfg = BackendConfig {
            shutdown_drain_timeout_ms: 50,
            ..test_config("shutdown")
        };
        let server = RpcServer::new(cfg).expect("server");
        server.begin_shutdown();

        let denied = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 1,
                    "method": "system.health"
                })
                .to_string(),
            )
            .await;
        let denied: Value = serde_json::from_str(&denied).expect("json");
        assert_eq!(denied["error"]["code"], "RATE_LIMITED");

        server.shutdown_and_drain().await;
        assert!(server.state.terminal_runtime.lock().await.is_empty());
        assert!(server.state.browser_runtime.lock().await.is_empty());
    }

    #[tokio::test]
    async fn overload_threshold_rejects_requests() {
        let cfg = BackendConfig {
            overload_reject_threshold: 1,
            ..test_config("overload")
        };
        let server = RpcServer::new(cfg).expect("server");
        {
            let mut inflight = server
                .state
                .inflight_by_connection
                .lock()
                .expect("inflight lock");
            inflight.insert("conn-a".to_string(), 1);
        }
        let output = server
            .handle_json_line(
                "conn-b",
                &json!({
                    "id": 1,
                    "method": "system.health"
                })
                .to_string(),
            )
            .await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["error"]["code"], "RATE_LIMITED");
    }

    #[tokio::test]
    async fn fault_injection_opens_breaker_and_then_recovers() {
        let cfg = BackendConfig {
            breaker_failure_threshold: 2,
            breaker_cooldown_ms: 5,
            ..test_config("breaker")
        };
        let mut faults = HashMap::new();
        faults.insert(FaultHook::MethodDispatch, FaultAction::ReturnInternal);
        let server = RpcServer::new_with_faults(cfg, faults).expect("server");

        let request = json!({
            "id": 1,
            "method": "system.health"
        })
        .to_string();
        for _ in 0..2 {
            let out = server.handle_json_line("conn-a", &request).await;
            let parsed: Value = serde_json::from_str(&out).expect("json");
            assert_eq!(parsed["error"]["code"], "INTERNAL");
        }

        let blocked = server.handle_json_line("conn-a", &request).await;
        let blocked: Value = serde_json::from_str(&blocked).expect("json");
        assert_eq!(blocked["error"]["code"], "RATE_LIMITED");

        tokio::time::sleep(Duration::from_millis(10)).await;
        server.state.faults.lock().expect("fault lock").clear();
        let recovered = server.handle_json_line("conn-a", &request).await;
        let recovered: Value = serde_json::from_str(&recovered).expect("json");
        assert_eq!(recovered["result"]["ok"], true);
    }

    #[tokio::test]
    async fn fault_injection_can_fail_persist_path() {
        let cfg = test_config("persist-fault");
        let mut faults = HashMap::new();
        faults.insert(FaultHook::StoreAppend, FaultAction::ReturnInternal);
        let server = RpcServer::new_with_faults(cfg, faults).expect("server");

        let create = json!({
            "id": 1,
            "method": "session.create",
            "params": { "command_id": "cmd-1" }
        })
        .to_string();
        let output = server.handle_json_line("conn-a", &create).await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["error"]["code"], "INTERNAL");
    }

    #[tokio::test]
    async fn response_fault_can_force_timeout_path() {
        let cfg = test_config("response-fault");
        let mut faults = HashMap::new();
        faults.insert(FaultHook::Response, FaultAction::DropResponse);
        let server = RpcServer::new_with_faults(cfg, faults).expect("server");

        let output = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 1,
                    "method": "system.health"
                })
                .to_string(),
            )
            .await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["error"]["code"], "TIMEOUT");
    }

    #[tokio::test]
    async fn delay_fault_slows_dispatch_without_failing() {
        let cfg = test_config("delay-fault");
        let mut faults = HashMap::new();
        faults.insert(FaultHook::MethodDispatch, FaultAction::DelayMs(5));
        let server = RpcServer::new_with_faults(cfg, faults).expect("server");

        let start = Instant::now();
        let output = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 1,
                    "method": "system.health"
                })
                .to_string(),
            )
            .await;
        let parsed: Value = serde_json::from_str(&output).expect("json");
        assert_eq!(parsed["result"]["ok"], true);
        assert!(start.elapsed() >= Duration::from_millis(5));
    }

    #[tokio::test]
    async fn diagnostics_metrics_and_logs_require_auth_and_emit_data() {
        let cfg = test_config("system-observability");
        let server = RpcServer::new(cfg).expect("server");

        let denied = server
            .handle_json_line(
                "conn-a",
                &json!({"id":1,"method":"system.readiness"}).to_string(),
            )
            .await;
        let denied: Value = serde_json::from_str(&denied).expect("json");
        assert_eq!(denied["error"]["code"], "UNAUTHORIZED");

        let create = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 2,
                    "method": "session.create",
                    "params": {"command_id":"cmd-system-auth"}
                })
                .to_string(),
            )
            .await;
        let create: Value = serde_json::from_str(&create).expect("json");
        let token = create["result"]["token"].as_str().expect("token");

        let readiness = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 3,
                    "method": "system.readiness",
                    "params": {"command_id":"cmd-ready","auth":{"token": token}}
                })
                .to_string(),
            )
            .await;
        let readiness: Value = serde_json::from_str(&readiness).expect("json");
        assert!(readiness["result"]["ready"].is_boolean());
        assert!(readiness["result"]["artifact_root_ready"].is_boolean());
        assert!(readiness["result"]["event_store_ready"].is_boolean());

        let metrics = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 4,
                    "method": "system.metrics",
                    "params": {"command_id":"cmd-metrics","auth":{"token": token}}
                })
                .to_string(),
            )
            .await;
        let metrics: Value = serde_json::from_str(&metrics).expect("json");
        assert!(metrics["result"]["counters"].is_object());
        assert!(metrics["result"]["gauges"]["runtime.artifacts.ready"].is_u64());
        assert!(metrics["result"]["gauges"]["storage.event_dir.ready"].is_u64());

        let logs = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 5,
                    "method": "system.logs",
                    "params": {"command_id":"cmd-logs","auth":{"token": token}}
                })
                .to_string(),
            )
            .await;
        let logs: Value = serde_json::from_str(&logs).expect("json");
        assert!(logs["result"]["logs"].is_array());

        let diagnostics = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 6,
                    "method": "system.diagnostics",
                    "params": {"command_id":"cmd-diag","auth":{"token": token}}
                })
                .to_string(),
            )
            .await;
        let diagnostics: Value = serde_json::from_str(&diagnostics).expect("json");
        assert!(diagnostics["result"]["metrics"].is_object());
        assert!(diagnostics["result"]["artifact_root_ready"].is_boolean());
        assert!(diagnostics["result"]["event_store_ready"].is_boolean());
        assert!(!server.telemetry_snapshot().logs.is_empty());
    }

    #[tokio::test]
    async fn readiness_reports_dependency_failures() {
        let cfg = test_config("dependency-readiness");
        let event_dir = PathBuf::from(&cfg.event_dir);
        let server = RpcServer::new(cfg).expect("server");
        let create = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 1,
                    "method": "session.create",
                    "params": {"command_id":"cmd-dependency-auth","scopes":["diagnostics"]}
                })
                .to_string(),
            )
            .await;
        let create: Value = serde_json::from_str(&create).expect("json");
        let token = create["result"]["token"].as_str().expect("token");

        let artifact_root = event_dir.join("browser-artifacts");
        let _ = fs::remove_dir_all(&artifact_root);
        fs::write(&artifact_root, b"blocked").expect("artifact blocker");
        let readiness = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 2,
                    "method": "system.readiness",
                    "params": {"command_id":"cmd-dependency-ready-1","auth":{"token": token}}
                })
                .to_string(),
            )
            .await;
        let readiness: Value = serde_json::from_str(&readiness).expect("json");
        assert_eq!(readiness["result"]["artifact_root_ready"], false);
        assert_eq!(readiness["result"]["ready"], false);

        let _ = fs::remove_file(&artifact_root);
        let _ = fs::remove_dir_all(&event_dir);
        fs::write(&event_dir, b"blocked").expect("event dir blocker");
        let readiness = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id": 3,
                    "method": "system.readiness",
                    "params": {"command_id":"cmd-dependency-ready-2","auth":{"token": token}}
                })
                .to_string(),
            )
            .await;
        let readiness: Value = serde_json::from_str(&readiness).expect("json");
        assert_eq!(readiness["result"]["event_store_ready"], false);
        assert_eq!(readiness["result"]["store_available"], false);
        let _ = fs::remove_file(event_dir);
    }

    #[tokio::test]
    async fn restart_recovers_metadata_without_restoring_live_browser_runtime() {
        let cfg = test_config("restart-browser-runtime");
        let event_dir = cfg.event_dir.clone();
        let server = RpcServer::new(cfg.clone()).expect("server");
        let auth = server
            .handle_json_line(
                "conn-a",
                &json!({"id":1,"method":"session.create","params":{"command_id":"cmd-restart-auth"}})
                    .to_string(),
            )
            .await;
        let auth: Value = serde_json::from_str(&auth).expect("json");
        let token = auth["result"]["token"].as_str().expect("token");

        let browser = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id":2,
                    "method":"browser.create",
                    "params":{
                        "command_id":"cmd-restart-browser",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;
        let browser: Value = serde_json::from_str(&browser).expect("json");
        assert!(browser.get("result").is_some());

        let restarted = RpcServer::new(BackendConfig { event_dir, ..cfg }).expect("restart");
        let auth = restarted
            .handle_json_line(
                "conn-b",
                &json!({"id":3,"method":"session.create","params":{"command_id":"cmd-restart-auth-2","scopes":["diagnostics"]}})
                    .to_string(),
            )
            .await;
        let auth: Value = serde_json::from_str(&auth).expect("json");
        let diagnostics_token = auth["result"]["token"].as_str().expect("token");
        let diagnostics = restarted
            .handle_json_line(
                "conn-b",
                &json!({
                    "id":4,
                    "method":"system.diagnostics",
                    "params":{"command_id":"cmd-restart-diag","auth":{"token":diagnostics_token}}
                })
                .to_string(),
            )
            .await;
        let diagnostics: Value = serde_json::from_str(&diagnostics).expect("json");
        assert_eq!(diagnostics["result"]["browser_sessions"].as_u64(), Some(1));
        assert_eq!(
            diagnostics["result"]["browser_runtime_count"].as_u64(),
            Some(0)
        );
    }

    #[tokio::test]
    async fn mixed_runtime_stress_flow_keeps_diagnostics_consistent() {
        let cfg = BackendConfig {
            queue_limit: 32,
            ..test_config("mixed-stress")
        };
        let server = RpcServer::new(cfg).expect("server");
        let auth = server
            .handle_json_line(
                "conn-a",
                &json!({"id":1,"method":"session.create","params":{"command_id":"cmd-stress-auth"}})
                    .to_string(),
            )
            .await;
        let auth: Value = serde_json::from_str(&auth).expect("json");
        let token = auth["result"]["token"].as_str().expect("token");

        for idx in 0..3 {
            let spawn = server
                .handle_json_line(
                    "conn-a",
                    &json!({
                        "id": 10 + idx,
                        "method":"terminal.spawn",
                        "params":{
                            "command_id": format!("cmd-stress-term-{idx}"),
                            "workspace_id":"ws-1",
                            "surface_id": format!("sf-term-{idx}"),
                            "auth":{"token":token},
                            "shell": test_terminal_shell()
                        }
                    })
                    .to_string(),
                )
                .await;
            let spawn: Value = serde_json::from_str(&spawn).expect("json");
            assert!(spawn.get("result").is_some());
        }

        let browser = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id":20,
                    "method":"browser.create",
                    "params":{
                        "command_id":"cmd-stress-browser",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-browser",
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;
        let browser: Value = serde_json::from_str(&browser).expect("json");
        let browser_session_id = browser["result"]["browser_session_id"]
            .as_str()
            .expect("browser session");
        for idx in 0..3 {
            let tab = server
                .handle_json_line(
                    "conn-a",
                    &json!({
                        "id": 30 + idx,
                        "method":"browser.tab.open",
                        "params":{
                            "command_id": format!("cmd-stress-tab-{idx}"),
                            "workspace_id":"ws-1",
                            "surface_id":"sf-browser",
                            "browser_session_id": browser_session_id,
                            "auth":{"token":token},
                            "url": format!("https://example.com/{idx}")
                        }
                    })
                    .to_string(),
                )
                .await;
            let tab: Value = serde_json::from_str(&tab).expect("json");
            assert!(tab.get("result").is_some());
        }

        let worker = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id":40,
                    "method":"agent.worker.create",
                    "params":{
                        "command_id":"cmd-stress-worker",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-agent",
                        "auth":{"token":token},
                        "shell": test_terminal_shell()
                    }
                })
                .to_string(),
            )
            .await;
        let worker: Value = serde_json::from_str(&worker).expect("json");
        assert!(worker.get("result").is_some());

        let diagnostics = server
            .handle_json_line(
                "conn-a",
                &json!({
                    "id":50,
                    "method":"system.diagnostics",
                    "params":{"command_id":"cmd-stress-diag","auth":{"token":token}}
                })
                .to_string(),
            )
            .await;
        let diagnostics: Value = serde_json::from_str(&diagnostics).expect("json");
        assert!(
            diagnostics["result"]["terminal_runtime_count"]
                .as_u64()
                .unwrap_or_default()
                >= 3
        );
        assert!(
            diagnostics["result"]["browser_tabs"]
                .as_u64()
                .unwrap_or_default()
                >= 3
        );
        assert!(
            diagnostics["result"]["agent_runtime_count"]
                .as_u64()
                .unwrap_or_default()
                >= 1
        );
    }

    #[tokio::test]
    async fn browser_raw_requires_allow_raw_and_limits() {
        let cfg = BackendConfig {
            browser_raw_rate_limit_per_sec: 1,
            ..test_config("browser-raw")
        };
        let server = RpcServer::new(cfg).expect("server");
        let session = json!({
            "id": 1,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-raw"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let create = json!({
            "id": 2,
            "method": "browser.create",
            "params": {
                "command_id":"cmd-bcreate",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "auth":{"token": token}
            }
        })
        .to_string();
        let create_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &create).await).expect("json");
        let browser_session_id = create_out["result"]["browser_session_id"]
            .as_str()
            .expect("browser session");

        let tab_open = json!({
            "id":3,
            "method":"browser.tab.open",
            "params":{
                "command_id":"cmd-tab-open",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": browser_session_id,
                "auth":{"token": token},
                "url":"https://example.com"
            }
        })
        .to_string();
        let tab_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &tab_open).await).expect("json");
        let tab_id = tab_out["result"]["browser_tab_id"].as_str().expect("tab");

        let raw_denied = json!({
            "id":4,
            "method":"browser.raw.command",
            "params":{
                "command_id":"cmd-raw-1",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth":{"token": token},
                "raw_command":"Network.enable"
            }
        })
        .to_string();
        let denied: Value =
            serde_json::from_str(&server.handle_json_line("c", &raw_denied).await).expect("json");
        assert_eq!(denied["error"]["code"], "UNAUTHORIZED");

        let raw_allowed = json!({
            "id":5,
            "method":"browser.raw.command",
            "params":{
                "command_id":"cmd-raw-2",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": browser_session_id,
                "tab_id": tab_id,
                "auth":{"token": token},
                "allow_raw": true,
                "raw_command":"Network.enable"
            }
        })
        .to_string();
        let allowed: Value =
            serde_json::from_str(&server.handle_json_line("c", &raw_allowed).await).expect("json");
        assert!(allowed.get("result").is_some());
    }

    #[tokio::test]
    async fn session_scopes_gate_diagnostics_runtime_and_agent_methods() {
        let server = RpcServer::new(test_config("session-scopes")).expect("server");

        let create = server
            .handle_json_line(
                "c",
                &json!({
                    "id": 1,
                    "method": "session.create",
                    "params": {
                        "command_id":"cmd-scope-create",
                        "scopes":["diagnostics"]
                    }
                })
                .to_string(),
            )
            .await;
        let created: Value = serde_json::from_str(&create).expect("json");
        let token = created["result"]["token"].as_str().expect("token");
        assert_eq!(created["result"]["scopes"], json!(["diagnostics"]));

        let diagnostics = server
            .handle_json_line(
                "c",
                &json!({
                    "id": 2,
                    "method": "system.diagnostics",
                    "params": {"command_id":"cmd-scope-diag","auth":{"token":token}}
                })
                .to_string(),
            )
            .await;
        let diagnostics: Value = serde_json::from_str(&diagnostics).expect("json");
        assert!(diagnostics.get("result").is_some());

        let browser = server
            .handle_json_line(
                "c",
                &json!({
                    "id": 3,
                    "method": "browser.create",
                    "params": {
                        "command_id":"cmd-scope-browser",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;
        let browser: Value = serde_json::from_str(&browser).expect("json");
        assert_eq!(browser["error"]["code"], "UNAUTHORIZED");
    }

    #[test]
    fn redaction_and_artifact_retention_helpers_work() {
        let redacted = redact_value(&json!({
            "token":"secret-token",
            "prompt":"a very long prompt body that should not stay fully visible in logs",
            "nested":{"password":"abc123"}
        }));
        assert_eq!(redacted["token"], "[redacted]");
        assert!(redacted["prompt"].as_str().unwrap_or_default().len() <= 67);
        assert_eq!(redacted["nested"]["password"], "[redacted]");

        let root = std::env::temp_dir().join(format!("maxc-phase11-artifacts-{}", now_unix_ms()));
        let session = root.join("session-a");
        fs::create_dir_all(&session).expect("artifact dir");
        fs::write(session.join("old.txt"), b"1234").expect("old");
        std::thread::sleep(Duration::from_millis(5));
        fs::write(session.join("new.txt"), b"5678").expect("new");
        enforce_artifact_retention(&root, Some(&session), 1, 1, 8, 1).expect("retention");
        let stats = collect_artifact_stats(&root);
        assert!(stats.files <= 1);
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn browser_command_idempotency_returns_same_result() {
        let cfg = test_config("browser-idem");
        let server = RpcServer::new(cfg).expect("server");
        let session = json!({
            "id": 1,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-idem"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let req = json!({
            "id": 2,
            "method": "browser.create",
            "params": {
                "command_id":"cmd-browser-idem-1",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "auth":{"token": token}
            }
        })
        .to_string();
        let first: Value =
            serde_json::from_str(&server.handle_json_line("c", &req).await).expect("json");
        let second: Value =
            serde_json::from_str(&server.handle_json_line("c", &req).await).expect("json");
        assert_eq!(first["result"], second["result"]);
    }

    #[tokio::test]
    async fn browser_full_route_matrix_is_dispatched() {
        let cfg = test_config("browser-matrix");
        let server = RpcServer::new(cfg).expect("server");

        let session = json!({
            "id": 1,
            "method": "session.create",
            "params": {"command_id":"cmd-auth-matrix"}
        })
        .to_string();
        let session_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &session).await).expect("json");
        let token = session_out["result"]["token"].as_str().expect("token");

        let create = json!({
            "id": 2,
            "method": "browser.create",
            "params": {
                "command_id":"cmd-bm-1",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "auth":{"token": token}
            }
        })
        .to_string();
        let create_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &create).await).expect("json");
        let bs = create_out["result"]["browser_session_id"]
            .as_str()
            .expect("browser session");

        for (idx, method) in ["browser.attach", "browser.detach", "browser.attach"]
            .iter()
            .enumerate()
        {
            let req = json!({
                "id": 10 + idx as i64,
                "method": method,
                "params": {
                    "command_id": format!("cmd-bm-attach-{idx}"),
                    "workspace_id":"ws-1",
                    "surface_id":"sf-1",
                    "browser_session_id": bs,
                    "auth":{"token": token}
                }
            })
            .to_string();
            let out: Value =
                serde_json::from_str(&server.handle_json_line("c", &req).await).expect("json");
            assert!(out.get("result").is_some());
        }

        let tab_open = json!({
            "id": 30,
            "method": "browser.tab.open",
            "params": {
                "command_id":"cmd-bm-tab-open",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "auth":{"token": token},
                "url":"https://example.com"
            }
        })
        .to_string();
        let tab_open_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &tab_open).await).expect("json");
        let tab = tab_open_out["result"]["browser_tab_id"]
            .as_str()
            .expect("tab");

        let list = json!({
            "id": 31,
            "method":"browser.tab.list",
            "params":{
                "command_id":"cmd-bm-tab-list",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "auth":{"token": token}
            }
        })
        .to_string();
        let list_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &list).await).expect("json");
        assert!(list_out["result"]["tabs"].is_array());

        for (idx, method) in [
            "browser.tab.focus",
            "browser.goto",
            "browser.reload",
            "browser.back",
            "browser.forward",
            "browser.click",
            "browser.type",
            "browser.wait",
            "browser.evaluate",
            "browser.cookie.get",
            "browser.cookie.set",
            "browser.upload",
            "browser.trace.start",
            "browser.trace.stop",
        ]
        .iter()
        .enumerate()
        {
            let mut params = json!({
                "command_id": format!("cmd-bm-op-{idx}"),
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "tab_id": tab,
                "auth":{"token": token}
            });
            if *method == "browser.goto" {
                params["url"] = json!("https://example.com/next");
            }
            let req = json!({
                "id": 40 + idx as i64,
                "method": method,
                "params": params
            })
            .to_string();
            let out: Value =
                serde_json::from_str(&server.handle_json_line("c", &req).await).expect("json");
            assert!(out.get("result").is_some());
        }

        let screenshot = json!({
            "id": 70,
            "method":"browser.screenshot",
            "params":{
                "command_id":"cmd-bm-shot",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "tab_id": tab,
                "auth":{"token": token},
                "expected_bytes": 512
            }
        })
        .to_string();
        let screenshot_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &screenshot).await).expect("json");
        assert!(screenshot_out.get("result").is_some());

        let download = json!({
            "id": 71,
            "method":"browser.download",
            "params":{
                "command_id":"cmd-bm-download",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "tab_id": tab,
                "auth":{"token": token},
                "size_bytes": 1024
            }
        })
        .to_string();
        let download_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &download).await).expect("json");
        assert!(download_out.get("result").is_some());

        let subscribe = json!({
            "id": 72,
            "method":"browser.subscribe",
            "params":{
                "command_id":"cmd-bm-sub",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "auth":{"token": token}
            }
        })
        .to_string();
        let subscribe_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &subscribe).await).expect("json");
        assert!(subscribe_out.get("result").is_some());

        let raw = json!({
            "id": 73,
            "method":"browser.raw.command",
            "params":{
                "command_id":"cmd-bm-raw",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "tab_id": tab,
                "auth":{"token": token},
                "allow_raw": true,
                "raw_command":"Runtime.evaluate"
            }
        })
        .to_string();
        let raw_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &raw).await).expect("json");
        assert!(raw_out.get("result").is_some());

        let tab_close = json!({
            "id": 74,
            "method":"browser.tab.close",
            "params":{
                "command_id":"cmd-bm-tab-close",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "tab_id": tab,
                "auth":{"token": token}
            }
        })
        .to_string();
        let tab_close_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &tab_close).await).expect("json");
        assert!(tab_close_out.get("result").is_some());

        let close = json!({
            "id": 75,
            "method":"browser.close",
            "params":{
                "command_id":"cmd-bm-close",
                "workspace_id":"ws-1",
                "surface_id":"sf-1",
                "browser_session_id": bs,
                "auth":{"token": token}
            }
        })
        .to_string();
        let close_out: Value =
            serde_json::from_str(&server.handle_json_line("c", &close).await).expect("json");
        assert!(close_out.get("result").is_some());
    }

    #[test]
    fn browser_helper_encoders_and_url_parsing_work() {
        let encoded = base64_encode(b"hello");
        assert_eq!(encoded, "aGVsbG8=");
        assert_eq!(base64_decode(&encoded).expect("decode"), b"hello");
        assert!(base64_decode("%%%").is_err());

        let (host, port, path) =
            parse_url_parts("ws://127.0.0.1:9222/devtools/page/1", "ws").expect("parts");
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 9222);
        assert_eq!(path, "/devtools/page/1");
    }

    #[test]
    fn browser_http_request_and_target_list_work() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut request = [0_u8; 2048];
                let read = stream.read(&mut request).expect("read");
                let body = if String::from_utf8_lossy(&request[..read]).contains("/json/list") {
                    r#"[{"id":"target-1","type":"page","title":"Hello","url":"https://example.com","webSocketDebuggerUrl":"ws://127.0.0.1:9222/devtools/page/1"}]"#
                } else {
                    "{}"
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).expect("write");
            }
        });

        let body = browser_http_request("GET", &format!("http://127.0.0.1:{port}/json/list"))
            .expect("http body");
        assert!(body.contains("target-1"));

        let child = StdCommand::new(if cfg!(windows) { "cmd" } else { "sh" })
            .args(if cfg!(windows) {
                vec!["/C", "exit", "0"]
            } else {
                vec!["-c", "exit 0"]
            })
            .spawn()
            .expect("child");
        let process = BrowserProcessRuntime {
            runtime: "chromium-cdp".to_string(),
            executable: "fake".to_string(),
            port,
            http_base_url: format!("http://127.0.0.1:{port}"),
            websocket_url: "ws://127.0.0.1:9222/devtools/browser/1".to_string(),
            user_data_dir: std::env::temp_dir(),
            artifact_dir: std::env::temp_dir(),
            child: StdMutex::new(child),
        };
        let targets = browser_target_list(&process).expect("targets");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].title, "Hello");
        handle.join().expect("server");
    }

    #[test]
    fn websocket_and_cdp_command_work() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).expect("read");
            assert!(String::from_utf8_lossy(&request[..read]).contains("Upgrade: websocket"));
            stream
                .write_all(
                    b"HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: ignored\r\n\r\n",
                )
                .expect("handshake");

            let mut header = [0_u8; 2];
            stream.read_exact(&mut header).expect("frame header");
            let masked = (header[1] & 0x80) != 0;
            assert!(masked);
            let mut payload_len = (header[1] & 0x7F) as usize;
            if payload_len == 126 {
                let mut ext = [0_u8; 2];
                stream.read_exact(&mut ext).expect("ext");
                payload_len = u16::from_be_bytes(ext) as usize;
            }
            let mut mask = [0_u8; 4];
            stream.read_exact(&mut mask).expect("mask");
            let mut payload = vec![0_u8; payload_len];
            stream.read_exact(&mut payload).expect("payload");
            for (idx, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[idx % 4];
            }
            let text = String::from_utf8(payload).expect("utf8");
            assert!(text.contains("\"method\":\"Runtime.evaluate\""));

            let response = json!({"id":1,"result":{"value":"ok"}}).to_string();
            let bytes = response.as_bytes();
            let mut frame = vec![0x81];
            frame.push(bytes.len() as u8);
            frame.extend_from_slice(bytes);
            stream.write_all(&frame).expect("write response");
        });

        let mut connection =
            CdpConnection::connect(&format!("ws://127.0.0.1:{port}/devtools/page/1"))
                .expect("connect");
        let result = connection
            .command("Runtime.evaluate", json!({"expression":"1+1"}))
            .expect("command");
        assert_eq!(result["value"], "ok");
        handle.join().expect("server");
    }

    #[test]
    fn browser_artifact_writer_and_readiness_checks_work() {
        let default_config = BackendConfig::default();
        assert_eq!(
            browser_dependency_ready(&default_config),
            !browser_launch_targets(&default_config).is_empty()
        );

        let child = StdCommand::new(if cfg!(windows) { "cmd" } else { "sh" })
            .args(if cfg!(windows) {
                vec!["/C", "exit", "0"]
            } else {
                vec!["-c", "exit 0"]
            })
            .spawn()
            .expect("child");
        let artifact_dir = std::env::temp_dir().join(format!("maxc-artifacts-{}", now_unix_ms()));
        let process = BrowserProcessRuntime {
            runtime: "chromium-cdp".to_string(),
            executable: "fake".to_string(),
            port: 9222,
            http_base_url: "http://127.0.0.1:9222".to_string(),
            websocket_url: "ws://127.0.0.1:9222/devtools/browser/1".to_string(),
            user_data_dir: std::env::temp_dir(),
            artifact_dir: artifact_dir.clone(),
            child: StdMutex::new(child),
        };
        let path =
            write_browser_artifact(&process, "tab-1", "shot", "txt", b"hello").expect("artifact");
        assert!(std::path::Path::new(&path).exists());
        let _ = fs::remove_dir_all(artifact_dir);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn terminal_input_preserves_raw_bytes() {
        let path = std::env::temp_dir().join(format!("maxc-terminal-input-{}.txt", now_unix_ms()));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .expect("open");
        let written = write_to_terminal_input(
            TerminalInputHandle::BlockingPipe(Arc::new(StdMutex::new(file))),
            "\u{3}\u{1b}[A",
        )
        .await
        .expect("write");
        assert_eq!(written, "\u{3}\u{1b}[A".len());
        let content = fs::read(&path).expect("read");
        assert_eq!(content, b"\x03\x1b[A");
        let _ = fs::remove_file(path);
    }

    #[tokio::test]
    async fn terminal_output_task_emits_partial_chunks_without_newline() {
        let server = RpcServer::new(test_config("terminal-chunks")).expect("server");
        let terminal_session_id = "ts-chunk".to_string();
        {
            let mut runtime = server.state.terminal_runtime.lock().await;
            runtime.insert(
                terminal_session_id.clone(),
                TerminalSessionRuntime {
                    workspace_id: "ws-1".to_string(),
                    surface_id: "sf-1".to_string(),
                    cols: 80,
                    rows: 24,
                    alive: true,
                    last_output: String::new(),
                    program: "test".to_string(),
                    cwd: ".".to_string(),
                    pid: 1,
                    runtime: "process-stdio".to_string(),
                    status: "ready".to_string(),
                    exit_code: None,
                    input: None,
                    kill_tx: None,
                    next_sequence: 1,
                    history: VecDeque::new(),
                    history_bytes: 0,
                    #[cfg(windows)]
                    conpty: None,
                },
            );
        }
        let (mut writer, reader) = tokio::io::duplex(64);
        spawn_terminal_output_task(
            server.state.clone(),
            server.config.queue_limit.max(1),
            terminal_session_id.clone(),
            reader,
            "stdout",
        );
        writer
            .write_all(b"prompt> \rprogress 10%")
            .await
            .expect("write");
        drop(writer);
        tokio::time::sleep(Duration::from_millis(50)).await;
        let runtime = server.state.terminal_runtime.lock().await;
        let session = runtime.get(&terminal_session_id).expect("session");
        assert_eq!(session.last_output, "prompt> \rprogress 10%");
        let event = session.history.back().expect("event");
        assert_eq!(event["type"], "terminal.output");
        assert_eq!(event["output"], "prompt> \rprogress 10%");
    }

    #[cfg(windows)]
    #[test]
    fn conpty_creation_flags_include_unicode_env_when_needed() {
        let with_env = conpty_creation_flags(true);
        let without_env = conpty_creation_flags(false);
        assert_ne!(with_env, without_env);
        assert_eq!(
            with_env & windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT,
            windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT
        );
        assert_eq!(
            without_env & windows_sys::Win32::System::Threading::CREATE_UNICODE_ENVIRONMENT,
            0
        );
    }

    #[test]
    fn cleanup_failed_browser_launch_removes_runtime_state() {
        let root = std::env::temp_dir().join(format!("maxc-browser-cleanup-{}", now_unix_ms()));
        let user_data_dir = root.join("runtime");
        let artifact_dir = root.join("artifacts");
        fs::create_dir_all(&user_data_dir).expect("runtime dir");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        let child = StdCommand::new(if cfg!(windows) { "cmd" } else { "sh" })
            .args(if cfg!(windows) {
                vec!["/C", "ping", "127.0.0.1", "-n", "6", ">NUL"]
            } else {
                vec!["-c", "sleep 5"]
            })
            .spawn()
            .expect("child");
        cleanup_failed_browser_launch(child, &user_data_dir, &artifact_dir);
        assert!(!user_data_dir.exists());
        assert!(!artifact_dir.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn browser_launch_target_resolution_prefers_chromium_then_webview2() {
        let temp_executable =
            std::env::temp_dir().join(format!("maxc-browser-bin-{}.exe", now_unix_ms()));
        fs::write(&temp_executable, b"stub").expect("stub executable");
        let config = BackendConfig {
            browser_executable_or_channel: temp_executable.to_string_lossy().to_string(),
            ..BackendConfig::default()
        };
        let targets = browser_launch_targets(&config);
        assert_eq!(
            targets.first().map(|target| target.runtime.as_str()),
            Some("chromium-cdp")
        );
        if resolve_webview2_executable().is_ok() {
            assert!(targets.iter().any(|target| target.runtime == "webview2"));
        }
        let _ = fs::remove_file(temp_executable);
    }

    #[tokio::test]
    async fn browser_history_returns_sequence_buffer() {
        let cfg = BackendConfig {
            browser_executable_or_channel: "__synthetic__".to_string(),
            ..test_config("browser-history")
        };
        let server = RpcServer::new(cfg).expect("server");
        let auth: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-auth",
                    &json!({"id":1,"method":"session.create","params":{"command_id":"cmd-auth-browser-history"}}).to_string(),
                )
                .await,
        )
        .expect("auth");
        let token = auth["result"]["token"].as_str().expect("token");

        let created: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-browser",
                    &json!({
                        "id":2,
                        "method":"browser.create",
                        "params":{
                            "command_id":"cmd-browser-history-create",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("browser create");
        let browser_session_id = created["result"]["browser_session_id"]
            .as_str()
            .expect("browser session");

        let _ = server
            .handle_json_line(
                "conn-browser",
                &json!({
                    "id":3,
                    "method":"browser.subscribe",
                    "params":{
                        "command_id":"cmd-browser-subscribe",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id":browser_session_id,
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;

        let opened: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-browser",
                    &json!({
                        "id":4,
                        "method":"browser.tab.open",
                        "params":{
                            "command_id":"cmd-browser-history-tab",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id":browser_session_id,
                            "url":"https://example.com",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("tab open");
        let tab_id = opened["result"]["browser_tab_id"].as_str().expect("tab id");

        let _ = server
            .handle_json_line(
                "conn-browser",
                &json!({
                    "id":5,
                    "method":"browser.goto",
                    "params":{
                        "command_id":"cmd-browser-history-goto",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id":browser_session_id,
                        "tab_id":tab_id,
                        "url":"https://example.com/next",
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;

        let history: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-browser",
                    &json!({
                        "id":6,
                        "method":"browser.history",
                        "params":{
                            "command_id":"cmd-browser-history-read",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id":browser_session_id,
                            "from_sequence":1,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("browser history");
        assert!(
            history["result"]["last_sequence"]
                .as_u64()
                .unwrap_or_default()
                >= 2
        );
        assert!(history["result"]["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["type"] == "browser.navigation"));
        let _ = server
            .handle_json_line(
                "conn-browser",
                &json!({
                    "id":7,
                    "method":"browser.close",
                    "params":{
                        "command_id":"cmd-browser-history-close",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "browser_session_id":browser_session_id,
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;
    }

    #[tokio::test]
    async fn agent_worker_and_task_flow_is_supported() {
        let cfg = test_config("agent-flow");
        let server = RpcServer::new(cfg).expect("server");
        let auth: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-auth",
                    &json!({"id":1,"method":"session.create","params":{"command_id":"cmd-auth-agent"}}).to_string(),
                )
                .await,
        )
        .expect("auth");
        let token = auth["result"]["token"].as_str().expect("token");

        let worker: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":2,
                        "method":"agent.worker.create",
                        "params":{
                            "command_id":"cmd-agent-worker-create",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("worker create");
        let worker_id = worker["result"]["agent_worker_id"]
            .as_str()
            .expect("worker id");

        let task: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":3,
                        "method":"agent.task.start",
                        "params":{
                            "command_id":"cmd-agent-task-start",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "prompt":"echo phase9-phase10",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("task start");
        let task_id = task["result"]["agent_task_id"].as_str().expect("task id");

        let fetched: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":4,
                        "method":"agent.task.get",
                        "params":{
                            "command_id":"cmd-agent-task-get",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "agent_task_id":task_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("task get");
        assert_eq!(fetched["result"]["status"], "running");

        let cancelled: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":5,
                        "method":"agent.task.cancel",
                        "params":{
                            "command_id":"cmd-agent-task-cancel",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_task_id":task_id,
                            "reason":"test cancel",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("task cancel");
        assert_eq!(cancelled["result"]["cancelled"], true);
        let closed: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":6,
                        "method":"agent.worker.close",
                        "params":{
                            "command_id":"cmd-agent-worker-close",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("worker close");
        assert_eq!(closed["result"]["closed"], true);
    }

    #[tokio::test]
    async fn agent_worker_attach_list_get_detach_and_close_are_supported() {
        let cfg = BackendConfig {
            browser_executable_or_channel: "__synthetic__".to_string(),
            ..test_config("agent-lifecycle")
        };
        let server = RpcServer::new(cfg).expect("server");
        let auth: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-auth",
                    &json!({"id":1,"method":"session.create","params":{"command_id":"cmd-auth-agent-lifecycle"}}).to_string(),
                )
                .await,
        )
        .expect("auth");
        let token = auth["result"]["token"].as_str().expect("token");

        let worker: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":2,
                        "method":"agent.worker.create",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-create",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("worker create");
        let worker_id = worker["result"]["agent_worker_id"]
            .as_str()
            .expect("worker id");
        let terminal_session_id = worker["result"]["terminal_session_id"]
            .as_str()
            .expect("terminal session")
            .to_string();

        let browser: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":3,
                        "method":"browser.create",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-browser",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("browser create");
        let browser_session_id = browser["result"]["browser_session_id"]
            .as_str()
            .expect("browser session")
            .to_string();

        let attach_browser: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":4,
                        "method":"agent.attach.browser",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-attach-browser",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "browser_session_id":browser_session_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("attach browser");
        assert_eq!(attach_browser["result"]["attached"], true);

        let detach_terminal: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":5,
                        "method":"agent.detach.terminal",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-detach-terminal",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("detach terminal");
        assert_eq!(detach_terminal["result"]["attached"], false);

        let attach_terminal: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":6,
                        "method":"agent.attach.terminal",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-attach-terminal",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "terminal_session_id":terminal_session_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("attach terminal");
        assert_eq!(attach_terminal["result"]["attached"], true);

        let listed: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":7,
                        "method":"agent.worker.list",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-list",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("worker list");
        assert_eq!(listed["result"]["workers"][0]["agent_worker_id"], worker_id);

        let fetched: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":8,
                        "method":"agent.worker.get",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-get",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("worker get");
        assert_eq!(fetched["result"]["browser_session_id"], browser_session_id);

        let detach_browser: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":9,
                        "method":"agent.detach.browser",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-detach-browser",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("detach browser");
        assert_eq!(detach_browser["result"]["attached"], false);

        let closed: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-agent",
                    &json!({
                        "id":10,
                        "method":"agent.worker.close",
                        "params":{
                            "command_id":"cmd-agent-lifecycle-close",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id":worker_id,
                            "auth":{"token":token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("worker close");
        assert_eq!(closed["result"]["closed"], true);
    }

    #[tokio::test]
    async fn browser_attachment_conflicts_across_workers() {
        let cfg = BackendConfig {
            browser_executable_or_channel: "__synthetic__".to_string(),
            ..test_config("agent-browser-conflict")
        };
        let server = RpcServer::new(cfg).expect("server");
        let auth: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn-auth",
                    &json!({"id":1,"method":"session.create","params":{"command_id":"cmd-auth-agent-browser"}}).to_string(),
                )
                .await,
        )
        .expect("auth");
        let token = auth["result"]["token"].as_str().expect("token");
        let browser_session_id = "bs-test".to_string();
        {
            let mut browser_runtime = server.state.browser_runtime.lock().await;
            browser_runtime.insert(
                browser_session_id.clone(),
                BrowserSessionRuntime {
                    workspace_id: "ws-1".to_string(),
                    surface_id: "sf-1".to_string(),
                    attached: true,
                    closed: false,
                    status: "ready".to_string(),
                    tabs: HashMap::new(),
                    tracing_enabled: false,
                    network_interception: false,
                    runtime: "browser-simulated".to_string(),
                    executable: "synthetic".to_string(),
                    last_error: None,
                    process: None,
                    next_sequence: 1,
                    history: VecDeque::new(),
                    history_bytes: 0,
                },
            );
        }
        {
            let mut workers = server.state.agent_workers.lock().await;
            workers.insert(
                "aw-a".to_string(),
                AgentWorkerRuntime {
                    workspace_id: "ws-1".to_string(),
                    surface_id: "sf-1".to_string(),
                    status: "ready".to_string(),
                    terminal_session_id: "ts-a".to_string(),
                    browser_session_id: None,
                    current_task_id: None,
                    closed: false,
                },
            );
            workers.insert(
                "aw-b".to_string(),
                AgentWorkerRuntime {
                    workspace_id: "ws-1".to_string(),
                    surface_id: "sf-1".to_string(),
                    status: "ready".to_string(),
                    terminal_session_id: "ts-b".to_string(),
                    browser_session_id: None,
                    current_task_id: None,
                    closed: false,
                },
            );
        }

        let _ = server
            .handle_json_line(
                "conn-agent",
                &json!({
                    "id":5,
                    "method":"agent.attach.browser",
                    "params":{
                        "command_id":"cmd-agent-attach-browser-a",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "agent_worker_id":"aw-a",
                        "browser_session_id":browser_session_id,
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;

        let second = server
            .handle_json_line(
                "conn-agent",
                &json!({
                    "id":6,
                    "method":"agent.attach.browser",
                    "params":{
                        "command_id":"cmd-agent-attach-browser-b",
                        "workspace_id":"ws-1",
                        "surface_id":"sf-1",
                        "agent_worker_id":"aw-b",
                        "browser_session_id":browser_session_id,
                        "auth":{"token":token}
                    }
                })
                .to_string(),
            )
            .await;
        let parsed: Value = serde_json::from_str(&second).expect("json");
        assert_eq!(parsed["error"]["code"], "CONFLICT");
    }

    #[tokio::test]
    async fn session_revoke_and_agent_task_list_are_supported() {
        let server = RpcServer::new(test_config("session-revoke-agent-list")).expect("server");

        let created: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 1,
                        "method": "session.create",
                        "params": {"command_id":"cmd-session-create"}
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        let token = created["result"]["token"]
            .as_str()
            .expect("token")
            .to_string();

        let worker: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 2,
                        "method": "agent.worker.create",
                        "params": {
                            "command_id":"cmd-worker-create",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token": token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        let worker_id = worker["result"]["agent_worker_id"]
            .as_str()
            .expect("worker id");

        let task: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 3,
                        "method": "agent.task.start",
                        "params": {
                            "command_id":"cmd-task-start",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "agent_worker_id": worker_id,
                            "prompt":"echo hi",
                            "auth":{"token": created["result"]["token"].as_str().expect("token")}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        assert!(task.get("result").is_some());

        let task_list: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 4,
                        "method": "agent.task.list",
                        "params": {
                            "command_id":"cmd-task-list",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token": created["result"]["token"].as_str().expect("token")}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        assert_eq!(
            task_list["result"]["tasks"].as_array().map(|v| v.len()),
            Some(1)
        );

        let revoke: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 5,
                        "method": "session.revoke",
                        "params": {
                            "command_id":"cmd-session-revoke",
                            "auth":{"token": created["result"]["token"].as_str().expect("token")}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        assert_eq!(revoke["result"]["revoked"], true);
    }

    #[tokio::test]
    async fn browser_tab_list_focus_and_close_work() {
        let server = RpcServer::new(test_config("browser-list-focus-close")).expect("server");
        let session: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 1,
                        "method": "session.create",
                        "params": {"command_id":"cmd-auth-browser-list"}
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        let token = session["result"]["token"].as_str().expect("token");

        let browser: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 2,
                        "method": "browser.create",
                        "params": {
                            "command_id":"cmd-browser-create",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "auth":{"token": token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        let browser_session_id = browser["result"]["browser_session_id"]
            .as_str()
            .expect("browser session");

        let tab: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 3,
                        "method": "browser.tab.open",
                        "params": {
                            "command_id":"cmd-browser-tab-open",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id": browser_session_id,
                            "auth":{"token": token},
                            "url":"https://example.com"
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        let tab_id = tab["result"]["browser_tab_id"].as_str().expect("tab id");

        let list: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 4,
                        "method": "browser.tab.list",
                        "params": {
                            "command_id":"cmd-browser-tab-list",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id": browser_session_id,
                            "auth":{"token": token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        assert_eq!(list["result"]["tabs"].as_array().map(|v| v.len()), Some(1));

        let focus: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 5,
                        "method": "browser.tab.focus",
                        "params": {
                            "command_id":"cmd-browser-tab-focus",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id": browser_session_id,
                            "tab_id": tab_id,
                            "auth":{"token": token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        assert_eq!(focus["result"]["focused"], true);

        let close: Value = serde_json::from_str(
            &server
                .handle_json_line(
                    "conn",
                    &json!({
                        "id": 6,
                        "method": "browser.tab.close",
                        "params": {
                            "command_id":"cmd-browser-tab-close",
                            "workspace_id":"ws-1",
                            "surface_id":"sf-1",
                            "browser_session_id": browser_session_id,
                            "tab_id": tab_id,
                            "auth":{"token": token}
                        }
                    })
                    .to_string(),
                )
                .await,
        )
        .expect("json");
        assert_eq!(close["result"]["closed"], true);
    }
}
