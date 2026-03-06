use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryContext {
    pub correlation_id: String,
}

impl TelemetryContext {
    pub fn new(correlation_id: impl Into<String>) -> Self {
        Self {
            correlation_id: correlation_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogRecord {
    pub timestamp_ms: u64,
    pub level: LogLevel,
    pub component: String,
    pub event: String,
    pub correlation_id: String,
    #[serde(default)]
    pub command_id: Option<String>,
    #[serde(default)]
    pub connection_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub surface_id: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    pub status: String,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanRecord {
    pub name: String,
    pub correlation_id: String,
    pub started_at_ms: u64,
    pub duration_ms: u64,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct LatencySnapshot {
    pub count: u64,
    pub avg_ms: f64,
    pub max_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MetricsSnapshot {
    pub counters: BTreeMap<String, u64>,
    pub gauges: BTreeMap<String, u64>,
    pub latencies: BTreeMap<String, LatencySnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TelemetrySnapshot {
    pub logs: Vec<LogRecord>,
    pub spans: Vec<SpanRecord>,
}

#[derive(Debug, Clone, Default)]
pub struct LatencyMetric {
    samples: VecDeque<f64>,
    total_ms: f64,
    max_ms: f64,
}

impl LatencyMetric {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(256),
            total_ms: 0.0,
            max_ms: 0.0,
        }
    }

    pub fn record(&mut self, value_ms: f64) {
        self.total_ms += value_ms;
        self.max_ms = self.max_ms.max(value_ms);
        if self.samples.len() == 256 {
            self.samples.pop_front();
        }
        self.samples.push_back(value_ms);
    }

    pub fn snapshot(&self) -> LatencySnapshot {
        if self.samples.is_empty() {
            return LatencySnapshot::default();
        }
        let mut ordered: Vec<f64> = self.samples.iter().copied().collect();
        ordered.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = self.samples.len() as u64;
        LatencySnapshot {
            count,
            avg_ms: self.total_ms / count as f64,
            max_ms: self.max_ms,
            p50_ms: percentile(&ordered, 0.50),
            p95_ms: percentile(&ordered, 0.95),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TelemetryCollector {
    logs: VecDeque<LogRecord>,
    spans: VecDeque<SpanRecord>,
    limit: usize,
}

impl TelemetryCollector {
    pub fn new(limit: usize) -> Self {
        Self {
            logs: VecDeque::with_capacity(limit.max(1)),
            spans: VecDeque::with_capacity(limit.max(1)),
            limit: limit.max(1),
        }
    }

    pub fn push_log(&mut self, record: LogRecord) {
        if self.logs.len() == self.limit {
            self.logs.pop_front();
        }
        self.logs.push_back(record);
    }

    pub fn push_span(&mut self, record: SpanRecord) {
        if self.spans.len() == self.limit {
            self.spans.pop_front();
        }
        self.spans.push_back(record);
    }

    pub fn snapshot(&self) -> TelemetrySnapshot {
        TelemetrySnapshot {
            logs: self.logs.iter().cloned().collect(),
            spans: self.spans.iter().cloned().collect(),
        }
    }
}

fn percentile(samples: &[f64], pct: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let idx = ((samples.len() - 1) as f64 * pct).round() as usize;
    samples[idx.min(samples.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_context() {
        let ctx = TelemetryContext::new("corr-123");
        assert_eq!(ctx.correlation_id, "corr-123");
    }

    #[test]
    fn collector_and_latency_snapshot_work() {
        let mut collector = TelemetryCollector::new(2);
        collector.push_log(LogRecord {
            timestamp_ms: 1,
            level: LogLevel::Info,
            component: "rpc".to_string(),
            event: "request.finish".to_string(),
            correlation_id: "corr-1".to_string(),
            command_id: None,
            connection_id: None,
            workspace_id: None,
            surface_id: None,
            method: Some("system.health".to_string()),
            duration_ms: Some(3),
            status: "ok".to_string(),
            fields: BTreeMap::new(),
        });
        collector.push_span(SpanRecord {
            name: "rpc.request".to_string(),
            correlation_id: "corr-1".to_string(),
            started_at_ms: 1,
            duration_ms: 3,
            attributes: BTreeMap::new(),
        });
        assert_eq!(collector.snapshot().logs.len(), 1);

        let mut metric = LatencyMetric::new();
        metric.record(5.0);
        metric.record(10.0);
        let snapshot = metric.snapshot();
        assert_eq!(snapshot.count, 2);
        assert!(snapshot.p95_ms >= 5.0);
    }
}
