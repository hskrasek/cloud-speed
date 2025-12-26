//! Retry logic with exponential backoff for network operations.
//!
//! This module provides utilities for retrying failed network operations
//! with configurable retry counts and exponential backoff delays.

use log::{debug, warn};
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

/// Default number of retry attempts.
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Default base delay for exponential backoff (in milliseconds).
pub const DEFAULT_BASE_DELAY_MS: u64 = 100;

/// Maximum delay cap for exponential backoff (in milliseconds).
pub const DEFAULT_MAX_DELAY_MS: u64 = 5000;

/// Configuration for retry behavior.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not including the initial attempt).
    pub max_retries: u32,
    /// Base delay for exponential backoff in milliseconds.
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds.
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_ms: DEFAULT_BASE_DELAY_MS,
            max_delay_ms: DEFAULT_MAX_DELAY_MS,
        }
    }
}

impl RetryConfig {
    /// Create a new retry configuration.
    pub fn new(
        max_retries: u32,
        base_delay_ms: u64,
        max_delay_ms: u64,
    ) -> Self {
        Self { max_retries, base_delay_ms, max_delay_ms }
    }

    /// Calculate the delay for a given attempt number using exponential backoff.
    ///
    /// The delay is calculated as: base_delay * 2^attempt, capped at max_delay.
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms =
            self.base_delay_ms.saturating_mul(2u64.saturating_pow(attempt));
        let capped_delay_ms = delay_ms.min(self.max_delay_ms);
        Duration::from_millis(capped_delay_ms)
    }
}

/// Error that wraps the last error from a series of retry attempts.
#[derive(Debug)]
pub struct RetryError {
    /// The last error that occurred.
    pub last_error: Box<dyn Error + Send + Sync>,
    /// Number of attempts made.
    pub attempts: u32,
    /// Description of the operation that failed.
    pub operation: String,
}

impl fmt::Display for RetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} failed after {} attempts: {}",
            self.operation, self.attempts, self.last_error
        )
    }
}

impl Error for RetryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.last_error.as_ref())
    }
}

/// Result of a retry operation.
#[derive(Debug)]
pub enum RetryResult<T> {
    /// Operation succeeded.
    Success(T),
    /// Operation failed after all retries.
    Failed {
        /// The last error that occurred.
        last_error: Box<dyn Error + Send + Sync>,
        /// Number of attempts made.
        attempts: u32,
    },
}

impl<T> RetryResult<T> {
    /// Returns true if the operation succeeded.
    pub fn is_success(&self) -> bool {
        matches!(self, RetryResult::Success(_))
    }

    /// Returns true if the operation failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, RetryResult::Failed { .. })
    }

    /// Converts to Option, discarding error information.
    pub fn ok(self) -> Option<T> {
        match self {
            RetryResult::Success(v) => Some(v),
            RetryResult::Failed { .. } => None,
        }
    }

    /// Converts to Result with a RetryError.
    pub fn into_result(self, operation: &str) -> Result<T, RetryError> {
        match self {
            RetryResult::Success(v) => Ok(v),
            RetryResult::Failed { last_error, attempts } => Err(RetryError {
                last_error,
                attempts,
                operation: operation.to_string(),
            }),
        }
    }
}

/// Execute an async operation with retry logic and exponential backoff.
///
/// # Arguments
/// * `config` - Retry configuration
/// * `operation_name` - Name of the operation for logging
/// * `f` - Async function that returns a Result
///
/// # Returns
/// RetryResult indicating success or failure with attempt count
///
/// # Example
/// ```no_run
/// use cloud_speed::retry::{retry_async, RetryConfig};
///
/// async fn example() {
///     let config = RetryConfig::default();
///     let result = retry_async(&config, "download test", || async {
///         // Your async operation here
///         Ok::<_, Box<dyn std::error::Error + Send + Sync>>(42)
///     }).await;
/// }
/// ```
pub async fn retry_async<T, E, F, Fut>(
    config: &RetryConfig,
    operation_name: &str,
    mut f: F,
) -> RetryResult<T>
where
    E: Error + Send + Sync + 'static,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    let mut last_error: Option<Box<dyn Error + Send + Sync>> = None;
    let total_attempts = config.max_retries + 1;

    for attempt in 0..total_attempts {
        if attempt > 0 {
            let delay = config.delay_for_attempt(attempt - 1);
            debug!(
                "{}: Retry attempt {}/{} after {:?} delay",
                operation_name, attempt, config.max_retries, delay
            );
            sleep(delay).await;
        }

        match f().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        "{}: Succeeded on attempt {}",
                        operation_name,
                        attempt + 1
                    );
                }
                return RetryResult::Success(result);
            }
            Err(e) => {
                let error_msg = e.to_string();
                last_error = Some(Box::new(e));

                if attempt < config.max_retries {
                    warn!(
                        "{}: Attempt {} failed: {}",
                        operation_name,
                        attempt + 1,
                        error_msg
                    );
                } else {
                    warn!(
                        "{}: All {} attempts failed. Last error: {}",
                        operation_name, total_attempts, error_msg
                    );
                }
            }
        }
    }

    RetryResult::Failed {
        last_error: last_error.unwrap(),
        attempts: total_attempts,
    }
}

/// Check if an error is retryable (network-related).
///
/// This function determines if an error is likely to be transient
/// and worth retrying, such as connection timeouts or temporary
/// network issues.
pub fn is_retryable_error(error: &dyn Error) -> bool {
    let error_str = error.to_string().to_lowercase();

    // Common retryable error patterns
    let retryable_patterns = [
        "connection refused",
        "connection reset",
        "connection timed out",
        "timeout",
        "temporarily unavailable",
        "network unreachable",
        "host unreachable",
        "no route to host",
        "broken pipe",
        "connection aborted",
        "would block",
        "try again",
        "interrupted",
        "dns",
        "resolve",
    ];

    retryable_patterns.iter().any(|pattern| error_str.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(config.base_delay_ms, DEFAULT_BASE_DELAY_MS);
        assert_eq!(config.max_delay_ms, DEFAULT_MAX_DELAY_MS);
    }

    #[test]
    fn test_delay_for_attempt_exponential() {
        let config = RetryConfig::new(3, 100, 5000);

        // Attempt 0: 100 * 2^0 = 100ms
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(100));
        // Attempt 1: 100 * 2^1 = 200ms
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(200));
        // Attempt 2: 100 * 2^2 = 400ms
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(400));
        // Attempt 3: 100 * 2^3 = 800ms
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(800));
    }

    #[test]
    fn test_delay_for_attempt_capped() {
        let config = RetryConfig::new(10, 100, 500);

        // Attempt 5: 100 * 2^5 = 3200ms, but capped at 500ms
        assert_eq!(config.delay_for_attempt(5), Duration::from_millis(500));
    }

    #[test]
    fn test_retry_result_is_success() {
        let success: RetryResult<i32> = RetryResult::Success(42);
        assert!(success.is_success());
        assert!(!success.is_failed());
    }

    #[test]
    fn test_retry_result_is_failed() {
        let failed: RetryResult<i32> = RetryResult::Failed {
            last_error: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "test error",
            )),
            attempts: 3,
        };
        assert!(!failed.is_success());
        assert!(failed.is_failed());
    }

    #[test]
    fn test_retry_result_ok() {
        let success: RetryResult<i32> = RetryResult::Success(42);
        assert_eq!(success.ok(), Some(42));

        let failed: RetryResult<i32> = RetryResult::Failed {
            last_error: Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "test error",
            )),
            attempts: 3,
        };
        assert_eq!(failed.ok(), None);
    }

    #[test]
    fn test_retry_error_display() {
        let error = RetryError {
            last_error: Box::new(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "connection refused",
            )),
            attempts: 3,
            operation: "download test".to_string(),
        };

        let display = format!("{}", error);
        assert!(display.contains("download test"));
        assert!(display.contains("3 attempts"));
        assert!(display.contains("connection refused"));
    }

    #[test]
    fn test_is_retryable_error() {
        // Retryable errors
        let timeout_err = std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "connection timed out",
        );
        assert!(is_retryable_error(&timeout_err));

        let refused_err = std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        );
        assert!(is_retryable_error(&refused_err));

        // Non-retryable errors
        let perm_err = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied",
        );
        assert!(!is_retryable_error(&perm_err));
    }

    #[tokio::test]
    async fn test_retry_async_success_first_attempt() {
        let config = RetryConfig::new(3, 10, 100);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_async(&config, "test op", || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok::<_, std::io::Error>(42)
            }
        })
        .await;

        assert!(result.is_success());
        assert_eq!(result.ok(), Some(42));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_async_success_after_retries() {
        let config = RetryConfig::new(3, 10, 100);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result = retry_async(&config, "test op", || {
            let counter = counter_clone.clone();
            async move {
                let attempt = counter.fetch_add(1, Ordering::SeqCst);
                if attempt < 2 {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "temporary failure",
                    ))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert!(result.is_success());
        assert_eq!(result.ok(), Some(42));
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_async_all_attempts_fail() {
        let config = RetryConfig::new(2, 10, 100);
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let result: RetryResult<i32> = retry_async(&config, "test op", || {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "persistent failure",
                ))
            }
        })
        .await;

        assert!(result.is_failed());
        // 1 initial + 2 retries = 3 total attempts
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }
}
