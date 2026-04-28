//! Progress reporting for the semver-analyzer CLI.
//!
//! Provides [`ProgressReporter`] which wraps `indicatif::MultiProgress` to
//! display spinners for timed phases and bar graphs for counted work.

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Instant;

// ── ProgressReporter ────────────────────────────────────────────────────

/// Thread-safe progress reporter backed by `indicatif::MultiProgress`.
///
/// Create once at startup, pass (by reference or clone) into the
/// orchestrator and any async tasks that need progress display.
#[derive(Clone)]
pub struct ProgressReporter {
    multi: Arc<MultiProgress>,
}

impl ProgressReporter {
    /// Create a new reporter. The underlying `MultiProgress` is drawn to
    /// stderr, keeping stdout clean for JSON data output.
    pub fn new() -> Self {
        Self {
            multi: Arc::new(MultiProgress::new()),
        }
    }

    /// Start a spinner for a phase without a known item count.
    ///
    /// Returns a [`PhaseGuard`] that shows a spinning animation while
    /// alive. When dropped (or explicitly finished), it replaces the
    /// spinner with a checkmark and the elapsed time.
    pub fn start_phase(&self, message: &str) -> PhaseGuard {
        let style = ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]);

        let pb = self.multi.add(ProgressBar::new_spinner());
        pb.set_style(style);
        pb.set_message(message.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));

        PhaseGuard {
            pb,
            start: Instant::now(),
            finish_message: None,
            failed: false,
        }
    }

    /// Start a progress bar for work with a known total count.
    ///
    /// Returns a [`CountedProgress`] with an `inc()` method. The bar
    /// is automatically finished when dropped.
    pub fn start_counted(&self, message: &str, total: u64) -> CountedProgress {
        let style = ProgressStyle::with_template(
            "{msg}  {bar:30.cyan/dim} {pos}/{len}  [elapsed: {elapsed}, eta: {eta}]",
        )
        .unwrap()
        .progress_chars("██░");

        let pb = self.multi.add(ProgressBar::new(total));
        pb.set_style(style);
        pb.set_message(message.to_string());

        CountedProgress { pb }
    }

    /// Print a line to stderr without clobbering any active progress bars.
    pub fn println(&self, msg: &str) {
        let _ = self.multi.println(msg);
    }
}

// ── PhaseGuard ──────────────────────────────────────────────────────────

/// RAII guard for a timed spinner phase.
///
/// While alive the spinner animates. Call [`finish`](PhaseGuard::finish)
/// with a completion message, or let it drop to auto-finish.
///
/// Use [`finish_failed`](PhaseGuard::finish_failed) to show a failure
/// indicator (✗) instead of the default success indicator (✓).
pub struct PhaseGuard {
    pb: ProgressBar,
    start: Instant,
    finish_message: Option<String>,
    failed: bool,
}

impl PhaseGuard {
    /// Finish the spinner with a custom completion message.
    /// The elapsed time is appended automatically. Shows ✓.
    pub fn finish(mut self, message: &str) {
        self.finish_message = Some(message.to_string());
        // Drop will handle the actual finish
    }

    /// Finish the spinner with a custom message and detail suffix.
    /// Produces: `✓ {message} ({detail}) ({elapsed})`
    pub fn finish_with_detail(mut self, message: &str, detail: &str) {
        self.finish_message = Some(format!("{} ({})", message, detail));
    }

    /// Finish the spinner with a failure indicator (✗).
    ///
    /// Use this for phases that failed but were non-fatal (the pipeline
    /// continues with degraded results). Fatal errors don't need this
    /// because the process exits via `render_error()`.
    pub fn finish_failed(mut self, message: &str) {
        self.failed = true;
        self.finish_message = Some(message.to_string());
    }

    fn do_finish(&self) {
        let elapsed = self.start.elapsed();
        let elapsed_str = format_duration(elapsed);
        let done_style = ProgressStyle::with_template("{msg}").unwrap();
        self.pb.set_style(done_style);

        let default_msg = self.pb.message();
        let msg = self.finish_message.as_deref().unwrap_or(&default_msg);

        let icon = if self.failed { "✗" } else { "✓" };
        self.pb
            .finish_with_message(format!("{} {} ({})", icon, msg, elapsed_str));
    }
}

impl Drop for PhaseGuard {
    fn drop(&mut self) {
        self.do_finish();
    }
}

// ── CountedProgress ─────────────────────────────────────────────────────

/// Progress bar for work with a known item count.
pub struct CountedProgress {
    pb: ProgressBar,
}

impl CountedProgress {
    /// Increment the progress bar by one unit.
    pub fn inc(&self) {
        self.pb.inc(1);
    }

    /// Update the message displayed to the left of the bar.
    /// Useful for showing the current item name alongside the bar.
    #[allow(dead_code)]
    pub fn set_message(&self, msg: &str) {
        self.pb.set_message(msg.to_string());
    }

    /// Finish the bar (marks it as complete).
    pub fn finish(self) {
        // Drop handles it
    }
}

impl Drop for CountedProgress {
    fn drop(&mut self) {
        let done_style = ProgressStyle::with_template("✓ {msg}  {len} items  [{elapsed}]").unwrap();
        self.pb.set_style(done_style);
        self.pb.finish();
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Format a duration as human-readable: "1.2s", "350ms", "2m 15s".
fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = secs as u64 / 60;
        let remaining = secs as u64 % 60;
        format!("{}m {}s", mins, remaining)
    }
}
