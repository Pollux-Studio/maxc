use serde::{Deserialize, Serialize};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_context() {
        let ctx = TelemetryContext::new("corr-123");
        assert_eq!(ctx.correlation_id, "corr-123");
    }
}
