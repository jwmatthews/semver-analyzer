//! Error tip and diagnostic wrapper types for user-facing error reporting.
//!
//! This module provides the error reporting contract for the semver-analyzer:
//!
//! - [`ErrorTip`] — Trait for errors that carry remediation tips
//! - [`Diagnosed`] — Marker type that carries tips through the `anyhow` chain
//! - [`DiagnoseWithTip`] — Extension trait for `Result<T, E: ErrorTip>`
//! - [`DiagnoseExt`] — Extension trait for attaching explicit tip strings
//!
//! ## How It Works
//!
//! Language implementations define domain-specific error types (e.g.,
//! `WorktreeError` for TypeScript) and implement `ErrorTip` on them.
//! At the boundary where errors enter `anyhow::Result`, call `.diagnose()`
//! to capture the tip into a `Diagnosed` marker. The CLI renderer walks
//! the `anyhow` chain and extracts the tip via a single
//! `downcast_ref::<Diagnosed>()`.
//!
//! ```text
//! Language impl                   Orchestrator            CLI
//! ─────────────                   ────────────            ───
//! WorktreeError::TscFailed        .context("...")         render_error()
//!   → .diagnose()                   → propagates           → downcast Diagnosed
//!   → Diagnosed { tip } added       via ?                  → shows tip
//! ```

use std::fmt;

// ── ErrorTip trait ─────────────────────────────────────────────────────

/// Contract for errors that carry user-facing remediation tips.
///
/// Implement this on any error type that should provide actionable
/// guidance when rendered to the user. The tip is captured into a
/// [`Diagnosed`] wrapper via [`.diagnose()`](DiagnoseWithTip::diagnose)
/// at the error boundary.
///
/// # For Language implementors
///
/// 1. Define your error enum with `thiserror::Error`
/// 2. Implement `ErrorTip` — every variant a user can trigger MUST have a tip
/// 3. At the boundary where the error enters `anyhow::Result`, call `.diagnose()`
///
/// # Example
///
/// ```rust,ignore
/// impl ErrorTip for WorktreeError {
///     fn tip(&self) -> Option<String> {
///         Some(match self {
///             Self::TscFailed { error_count, .. } => format!(
///                 "TypeScript compilation failed with {} error(s).\n\
///                  Use --log-file debug.log to see full tsc output.",
///                 error_count
///             ),
///             // ...
///         })
///     }
/// }
///
/// // At boundary:
/// let guard = WorktreeGuard::new(repo, ref, cmd).diagnose()?;
/// ```
pub trait ErrorTip: std::error::Error {
    /// Return a remediation tip for this error, if one is available.
    ///
    /// Each line in the returned string is a separate suggestion.
    /// Return `None` for errors where no actionable advice exists
    /// (e.g., `Io` transparent wrappers).
    fn tip(&self) -> Option<String>;
}

// ── Diagnosed wrapper ──────────────────────────────────────────────────

/// Marker type that carries a user-facing tip through the `anyhow` error chain.
///
/// Added automatically by [`.diagnose()`](DiagnoseWithTip::diagnose) or
/// [`.with_diagnosis()`](DiagnoseExt::with_diagnosis). The CLI renderer
/// extracts it via a single `downcast_ref::<Diagnosed>()` — no per-type
/// dispatch needed.
///
/// `Display` returns an empty string so the marker is invisible in the
/// error chain's "caused by" output.
#[derive(Debug)]
pub struct Diagnosed {
    tip: String,
}

impl Diagnosed {
    /// Create a new diagnosed marker with the given tip.
    pub fn new(tip: impl Into<String>) -> Self {
        Self { tip: tip.into() }
    }

    /// Get the remediation tip.
    pub fn tip(&self) -> &str {
        &self.tip
    }
}

impl fmt::Display for Diagnosed {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Invisible in the error chain — just a tip carrier
        Ok(())
    }
}

impl std::error::Error for Diagnosed {}

/// Internal error wrapper that carries a tip alongside the original error.
///
/// This is the actual type inserted into the anyhow chain. The CLI
/// extracts the tip by downcasting to `Diagnosed` (which this derefs to
/// via the chain's source).
///
/// The `Display` impl delegates to the source error so the tip doesn't
/// appear in the error message — only in the rendered output.
#[derive(Debug)]
pub struct DiagnosedError {
    tip: String,
    source: anyhow::Error,
}

impl fmt::Display for DiagnosedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display the source error, not the tip
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for DiagnosedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

impl DiagnosedError {
    /// Get the remediation tip.
    pub fn tip(&self) -> &str {
        &self.tip
    }
}

// ── DiagnoseWithTip extension ──────────────────────────────────────────

/// Extension trait for `Result<T, E>` where `E` implements [`ErrorTip`].
///
/// Automatically extracts the tip from the error and wraps it in a
/// [`Diagnosed`] context layer on the `anyhow::Error`.
pub trait DiagnoseWithTip<T> {
    /// Convert the error to `anyhow::Error` and attach its tip as a
    /// [`Diagnosed`] context layer.
    ///
    /// If the error's `tip()` returns `None`, the error is converted
    /// to `anyhow::Error` without a `Diagnosed` marker.
    fn diagnose(self) -> anyhow::Result<T>;
}

impl<T, E> DiagnoseWithTip<T> for Result<T, E>
where
    E: ErrorTip + Send + Sync + 'static,
{
    fn diagnose(self) -> anyhow::Result<T> {
        self.map_err(|e| {
            let tip = e.tip();
            let err: anyhow::Error = e.into();
            match tip {
                Some(t) => {
                    // Wrap in Diagnosed as the outer error, with the original
                    // as its source. This way downcast_ref::<Diagnosed>() works
                    // on the chain because Diagnosed is the outermost error.
                    let diagnosed = DiagnosedError {
                        tip: t,
                        source: err,
                    };
                    anyhow::Error::new(diagnosed)
                }
                None => err,
            }
        })
    }
}

// ── DiagnoseExt extension ──────────────────────────────────────────────

/// Extension trait for attaching an explicit tip string to any `Result`.
///
/// Use this for errors that don't implement [`ErrorTip`] but where you
/// still want to provide remediation guidance.
///
/// # Example
///
/// ```rust,ignore
/// use semver_analyzer_core::error::DiagnoseExt;
///
/// fs::read(path)
///     .with_context(|| format!("Failed to read {}", path.display()))
///     .with_diagnosis("Check the file exists and you have read permissions.")?;
/// ```
pub trait DiagnoseExt<T> {
    /// Attach a remediation tip to the error.
    fn with_diagnosis(self, tip: impl Into<String>) -> anyhow::Result<T>;
}

impl<T, E> DiagnoseExt<T> for Result<T, E>
where
    E: Into<anyhow::Error>,
{
    fn with_diagnosis(self, tip: impl Into<String>) -> anyhow::Result<T> {
        self.map_err(|e| {
            let source = e.into();
            anyhow::Error::new(DiagnosedError {
                tip: tip.into(),
                source,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A test error type implementing ErrorTip
    #[derive(Debug)]
    struct TestError {
        msg: String,
        has_tip: bool,
    }

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.msg)
        }
    }

    impl std::error::Error for TestError {}

    impl ErrorTip for TestError {
        fn tip(&self) -> Option<String> {
            if self.has_tip {
                Some("Try doing X instead.".into())
            } else {
                None
            }
        }
    }

    #[test]
    fn diagnose_captures_tip() {
        let result: Result<(), TestError> = Err(TestError {
            msg: "something broke".into(),
            has_tip: true,
        });

        let err = result.diagnose().unwrap_err();

        // The outermost error should be DiagnosedError
        let diagnosed = err.downcast_ref::<DiagnosedError>();
        assert!(
            diagnosed.is_some(),
            "DiagnosedError not found at top of chain"
        );
        assert_eq!(diagnosed.unwrap().tip(), "Try doing X instead.");
    }

    #[test]
    fn diagnose_without_tip_skips_marker() {
        let result: Result<(), TestError> = Err(TestError {
            msg: "something broke".into(),
            has_tip: false,
        });

        let err = result.diagnose().unwrap_err();

        // No DiagnosedError should be present
        assert!(
            err.downcast_ref::<DiagnosedError>().is_none(),
            "DiagnosedError should not be present when tip() returns None"
        );
    }

    #[test]
    fn with_diagnosis_attaches_tip() {
        let result: Result<(), std::io::Error> = Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));

        let err = result.with_diagnosis("Check the path exists.").unwrap_err();

        let diagnosed = err.downcast_ref::<DiagnosedError>();
        assert!(
            diagnosed.is_some(),
            "DiagnosedError not found at top of chain"
        );
        assert_eq!(diagnosed.unwrap().tip(), "Check the path exists.");
    }

    #[test]
    fn diagnosed_display_is_empty() {
        let d = Diagnosed::new("some tip");
        assert_eq!(d.to_string(), "");
    }
}
