use anyhow::Result;
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

    pub fn run<F, T>(&self, mut op: F) -> Result<T>
    where
        F: FnMut() -> Result<T>,
    {
        let policy = self.retry_policy();
        let mut attempt = 0usize;

        loop {
            match op() {
                Ok(value) => return Ok(value),
                Err(error) if attempt < policy.max_retries => {
                    attempt += 1;
                    std::thread::sleep(policy.delay);
                    if attempt > policy.max_retries {
                        return Err(error);
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }
}
