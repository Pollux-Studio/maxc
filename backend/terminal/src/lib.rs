use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalCapability {
    Spawn,
    Input,
    Resize,
    Kill,
}

pub fn phase_one_capabilities() -> Vec<TerminalCapability> {
    vec![
        TerminalCapability::Spawn,
        TerminalCapability::Input,
        TerminalCapability::Resize,
        TerminalCapability::Kill,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_list_is_not_empty() {
        assert!(!phase_one_capabilities().is_empty());
    }
}
