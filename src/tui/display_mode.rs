//! Display mode detection and configuration.
//!
//! Determines whether to use TUI, silent, or JSON output mode
//! based on CLI flags and terminal capabilities.

/// The display mode for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    /// Full TUI with progress indicators and live updates
    Tui,
    /// Silent mode - no output until final results
    Silent,
    /// JSON mode - structured output only
    Json,
}

impl DisplayMode {
    /// Determine display mode from CLI flags and environment.
    ///
    /// # Arguments
    /// * `json_flag` - Whether the `--json` flag was provided
    /// * `is_tty` - Whether stdout is a TTY (interactive terminal)
    ///
    /// # Returns
    /// * `Json` when json_flag is true (regardless of is_tty)
    /// * `Tui` when json_flag is false AND is_tty is true
    /// * `Silent` when json_flag is false AND is_tty is false
    pub fn detect(json_flag: bool, is_tty: bool) -> Self {
        if json_flag {
            DisplayMode::Json
        } else if is_tty {
            DisplayMode::Tui
        } else {
            DisplayMode::Silent
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_json_flag_returns_json_mode() {
        // JSON flag takes precedence regardless of TTY status
        assert_eq!(DisplayMode::detect(true, true), DisplayMode::Json);
        assert_eq!(DisplayMode::detect(true, false), DisplayMode::Json);
    }

    #[test]
    fn test_tty_without_json_returns_tui_mode() {
        assert_eq!(DisplayMode::detect(false, true), DisplayMode::Tui);
    }

    #[test]
    fn test_non_tty_without_json_returns_silent_mode() {
        assert_eq!(DisplayMode::detect(false, false), DisplayMode::Silent);
    }

    // Feature: tui-progress-display, Property 1: Display Mode Selection
    // Validates: Requirements 1.1, 1.2, 1.3
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any combination of (json_flag, is_tty), DisplayMode::detect
        /// returns:
        /// - Json when json_flag is true (regardless of is_tty)
        /// - Tui when json_flag is false AND is_tty is true
        /// - Silent when json_flag is false AND is_tty is false
        #[test]
        fn display_mode_selection_property(
            json_flag in any::<bool>(),
            is_tty in any::<bool>()
        ) {
            let result = DisplayMode::detect(json_flag, is_tty);

            // Property 1: JSON flag takes precedence
            if json_flag {
                prop_assert_eq!(
                    result,
                    DisplayMode::Json,
                    "When json_flag is true, mode should be Json"
                );
            }
            // Property 2: TTY without JSON flag gives TUI
            else if is_tty {
                prop_assert_eq!(
                    result,
                    DisplayMode::Tui,
                    "When json_flag is false and is_tty is true, mode should be Tui"
                );
            }
            // Property 3: Non-TTY without JSON flag gives Silent
            else {
                prop_assert_eq!(
                    result,
                    DisplayMode::Silent,
                    "When json_flag is false and is_tty is false, mode should be Silent"
                );
            }
        }
    }
}
