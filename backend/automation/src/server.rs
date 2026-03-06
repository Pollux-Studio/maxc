use crate::{RpcErrorCode, RpcId, RpcRequest, RpcSuccess};
use maxc_core::{BackendConfig, CommandId};
use maxc_security::SessionToken;
use maxc_storage::{
    EventRecord, EventStore, EventStoreConfig, EventType, ProjectionState, SessionProjection,
    StoreError,
};
use rand::RngCore;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    pub token: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub last_seen_ms: u64,
    pub revoked: bool,
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
    inflight_by_connection: StdMutex<HashMap<String, usize>>,
    correlation: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct RpcServer {
    config: BackendConfig,
    state: Arc<ServerState>,
}

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
                inflight_by_connection: StdMutex::new(HashMap::new()),
                correlation: AtomicU64::new(1),
            }),
        })
    }

    pub async fn handle_json_line(&self, connection_id: &str, line: &str) -> String {
        let corr = self.next_correlation_id();
        let result = self
            .handle_json_line_inner(connection_id, line)
            .await
            .unwrap_or_else(|err| {
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
        let id = request.id.clone().unwrap_or(RpcId::Null);
        let _guard = InflightGuard::acquire(
            Arc::clone(&self.state),
            connection_id,
            self.config.max_inflight_per_connection,
        )?;

        let timeout_duration = Duration::from_millis(self.config.request_timeout_ms);
        let response = timeout(timeout_duration, self.dispatch(request))
            .await
            .map_err(|_| ServerError::Timeout)??;

        serde_json::to_value(RpcSuccess {
            id,
            result: response,
        })
        .map_err(|_| ServerError::Internal)
    }

    async fn check_limits(&self, connection_id: &str) -> Result<(), ServerError> {
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
        match request.method.as_str() {
            "session.create" => self.session_create(request.params).await,
            "session.refresh" => self.session_refresh(request.params).await,
            "session.revoke" => self.session_revoke(request.params).await,
            "system.health" => Ok(json!({ "ok": true })),
            _ => Err(ServerError::NotFound),
        }
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

    async fn persist_and_apply(&self, event: EventRecord) -> Result<Value, ServerError> {
        let mut projection = self.state.projection.lock().await;
        if let Some(existing) = projection.command_results.get(&event.command_id) {
            return Ok(existing.clone());
        }
        let mut store = self.state.store.lock().await;
        let cursor = store.append(&event).map_err(map_store_error)?;
        projection.apply(&event, cursor).map_err(map_store_error)?;
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

    #[cfg(windows)]
    pub async fn serve_named_pipe(&self, pipe_name: &str) -> Result<(), std::io::Error> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::windows::named_pipe::ServerOptions;

        let mut listener = ServerOptions::new().create(pipe_name)?;
        loop {
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
    }

    #[cfg(not(windows))]
    pub async fn serve_named_pipe(&self, _pipe_name: &str) -> Result<(), std::io::Error> {
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
}
