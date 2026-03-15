use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionToken(String);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("token cannot be empty")]
    Empty,
}

impl SessionToken {
    pub fn new(value: impl Into<String>) -> Result<Self, TokenError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TokenError::Empty);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_requires_non_empty_input() {
        assert_eq!(SessionToken::new(""), Err(TokenError::Empty));
        assert!(SessionToken::new("token-1").is_ok());
    }
}
