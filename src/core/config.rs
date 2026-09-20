use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_suggestions: usize,
    pub debounce_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_suggestions: 5,
            debounce_ms: 10,
        }
    }
}

impl Config {
    pub fn new(max_suggestions: usize, debounce_ms: u64) -> Self {
        Self {
            max_suggestions,
            debounce_ms,
        }
    }
}
