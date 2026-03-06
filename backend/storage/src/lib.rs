use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageHealth {
    pub ready: bool,
}

impl Default for StorageHealth {
    fn default() -> Self {
        Self { ready: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_is_ready_by_default() {
        let health = StorageHealth::default();
        assert!(health.ready);
    }
}
