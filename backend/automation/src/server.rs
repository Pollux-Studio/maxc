use crate::{RpcErrorCode, RpcId, RpcRequest, RpcSuccess};
use maxc_browser::{BrowserSessionId, BrowserTabId};
use maxc_core::{BackendConfig, CommandId};
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
use std::fs::File as StdFile;
use std::io::{BufRead, BufReader as StdBufReader, Write};
#[cfg(windows)]
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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

impl SessionRecord {
    fn from_projection(value: &SessionProjection) -> Self {
        Self {
            token: value.token.clone(),
            issued_at_ms: value.issued_at_ms,
            expires_at_ms: value.expires_at_ms,
            last_seen_ms: value.last_seen_ms,
            revoked: value.revoked,
        }
    }

    fn is_active(&self, now_ms: u64) -> bool {
        !self.revoked && self.expires_at_ms > now_ms
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
    attached: bool,
    closed: bool,
    tabs: HashMap<String, BrowserTabRuntime>,
    tracing_enabled: bool,
    network_interception: bool,
}

#[derive(Debug, Clone)]
struct BrowserTabRuntime {
    browser_tab_id: String,
    url: String,
    focused: bool,
    closed: bool,
    history: Vec<String>,
    history_index: usize,
    cookies: HashMap<String, String>,
    storage: HashMap<String, String>,
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
        Ok(Self {
            config,
            state: Arc::new(ServerState {
                projection: Mutex::new(projection),
                store: Mutex::new(store),
                global_limiter: Mutex::new(global_limiter),
                connection_limiters: Mutex::new(HashMap::new()),
                raw_limiters: Mutex::new(HashMap::new()),
                terminal_runtime: Mutex::new(HashMap::new()),
                browser_runtime: Mutex::new(HashMap::new()),
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
        })
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
        self.require_active_session(params.as_ref()).await?;
        Ok(json!({
            "ready": !self.is_shutting_down() && !self.breaker_is_open(),
            "accepting_requests": !self.is_shutting_down(),
            "breaker_open": self.breaker_is_open(),
            "queue_saturated": self.active_request_count() >= self.config.overload_reject_threshold,
            "store_available": true
        }))
    }

    async fn system_diagnostics(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session(params.as_ref()).await?;
        let projection = self.state.projection.lock().await;
        let terminal_runtime = self.state.terminal_runtime.lock().await;
        let browser_runtime = self.state.browser_runtime.lock().await;
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
        Ok(json!({
            "sessions": projection.sessions.len(),
            "browser_sessions": projection.browser_sessions.len(),
            "browser_tabs": projection.browser_tabs.len(),
            "terminal_runtime_count": terminal_runtime.len(),
            "browser_runtime_count": browser_runtime.len(),
            "terminal_subscription_count": terminal_subscriptions.values().map(|v| v.len()).sum::<usize>(),
            "browser_subscription_count": browser_subscriptions.values().map(|v| v.len()).sum::<usize>(),
            "terminal_history_events": terminal_runtime.values().map(|session| session.history.len()).sum::<usize>(),
            "terminal_history_bytes": terminal_runtime.values().map(|session| session.history_bytes).sum::<usize>(),
            "terminal_runtime_backend": selected_terminal_runtime_name(&self.config),
            "active_requests": self.active_request_count(),
            "shutting_down": self.is_shutting_down(),
            "breaker_open": self.breaker_is_open(),
            "metrics": metrics
        }))
    }

    async fn system_metrics(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session(params.as_ref()).await?;
        let mut metrics = self.metrics_snapshot();
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
        serde_json::to_value(metrics).map_err(|_| ServerError::Internal)
    }

    async fn system_logs(&self, params: Option<Value>) -> Result<Value, ServerError> {
        self.require_active_session(params.as_ref()).await?;
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
        let result = json!({
            "token": token,
            "issued_at_ms": now,
            "expires_at_ms": now + ttl
        });
        let payload = json!({
            "token": token,
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

        let result = json!({
            "token": token,
            "expires_at_ms": now + ttl
        });
        let payload = json!({
            "token": token,
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
        let token = extract_token(params.as_ref()).ok_or(ServerError::Unauthorized)?;
        let now = now_unix_ms();
        let session = self
            .find_session(&token)
            .await
            .ok_or(ServerError::Unauthorized)?;
        if !session.is_active(now) {
            return Err(ServerError::Unauthorized);
        }

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
            | "browser.trace.stop" => self.browser_automation(command_id, audit, method).await,
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
        let token = extract_token(params.as_ref()).ok_or(ServerError::Unauthorized)?;
        let now = now_unix_ms();
        let session = self
            .find_session(&token)
            .await
            .ok_or(ServerError::Unauthorized)?;
        if !session.is_active(now) {
            return Err(ServerError::Unauthorized);
        }

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
            .ok_or(ServerError::InvalidRequest)?;
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
            .ok_or(ServerError::InvalidRequest)?;
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
            .ok_or(ServerError::InvalidRequest)?;
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
            .ok_or(ServerError::InvalidRequest)?;
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
            .ok_or(ServerError::InvalidRequest)?;
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
        let result = json!({
            "workspace_id": audit.workspace_id,
            "surface_id": audit.surface_id,
            "browser_session_id": browser_session_id
        });
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            runtime.insert(
                browser_session_id.clone(),
                BrowserSessionRuntime {
                    attached: true,
                    closed: false,
                    tabs: HashMap::new(),
                    tracing_enabled: false,
                    network_interception: false,
                },
            );
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.session.created",
                "browser_session_id": browser_session_id,
                "workspace_id": audit.workspace_id,
                "surface_id": audit.surface_id
            }),
        );
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
            .ok_or(ServerError::InvalidRequest)?;
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
        );
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
            .ok_or(ServerError::InvalidRequest)?;
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
        );
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
            .ok_or(ServerError::InvalidRequest)?;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            session.closed = true;
            session.attached = false;
            for tab in session.tabs.values_mut() {
                tab.closed = true;
                tab.focused = false;
            }
        }
        self.cleanup_browser_subscribers(&browser_session_id);
        self.publish_browser_event(
            &browser_session_id,
            json!({"type":"browser.session.closed","browser_session_id": browser_session_id}),
        );
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
            .ok_or(ServerError::InvalidRequest)?;
        let browser_tab_id = format!(
            "tab-{}",
            random_token()?.chars().take(10).collect::<String>()
        );
        let url = params
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("about:blank")
            .to_string();
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            for tab in session.tabs.values_mut() {
                tab.focused = false;
            }
            session.tabs.insert(
                browser_tab_id.clone(),
                BrowserTabRuntime {
                    browser_tab_id: browser_tab_id.clone(),
                    url: url.clone(),
                    focused: true,
                    closed: false,
                    history: vec![url.clone()],
                    history_index: 0,
                    cookies: HashMap::new(),
                    storage: HashMap::new(),
                },
            );
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.tab.opened",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "url": url
            }),
        );
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "url": url
        });
        let payload = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "url": url,
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
            .ok_or(ServerError::InvalidRequest)?;
        let runtime = self.state.browser_runtime.lock().await;
        let session = runtime
            .get(&browser_session_id)
            .ok_or(ServerError::NotFound)?;
        let tabs: Vec<Value> = session
            .tabs
            .values()
            .map(|tab| {
                json!({
                    "browser_tab_id": tab.browser_tab_id,
                    "url": tab.url,
                    "focused": tab.focused,
                    "closed": tab.closed
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
            .ok_or(ServerError::InvalidRequest)?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        {
            let mut runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get_mut(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
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
        );
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
            .ok_or(ServerError::InvalidRequest)?;
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
            tab.closed = true;
            tab.focused = false;
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.tab.closed",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id
            }),
        );
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
            .ok_or(ServerError::InvalidRequest)?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        let requested_url = params
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("about:blank")
            .to_string();
        let url = {
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
            match method {
                "browser.goto" => {
                    tab.history.truncate(tab.history_index.saturating_add(1));
                    tab.history.push(requested_url.clone());
                    tab.history_index = tab.history.len().saturating_sub(1);
                    tab.url = requested_url.clone();
                }
                "browser.reload" => {}
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
            tab.url.clone()
        };
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.navigation",
                "method": method,
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "url": url
            }),
        );
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "method": method,
            "url": url
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
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
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
            match method {
                "browser.cookie.set" => {
                    tab.cookies.insert("session".to_string(), "set".to_string());
                }
                "browser.cookie.get" => {
                    extra = json!({
                        "cookies": tab.cookies
                    });
                }
                "browser.storage.set" => {
                    tab.storage.insert("state".to_string(), "set".to_string());
                }
                "browser.storage.get" => {
                    extra = json!({
                        "storage": tab.storage
                    });
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
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.automation",
                "method": method,
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id
            }),
        );
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "method": method,
            "ok": true,
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
            "subscriber_id": subscriber_id,
            "events": events,
            "dropped_events": dropped_events
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

    async fn browser_raw(
        &self,
        command_id: String,
        audit: BrowserAuditContext,
        params: &Value,
    ) -> Result<Value, ServerError> {
        let browser_session_id = audit
            .browser_session_id
            .ok_or(ServerError::InvalidRequest)?;
        let browser_tab_id = audit.tab_id.ok_or(ServerError::InvalidRequest)?;
        let command = params
            .get("raw_command")
            .and_then(Value::as_str)
            .unwrap_or("noop")
            .to_string();
        {
            let runtime = self.state.browser_runtime.lock().await;
            let session = runtime
                .get(&browser_session_id)
                .ok_or(ServerError::NotFound)?;
            if session.closed {
                return Err(ServerError::Conflict);
            }
            let tab = session
                .tabs
                .get(&browser_tab_id)
                .ok_or(ServerError::NotFound)?;
            if tab.closed {
                return Err(ServerError::Conflict);
            }
        }
        self.publish_browser_event(
            &browser_session_id,
            json!({
                "type":"browser.raw",
                "browser_session_id": browser_session_id,
                "browser_tab_id": browser_tab_id,
                "raw_command": command
            }),
        );
        let result = json!({
            "browser_session_id": browser_session_id,
            "browser_tab_id": browser_tab_id,
            "raw_command": command,
            "ok": true
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

    fn publish_browser_event(&self, browser_session_id: &str, event: Value) {
        let mut all = self
            .state
            .browser_subscriptions
            .lock()
            .expect("subscription lock poisoned");
        if let Some(subs) = all.get_mut(browser_session_id) {
            for sub in subs.values_mut() {
                sub.push(event.clone(), self.config.queue_limit.max(1));
            }
        }
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
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break,
                    Ok(_) => {
                        let trimmed = line.trim_end_matches(&['\r', '\n'][..]).to_string();
                        let state = state.clone();
                        let terminal = terminal_for_reader.clone();
                        runtime.block_on(async move {
                            {
                                let mut runtime = state.terminal_runtime.lock().await;
                                if let Some(session) = runtime.get_mut(&terminal) {
                                    session.last_output = trimmed.clone();
                                }
                            }
                            publish_terminal_event_for_state(
                                &state,
                                queue_limit,
                                history_events,
                                history_bytes,
                                &terminal,
                                json!({
                                    "type": "terminal.output",
                                    "terminal_session_id": terminal,
                                    "stream": "stdout",
                                    "output": trimmed
                                }),
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
        if !self.config.terminal_allowed_programs.is_empty()
            && !self
                .config
                .terminal_allowed_programs
                .iter()
                .any(|program| program.eq_ignore_ascii_case(&launch.program))
        {
            return Err(ServerError::RateLimited);
        }
        if !self.config.terminal_allowed_cwd_roots.is_empty()
            && !self
                .config
                .terminal_allowed_cwd_roots
                .iter()
                .any(|root| launch.cwd.starts_with(root))
        {
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
                fields,
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
            let bytes = if input.ends_with('\n') {
                input.len()
            } else {
                writer
                    .write_all(b"\n")
                    .await
                    .map_err(|_| ServerError::Internal)?;
                input.len() + 1
            };
            writer.flush().await.map_err(|_| ServerError::Internal)?;
            Ok(bytes)
        }
        TerminalInputHandle::BlockingPipe(file) => {
            let input = input.to_string();
            tokio::task::spawn_blocking(move || {
                let mut writer = file.lock().expect("pipe lock");
                writer
                    .write_all(input.as_bytes())
                    .map_err(|_| ServerError::Internal)?;
                let bytes = if input.ends_with('\n') {
                    input.len()
                } else {
                    writer
                        .write_all(b"\r\n")
                        .map_err(|_| ServerError::Internal)?;
                    input.len() + 2
                };
                writer.flush().map_err(|_| ServerError::Internal)?;
                Ok(bytes)
            })
            .await
            .map_err(|_| ServerError::Internal)?
        }
    }
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
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    {
                        let mut runtime = state.terminal_runtime.lock().await;
                        if let Some(session) = runtime.get_mut(&terminal_session_id) {
                            session.last_output = line.clone();
                        }
                    }
                    publish_terminal_event_for_state(
                        &state,
                        queue_limit,
                        queue_limit.saturating_mul(64),
                        queue_limit.saturating_mul(1024),
                        &terminal_session_id,
                        json!({
                            "type": "terminal.output",
                            "terminal_session_id": terminal_session_id,
                            "stream": stream,
                            "output": line
                        }),
                    )
                    .await;
                }
                Ok(None) => break,
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
            EXTENDED_STARTUPINFO_PRESENT,
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
        for _ in 0..20 {
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
        assert_eq!(resize_out["result"]["applied"], cfg!(windows));

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
        assert!(!server.telemetry_snapshot().logs.is_empty());
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
}
