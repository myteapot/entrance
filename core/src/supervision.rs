use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: usize,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            delay: Duration::from_millis(250),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Supervision;

impl Supervision {
    pub fn retry_policy(&self) -> RetryPolicy {
        RetryPolicy::default()
    }
}
