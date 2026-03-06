use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

const EVENT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("corrupt record checksum mismatch in segment {segment} line {line}")]
    ChecksumMismatch { segment: u64, line: u64 },
    #[error("invalid filename: {0}")]
    InvalidFilename(String),
    #[error("invalid event payload: {0}")]
    InvalidPayload(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    SessionCreated,
    SessionRefreshed,
    SessionRevoked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_id: String,
    pub event_type: EventType,
    pub aggregate_id: String,
    pub command_id: String,
    pub timestamp_ms: u64,
    pub schema_version: u16,
    pub payload: Value,
}

impl EventRecord {
    pub fn new(
        event_id: impl Into<String>,
        event_type: EventType,
        aggregate_id: impl Into<String>,
        command_id: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            event_id: event_id.into(),
            event_type,
            aggregate_id: aggregate_id.into(),
            command_id: command_id.into(),
            timestamp_ms: now_unix_ms(),
            schema_version: EVENT_SCHEMA_VERSION,
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ReplayCursor {
    pub segment: u64,
    pub line: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionProjection {
    pub token: String,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub last_seen_ms: u64,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectionState {
    pub sessions: HashMap<String, SessionProjection>,
    pub command_results: HashMap<String, Value>,
    pub last_cursor: ReplayCursor,
}

impl ProjectionState {
    pub fn apply(&mut self, record: &EventRecord, cursor: ReplayCursor) -> Result<(), StoreError> {
        match record.event_type {
            EventType::SessionCreated => {
                let token = payload_str(&record.payload, "token")?;
                let issued_at_ms = payload_u64(&record.payload, "issued_at_ms")?;
                let expires_at_ms = payload_u64(&record.payload, "expires_at_ms")?;
                let last_seen_ms = payload_u64(&record.payload, "last_seen_ms")?;
                self.sessions.insert(
                    token.to_string(),
                    SessionProjection {
                        token: token.to_string(),
                        issued_at_ms,
                        expires_at_ms,
                        last_seen_ms,
                        revoked: false,
                    },
                );
            }
            EventType::SessionRefreshed => {
                let token = payload_str(&record.payload, "token")?;
                let expires_at_ms = payload_u64(&record.payload, "expires_at_ms")?;
                let last_seen_ms = payload_u64(&record.payload, "last_seen_ms")?;
                let session = self.sessions.get_mut(token).ok_or_else(|| {
                    StoreError::InvalidPayload("refresh for unknown session".to_string())
                })?;
                session.expires_at_ms = expires_at_ms;
                session.last_seen_ms = last_seen_ms;
            }
            EventType::SessionRevoked => {
                let token = payload_str(&record.payload, "token")?;
                let session = self.sessions.get_mut(token).ok_or_else(|| {
                    StoreError::InvalidPayload("revoke for unknown session".to_string())
                })?;
                session.revoked = true;
            }
        }

        if let Some(result) = record.payload.get("result") {
            self.command_results
                .insert(record.command_id.clone(), result.clone());
        }
        self.last_cursor = cursor;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct EventStoreConfig {
    pub event_dir: PathBuf,
    pub segment_max_bytes: u64,
    pub snapshot_interval_events: u64,
    pub snapshot_retain_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct SnapshotFile {
    schema_version: u16,
    cursor: ReplayCursor,
    projection: ProjectionState,
    event_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredLine {
    record: EventRecord,
    checksum: u32,
}

#[derive(Debug)]
pub struct EventStore {
    config: EventStoreConfig,
    active_segment: u64,
    events_since_snapshot: u64,
}

impl EventStore {
    pub fn new(config: EventStoreConfig) -> Result<Self, StoreError> {
        fs::create_dir_all(&config.event_dir)?;
        let segments = list_segments(&config.event_dir)?;
        let active_segment = segments.last().copied().unwrap_or(0);
        Ok(Self {
            config,
            active_segment,
            events_since_snapshot: 0,
        })
    }

    pub fn recover(&mut self) -> Result<ProjectionState, StoreError> {
        let snapshot = self.load_latest_snapshot()?;
        let mut projection = snapshot
            .as_ref()
            .map(|s| s.projection.clone())
            .unwrap_or_default();
        let cursor = snapshot
            .as_ref()
            .map(|s| s.cursor.clone())
            .unwrap_or_default();
        let replayed = self.replay_from_cursor(&cursor, &mut projection)?;
        self.events_since_snapshot = replayed % self.config.snapshot_interval_events;
        Ok(projection)
    }

    pub fn append(&mut self, record: &EventRecord) -> Result<ReplayCursor, StoreError> {
        let segment = self.pick_segment_for_append(record)?;
        let event_path = segment_path(&self.config.event_dir, segment);
        let index_path = index_path(&self.config.event_dir, segment);
        let mut event_file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&event_path)?;
        let offset = event_file.seek(SeekFrom::End(0))?;

        let record_json = serde_json::to_vec(record)?;
        let stored = StoredLine {
            record: record.clone(),
            checksum: checksum(&record_json),
        };
        let mut line = serde_json::to_vec(&stored)?;
        line.push(b'\n');
        event_file.write_all(&line)?;
        event_file.flush()?;

        let line_no = line_count(&event_path)?;
        let mut idx = OpenOptions::new()
            .create(true)
            .append(true)
            .open(index_path)?;
        writeln!(idx, "{offset}")?;
        idx.flush()?;

        self.active_segment = segment;
        self.events_since_snapshot = self.events_since_snapshot.saturating_add(1);
        Ok(ReplayCursor {
            segment,
            line: line_no,
            offset,
        })
    }

    pub fn maybe_snapshot_and_compact(
        &mut self,
        projection: &ProjectionState,
    ) -> Result<(), StoreError> {
        if self.events_since_snapshot < self.config.snapshot_interval_events {
            return Ok(());
        }
        self.write_snapshot(projection)?;
        self.events_since_snapshot = 0;
        self.compact()?;
        Ok(())
    }

    fn write_snapshot(&self, projection: &ProjectionState) -> Result<(), StoreError> {
        let snap = SnapshotFile {
            schema_version: EVENT_SCHEMA_VERSION,
            cursor: projection.last_cursor.clone(),
            projection: projection.clone(),
            event_count: self.total_event_count()?,
        };
        let path = snapshot_path(&self.config.event_dir, snap.event_count);
        let data = serde_json::to_vec_pretty(&snap)?;
        fs::write(path, data)?;
        Ok(())
    }

    fn compact(&self) -> Result<(), StoreError> {
        let mut snapshots = list_snapshots(&self.config.event_dir)?;
        snapshots.sort_unstable();
        if snapshots.len() <= self.config.snapshot_retain_count {
            return Ok(());
        }

        let remove_count = snapshots.len() - self.config.snapshot_retain_count;
        for count in snapshots.iter().take(remove_count) {
            let path = snapshot_path(&self.config.event_dir, *count);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }

        if let Some(latest_count) = snapshots.last().copied() {
            let latest_snapshot = self.load_snapshot_by_count(latest_count)?;
            if let Some(snapshot) = latest_snapshot {
                let segments = list_segments(&self.config.event_dir)?;
                for segment in segments {
                    if segment < snapshot.cursor.segment {
                        let event_path = segment_path(&self.config.event_dir, segment);
                        let idx_path = index_path(&self.config.event_dir, segment);
                        if event_path.exists() {
                            fs::remove_file(event_path)?;
                        }
                        if idx_path.exists() {
                            fs::remove_file(idx_path)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn pick_segment_for_append(&self, record: &EventRecord) -> Result<u64, StoreError> {
        let current = self.active_segment;
        let path = segment_path(&self.config.event_dir, current);
        if !path.exists() {
            return Ok(current);
        }

        let current_size = fs::metadata(&path)?.len();
        let projected = serde_json::to_vec(&StoredLine {
            record: record.clone(),
            checksum: checksum(&serde_json::to_vec(record)?),
        })?
        .len() as u64
            + 1;

        if current_size + projected > self.config.segment_max_bytes {
            Ok(current + 1)
        } else {
            Ok(current)
        }
    }

    fn load_latest_snapshot(&self) -> Result<Option<SnapshotFile>, StoreError> {
        let mut snapshots = list_snapshots(&self.config.event_dir)?;
        snapshots.sort_unstable();
        if let Some(last) = snapshots.last().copied() {
            self.load_snapshot_by_count(last)
        } else {
            Ok(None)
        }
    }

    fn load_snapshot_by_count(&self, count: u64) -> Result<Option<SnapshotFile>, StoreError> {
        let path = snapshot_path(&self.config.event_dir, count);
        if !path.exists() {
            return Ok(None);
        }
        let mut data = Vec::new();
        let mut file = File::open(path)?;
        file.read_to_end(&mut data)?;
        let snapshot = serde_json::from_slice::<SnapshotFile>(&data)?;
        Ok(Some(snapshot))
    }

    fn replay_from_cursor(
        &self,
        cursor: &ReplayCursor,
        projection: &mut ProjectionState,
    ) -> Result<u64, StoreError> {
        let segments = list_segments(&self.config.event_dir)?;
        let mut replayed = 0_u64;
        for segment in segments {
            if segment < cursor.segment {
                continue;
            }
            let path = segment_path(&self.config.event_dir, segment);
            let file = File::open(path)?;
            for (idx, line) in BufReader::new(file).lines().enumerate() {
                let line_no = idx as u64 + 1;
                if segment == cursor.segment && line_no <= cursor.line {
                    continue;
                }
                let line = line?;
                let stored: StoredLine = serde_json::from_str(&line)?;
                let record_json = serde_json::to_vec(&stored.record)?;
                if checksum(&record_json) != stored.checksum {
                    return Err(StoreError::ChecksumMismatch {
                        segment,
                        line: line_no,
                    });
                }

                let cursor = ReplayCursor {
                    segment,
                    line: line_no,
                    offset: read_index_offset(&self.config.event_dir, segment, line_no)?,
                };
                projection.apply(&stored.record, cursor)?;
                replayed += 1;
            }
        }
        Ok(replayed)
    }

    fn total_event_count(&self) -> Result<u64, StoreError> {
        let segments = list_segments(&self.config.event_dir)?;
        let mut total = 0_u64;
        for segment in segments {
            total += line_count(&segment_path(&self.config.event_dir, segment))?;
        }
        Ok(total)
    }
}

fn payload_str<'a>(payload: &'a Value, key: &str) -> Result<&'a str, StoreError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::InvalidPayload(format!("missing string payload key: {key}")))
}

fn payload_u64(payload: &Value, key: &str) -> Result<u64, StoreError> {
    payload
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| StoreError::InvalidPayload(format!("missing numeric payload key: {key}")))
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0_u32, |acc, b| {
        acc.rotate_left(1).wrapping_add(u32::from(*b))
    })
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_millis(0))
        .as_millis() as u64
}

fn segment_path(base: &Path, segment: u64) -> PathBuf {
    base.join(format!("events-{segment:06}.log"))
}

fn index_path(base: &Path, segment: u64) -> PathBuf {
    base.join(format!("events-{segment:06}.idx"))
}

fn snapshot_path(base: &Path, count: u64) -> PathBuf {
    base.join(format!("snapshot-{count:020}.json"))
}

fn list_segments(base: &Path) -> Result<Vec<u64>, StoreError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("events-") && name.ends_with(".log") {
            let number = name
                .trim_start_matches("events-")
                .trim_end_matches(".log")
                .parse::<u64>()
                .map_err(|_| StoreError::InvalidFilename(name.to_string()))?;
            out.push(number);
        }
    }
    out.sort_unstable();
    Ok(out)
}

fn list_snapshots(base: &Path) -> Result<Vec<u64>, StoreError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("snapshot-") && name.ends_with(".json") {
            let number = name
                .trim_start_matches("snapshot-")
                .trim_end_matches(".json")
                .parse::<u64>()
                .map_err(|_| StoreError::InvalidFilename(name.to_string()))?;
            out.push(number);
        }
    }
    out.sort_unstable();
    Ok(out)
}

fn line_count(path: &Path) -> Result<u64, StoreError> {
    if !path.exists() {
        return Ok(0);
    }
    let file = File::open(path)?;
    let mut count = 0_u64;
    for line in BufReader::new(file).lines() {
        let _ = line?;
        count += 1;
    }
    Ok(count)
}

fn read_index_offset(base: &Path, segment: u64, line_no: u64) -> Result<u64, StoreError> {
    let path = index_path(base, segment);
    if !path.exists() {
        return Ok(0);
    }
    let file = File::open(path)?;
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        if idx as u64 + 1 == line_no {
            let line = line?;
            let offset = line
                .parse::<u64>()
                .map_err(|_| StoreError::InvalidFilename("invalid index offset".to_string()))?;
            return Ok(offset);
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(label: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let millis = now_unix_ms();
        path.push(format!("maxc-{label}-{millis}"));
        path
    }

    #[test]
    fn append_and_recover_roundtrip() {
        let dir = temp_dir("store-roundtrip");
        let cfg = EventStoreConfig {
            event_dir: dir.clone(),
            segment_max_bytes: 1024,
            snapshot_interval_events: 2,
            snapshot_retain_count: 2,
        };
        let mut store = EventStore::new(cfg).expect("store");
        let mut projection = ProjectionState::default();

        let created = EventRecord::new(
            "evt-1",
            EventType::SessionCreated,
            "sess-a",
            "cmd-1",
            json!({
                "token": "tok-a",
                "issued_at_ms": 1,
                "expires_at_ms": 10,
                "last_seen_ms": 1,
                "result": { "token": "tok-a", "issued_at_ms": 1, "expires_at_ms": 10 }
            }),
        );
        let cursor = store.append(&created).expect("append");
        projection.apply(&created, cursor).expect("apply");
        store
            .maybe_snapshot_and_compact(&projection)
            .expect("snapshot maybe");

        let refreshed = EventRecord::new(
            "evt-2",
            EventType::SessionRefreshed,
            "sess-a",
            "cmd-2",
            json!({
                "token": "tok-a",
                "expires_at_ms": 20,
                "last_seen_ms": 2,
                "result": { "token": "tok-a", "expires_at_ms": 20 }
            }),
        );
        let cursor = store.append(&refreshed).expect("append");
        projection.apply(&refreshed, cursor).expect("apply");
        store
            .maybe_snapshot_and_compact(&projection)
            .expect("snapshot maybe");

        let mut recovery_store = EventStore::new(EventStoreConfig {
            event_dir: dir.clone(),
            segment_max_bytes: 1024,
            snapshot_interval_events: 2,
            snapshot_retain_count: 2,
        })
        .expect("store");
        let recovered = recovery_store.recover().expect("recover");
        let session = recovered.sessions.get("tok-a").expect("session");
        assert_eq!(session.expires_at_ms, 20);
        assert!(recovered.command_results.contains_key("cmd-1"));
        assert!(recovered.command_results.contains_key("cmd-2"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detects_checksum_mismatch() {
        let dir = temp_dir("store-corrupt");
        let cfg = EventStoreConfig {
            event_dir: dir.clone(),
            segment_max_bytes: 1024,
            snapshot_interval_events: 10,
            snapshot_retain_count: 2,
        };
        let mut store = EventStore::new(cfg).expect("store");
        let event = EventRecord::new(
            "evt-1",
            EventType::SessionCreated,
            "sess-a",
            "cmd-1",
            json!({
                "token": "tok-a",
                "issued_at_ms": 1,
                "expires_at_ms": 10,
                "last_seen_ms": 1,
                "result": { "token": "tok-a" }
            }),
        );
        let cursor = store.append(&event).expect("append");
        let mut projection = ProjectionState::default();
        projection.apply(&event, cursor).expect("apply");

        let path = segment_path(&dir, 0);
        let mut content = fs::read_to_string(&path).expect("read");
        content = content.replace("tok-a", "tok-x");
        fs::write(&path, content).expect("write");

        let mut recovery_store = EventStore::new(EventStoreConfig {
            event_dir: dir.clone(),
            segment_max_bytes: 1024,
            snapshot_interval_events: 10,
            snapshot_retain_count: 2,
        })
        .expect("store");
        let err = recovery_store.recover().expect_err("must fail");
        match err {
            StoreError::ChecksumMismatch { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn compaction_removes_old_snapshots() {
        let dir = temp_dir("store-compact");
        let cfg = EventStoreConfig {
            event_dir: dir.clone(),
            segment_max_bytes: 128,
            snapshot_interval_events: 1,
            snapshot_retain_count: 2,
        };
        let mut store = EventStore::new(cfg).expect("store");
        let mut projection = ProjectionState::default();

        for idx in 0..5 {
            let event = EventRecord::new(
                format!("evt-{idx}"),
                EventType::SessionCreated,
                format!("sess-{idx}"),
                format!("cmd-{idx}"),
                json!({
                    "token": format!("tok-{idx}"),
                    "issued_at_ms": idx,
                    "expires_at_ms": idx + 10,
                    "last_seen_ms": idx,
                    "result": { "token": format!("tok-{idx}") }
                }),
            );
            let cursor = store.append(&event).expect("append");
            projection.apply(&event, cursor).expect("apply");
            store
                .maybe_snapshot_and_compact(&projection)
                .expect("snapshot");
        }

        let snapshots = list_snapshots(&dir).expect("list");
        assert!(snapshots.len() <= 2);
        let _ = fs::remove_dir_all(dir);
    }
}
