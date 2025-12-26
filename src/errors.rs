//! Custom error types for the speed test application.
//!
//! This module provides user-friendly error types that wrap underlying
//! errors with clear, actionable messages.

use std::error::Error;
use std::fmt;

/// Exit codes for the application.
pub mod exit_codes {
    /// Successful execution.
    pub const SUCCESS: i32 = 0;
    /// Network error (connection failed, timeout, etc.).
    pub const NETWORK_ERROR: i32 = 1;
    /// API error (server returned an error response).
    pub const API_ERROR: i32 = 2;
    /// Configuration error (invalid arguments, missing config).
    pub const CONFIG_ERROR: i32 = 3;
    /// Partial failure (some tests failed but others succeeded).
    pub const PARTIAL_FAILURE: i32 = 4;
    /// Unknown/unexpected error.
    pub const UNKNOWN_ERROR: i32 = 99;
}

/// Categories of errors that can occur during speed testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Network connectivity issues.
    Network,
    /// DNS resolution failures.
    Dns,
    /// Connection timeout.
    Timeout,
    /// TLS/SSL handshake failures.
    Tls,
    /// API returned an error response.
    Api,
    /// Invalid configuration or arguments.
    Config,
    /// Measurement calculation errors.
    Measurement,
    /// Unknown or unexpected errors.
    Unknown,
}

impl ErrorKind {
    /// Get the exit code for this error kind.
    pub fn exit_code(&self) -> i32 {
        match self {
            ErrorKind::Network => exit_codes::NETWORK_ERROR,
            ErrorKind::Dns => exit_codes::NETWORK_ERROR,
            ErrorKind::Timeout => exit_codes::NETWORK_ERROR,
            ErrorKind::Tls => exit_codes::NETWORK_ERROR,
            ErrorKind::Api => exit_codes::API_ERROR,
            ErrorKind::Config => exit_codes::CONFIG_ERROR,
            ErrorKind::Measurement => exit_codes::PARTIAL_FAILURE,
            ErrorKind::Unknown => exit_codes::UNKNOWN_ERROR,
        }
    }

    /// Get a user-friendly description of this error kind.
    pub fn description(&self) -> &'static str {
        match self {
            ErrorKind::Network => "Network error",
            ErrorKind::Dns => "DNS resolution error",
            ErrorKind::Timeout => "Connection timeout",
            ErrorKind::Tls => "TLS/SSL error",
            ErrorKind::Api => "API error",
            ErrorKind::Config => "Configuration error",
            ErrorKind::Measurement => "Measurement error",
            ErrorKind::Unknown => "Unknown error",
        }
    }
}

/// A user-friendly error type for speed test operations.
#[derive(Debug)]
pub struct SpeedTestError {
    /// The kind of error.
    pub kind: ErrorKind,
    /// User-friendly error message.
    pub message: String,
    /// Optional suggestion for how to resolve the error.
    pub suggestion: Option<String>,
    /// The underlying error, if any.
    pub source: Option<Box<dyn Error + Send + Sync>>,
}

impl SpeedTestError {
    /// Create a new SpeedTestError.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into(), suggestion: None, source: None }
    }

    /// Add a suggestion for how to resolve the error.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add the underlying error source.
    pub fn with_source(
        mut self,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Get the exit code for this error.
    pub fn exit_code(&self) -> i32 {
        self.kind.exit_code()
    }

    /// Create a network error.
    pub fn network(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Network, message)
            .with_suggestion("Check your internet connection and try again.")
    }

    /// Create a DNS error.
    pub fn dns(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Dns, message).with_suggestion(
            "Check your DNS settings or try using a different DNS server.",
        )
    }

    /// Create a timeout error.
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Timeout, message).with_suggestion(
            "The server may be slow or unreachable. Try again later.",
        )
    }

    /// Create a TLS error.
    pub fn tls(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Tls, message).with_suggestion(
            "There may be a certificate issue. Check your system time.",
        )
    }

    /// Create an API error.
    pub fn api(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Api, message).with_suggestion(
            "The Cloudflare API may be experiencing issues. Try again later.",
        )
    }

    /// Create a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Config, message)
    }

    /// Create a measurement error.
    pub fn measurement(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Measurement, message)
    }
}

impl fmt::Display for SpeedTestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.description(), self.message)?;

        if let Some(ref suggestion) = self.suggestion {
            write!(f, "\n  Suggestion: {}", suggestion)?;
        }

        Ok(())
    }
}

impl Error for SpeedTestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &(dyn Error + 'static))
    }
}

/// Classify an error into an ErrorKind based on its message.
pub fn classify_error(error: &dyn Error) -> ErrorKind {
    let error_str = error.to_string().to_lowercase();

    if error_str.contains("dns")
        || error_str.contains("resolve")
        || error_str.contains("no such host")
    {
        return ErrorKind::Dns;
    }

    if error_str.contains("timeout")
        || error_str.contains("timed out")
        || error_str.contains("deadline")
    {
        return ErrorKind::Timeout;
    }

    if error_str.contains("tls")
        || error_str.contains("ssl")
        || error_str.contains("certificate")
        || error_str.contains("handshake")
    {
        return ErrorKind::Tls;
    }

    if error_str.contains("connection refused")
        || error_str.contains("connection reset")
        || error_str.contains("network unreachable")
        || error_str.contains("host unreachable")
        || error_str.contains("no route")
        || error_str.contains("broken pipe")
    {
        return ErrorKind::Network;
    }

    if error_str.contains("status: 4")
        || error_str.contains("status: 5")
        || error_str.contains("api")
        || error_str.contains("server error")
    {
        return ErrorKind::Api;
    }

    ErrorKind::Unknown
}

/// Convert a generic error into a SpeedTestError with appropriate classification.
pub fn to_speed_test_error(
    error: Box<dyn Error + Send + Sync>,
    context: &str,
) -> SpeedTestError {
    let kind = classify_error(error.as_ref());
    let message = format!("{}: {}", context, error);

    let mut speed_error = SpeedTestError::new(kind, message);
    speed_error.source = Some(error);

    // Add appropriate suggestions based on error kind
    speed_error = match kind {
        ErrorKind::Network => speed_error
            .with_suggestion("Check your internet connection and try again."),
        ErrorKind::Dns => speed_error.with_suggestion(
            "Check your DNS settings or try using a different DNS server.",
        ),
        ErrorKind::Timeout => speed_error.with_suggestion(
            "The server may be slow or unreachable. Try again later.",
        ),
        ErrorKind::Tls => speed_error.with_suggestion(
            "There may be a certificate issue. Check your system time.",
        ),
        ErrorKind::Api => speed_error.with_suggestion(
            "The Cloudflare API may be experiencing issues. Try again later.",
        ),
        _ => speed_error,
    };

    speed_error
}

/// Format an error for user display.
///
/// This function creates a user-friendly error message that includes
/// the error description and any available suggestions.
pub fn format_error_for_display(error: &SpeedTestError) -> String {
    let mut output = format!("Error: {}", error.message);

    if let Some(ref suggestion) = error.suggestion {
        output.push_str(&format!("\n\nSuggestion: {}", suggestion));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_kind_exit_codes() {
        assert_eq!(ErrorKind::Network.exit_code(), exit_codes::NETWORK_ERROR);
        assert_eq!(ErrorKind::Dns.exit_code(), exit_codes::NETWORK_ERROR);
        assert_eq!(ErrorKind::Timeout.exit_code(), exit_codes::NETWORK_ERROR);
        assert_eq!(ErrorKind::Api.exit_code(), exit_codes::API_ERROR);
        assert_eq!(ErrorKind::Config.exit_code(), exit_codes::CONFIG_ERROR);
    }

    #[test]
    fn test_speed_test_error_display() {
        let error = SpeedTestError::network("Failed to connect to server")
            .with_suggestion("Check your internet connection.");

        let display = format!("{}", error);
        assert!(display.contains("Network error"));
        assert!(display.contains("Failed to connect"));
        assert!(display.contains("Suggestion"));
    }

    #[test]
    fn test_classify_error_dns() {
        let error = std::io::Error::new(
            std::io::ErrorKind::Other,
            "DNS resolution failed: no such host",
        );
        assert_eq!(classify_error(&error), ErrorKind::Dns);
    }

    #[test]
    fn test_classify_error_timeout() {
        let error = std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "connection timed out",
        );
        assert_eq!(classify_error(&error), ErrorKind::Timeout);
    }

    #[test]
    fn test_classify_error_network() {
        let error = std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "connection refused",
        );
        assert_eq!(classify_error(&error), ErrorKind::Network);
    }

    #[test]
    fn test_classify_error_unknown() {
        let error = std::io::Error::new(
            std::io::ErrorKind::Other,
            "some random error",
        );
        assert_eq!(classify_error(&error), ErrorKind::Unknown);
    }

    #[test]
    fn test_to_speed_test_error() {
        let error: Box<dyn Error + Send + Sync> =
            Box::new(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "connection refused",
            ));

        let speed_error = to_speed_test_error(error, "download test");

        assert_eq!(speed_error.kind, ErrorKind::Network);
        assert!(speed_error.message.contains("download test"));
        assert!(speed_error.suggestion.is_some());
    }
}
