//! Degradation tracking for non-fatal issues during analysis.
//!
//! The [`DegradationTracker`] collects issues that degrade analysis quality
//! without causing a fatal error. It is thread-safe and accessible via
//! `SharedFindings::degradation()` so all pipeline phases and Language
//! implementations can record issues.
//!
//! At the end of a run, the CLI renders a summary of all recorded issues
//! so the user knows what parts of the analysis may be incomplete.
//!
//! ## When to Record a Degradation
//!
//! - A pipeline phase fails but execution can continue with partial results
//! - An external tool (LLM, CSS extraction, dep repo build) fails
//! - Multiple per-item failures occur (batch into a single summary entry)
//!
//! ## When NOT to Record
//!
//! - Best-effort operations where failure is a normal code path
//!   (e.g., `read_git_file` returning `None` for a file that may not exist)
//! - Cleanup/teardown failures (Drop impls, worktree removal)

use std::sync::Mutex;

/// Tracks non-fatal issues that degrade analysis quality.
///
/// Thread-safe — wrap in `Arc` and share across pipeline phases.
/// Lives on `SharedFindings` for convenient access.
#[derive(Debug, Default)]
pub struct DegradationTracker {
    issues: Mutex<Vec<DegradationIssue>>,
}

/// A single non-fatal issue recorded during analysis.
#[derive(Debug, Clone)]
pub struct DegradationIssue {
    /// Short pipeline phase tag: "TD", "SD", "BU", "CSS", "LLM".
    pub phase: String,
    /// What happened (technical, concise).
    pub message: String,
    /// What the user is missing as a result (user-facing, actionable).
    pub impact: String,
}

impl DegradationTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a non-fatal issue.
    ///
    /// # Arguments
    ///
    /// * `phase` — Short tag identifying the pipeline phase ("TD", "SD", "BU", "CSS", "LLM")
    /// * `message` — What happened (technical, concise)
    /// * `impact` — What the user is missing (user-facing, actionable)
    pub fn record(
        &self,
        phase: impl Into<String>,
        message: impl Into<String>,
        impact: impl Into<String>,
    ) {
        self.issues.lock().unwrap().push(DegradationIssue {
            phase: phase.into(),
            message: message.into(),
            impact: impact.into(),
        });
    }

    /// Get a snapshot of all recorded issues.
    pub fn issues(&self) -> Vec<DegradationIssue> {
        self.issues.lock().unwrap().clone()
    }

    /// Check if any issues have been recorded.
    pub fn has_issues(&self) -> bool {
        !self.issues.lock().unwrap().is_empty()
    }

    /// Get the count of recorded issues.
    pub fn issue_count(&self) -> usize {
        self.issues.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn empty_tracker_has_no_issues() {
        let tracker = DegradationTracker::new();
        assert!(!tracker.has_issues());
        assert_eq!(tracker.issue_count(), 0);
        assert!(tracker.issues().is_empty());
    }

    #[test]
    fn record_and_retrieve_issues() {
        let tracker = DegradationTracker::new();
        tracker.record(
            "SD",
            "Source-level analysis failed",
            "Composition trees unavailable",
        );
        tracker.record(
            "CSS",
            "CSS extraction failed",
            "CSS removal rules incomplete",
        );

        assert!(tracker.has_issues());
        assert_eq!(tracker.issue_count(), 2);

        let issues = tracker.issues();
        assert_eq!(issues[0].phase, "SD");
        assert_eq!(issues[0].message, "Source-level analysis failed");
        assert_eq!(issues[0].impact, "Composition trees unavailable");
        assert_eq!(issues[1].phase, "CSS");
    }

    #[test]
    fn thread_safe_recording() {
        let tracker = Arc::new(DegradationTracker::new());
        let mut handles = Vec::new();

        for i in 0..10 {
            let tracker = tracker.clone();
            handles.push(std::thread::spawn(move || {
                tracker.record("TEST", format!("Issue {}", i), "Test impact");
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(tracker.issue_count(), 10);
    }
}
