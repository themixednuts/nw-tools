use std::fmt;
use std::io::{self, IsTerminal};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};

#[derive(Clone)]
pub(crate) struct Progress {
    state: Option<Arc<ProgressState>>,
}

struct ProgressState {
    multi: MultiProgress,
}

#[derive(Clone)]
pub(crate) struct Batch {
    state: Option<Arc<ProgressState>>,
    overall: Option<ProgressBar>,
    show_jobs: bool,
}

#[derive(Clone)]
pub(crate) struct Job {
    bar: Option<ProgressBar>,
    overall: Option<ProgressBar>,
    finished: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(crate) struct Stage {
    bar: Option<ProgressBar>,
}

impl Progress {
    pub(crate) fn auto(enabled: bool) -> Self {
        if !enabled || !io::stderr().is_terminal() {
            return Self { state: None };
        }

        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stderr_with_hz(10));
        Self {
            state: Some(Arc::new(ProgressState { multi })),
        }
    }

    pub(crate) fn batch(&self, label: impl Into<String>, total: usize) -> Batch {
        self.batch_with_detail(label, total, true)
    }

    pub(crate) fn batch_compact(&self, label: impl Into<String>, total: usize) -> Batch {
        self.batch_with_detail(label, total, false)
    }

    fn batch_with_detail(&self, label: impl Into<String>, total: usize, show_jobs: bool) -> Batch {
        let Some(state) = &self.state else {
            return Batch::disabled();
        };

        let overall = ProgressBar::new(to_u64(total));
        overall.set_style(overall_style());
        overall.set_prefix(trim_label(label, 24));
        overall.set_message("queued");
        let overall = state.multi.add(overall);

        Batch {
            state: Some(Arc::clone(state)),
            overall: Some(overall),
            show_jobs,
        }
    }

    pub(crate) fn stage(&self, label: impl Into<String>) -> Stage {
        let Some(state) = &self.state else {
            return Stage::disabled();
        };

        let bar = ProgressBar::new_spinner();
        bar.set_style(stage_style());
        bar.set_prefix(trim_label(label, 32));
        bar.set_message("running");
        bar.enable_steady_tick(std::time::Duration::from_millis(120));

        Stage {
            bar: Some(state.multi.add(bar)),
        }
    }
}

impl Batch {
    fn disabled() -> Self {
        Self {
            state: None,
            overall: None,
            show_jobs: false,
        }
    }

    pub(crate) fn job(&self, label: impl Into<String>) -> Job {
        let Some(state) = &self.state else {
            return Job::disabled();
        };

        let label = trim_label(label, 44);
        if let Some(overall) = &self.overall {
            overall.set_message(label.clone());
        }
        if !self.show_jobs {
            return Job {
                bar: None,
                overall: self.overall.clone(),
                finished: Arc::new(AtomicBool::new(false)),
            };
        }

        let bar = ProgressBar::new(0);
        bar.set_style(job_style());
        bar.set_prefix(label);
        bar.set_message("queued");

        Job {
            bar: Some(state.multi.add(bar)),
            overall: self.overall.clone(),
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn finish(&self) {
        if let Some(overall) = &self.overall {
            overall.finish_with_message("done");
        }
        if let Some(state) = &self.state {
            let _ = state.multi.clear();
        }
    }
}

impl Job {
    fn disabled() -> Self {
        Self {
            bar: None,
            overall: None,
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn set_len(&self, len: usize) {
        if let Some(bar) = &self.bar {
            bar.set_length(to_u64(len));
            bar.set_message("running");
        }
    }

    pub(crate) fn inc(&self, delta: u64) {
        if let Some(bar) = &self.bar {
            bar.inc(delta);
        }
    }

    pub(crate) fn step<R, E>(&self, f: impl FnOnce() -> Result<R, E>) -> Result<R, E> {
        self.set_len(1);
        let result = f();
        if result.is_ok() {
            self.inc(1);
        }
        result
    }

    pub(crate) fn finish(&self, message: &'static str) {
        if self.finished.swap(true, Ordering::Relaxed) {
            return;
        }
        if let Some(bar) = &self.bar {
            bar.finish_with_message(message);
        }
        if let Some(overall) = &self.overall {
            overall.inc(1);
        }
    }
}

impl Stage {
    fn disabled() -> Self {
        Self { bar: None }
    }

    pub(crate) fn finish(&self, message: &'static str) {
        if let Some(bar) = &self.bar {
            bar.set_message(message);
            bar.finish_and_clear();
        }
    }
}

impl fmt::Debug for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Progress")
            .field("enabled", &self.state.is_some())
            .finish()
    }
}

impl fmt::Debug for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Batch")
            .field("enabled", &self.state.is_some())
            .finish()
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Job")
            .field("enabled", &self.bar.is_some())
            .finish()
    }
}

impl fmt::Debug for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stage")
            .field("enabled", &self.bar.is_some())
            .finish()
    }
}

fn overall_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:>24} [{bar:36.cyan/blue}] {pos}/{len} {msg}")
        .expect("valid overall progress template")
        .progress_chars("=> ")
}

fn job_style() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:>44} [{bar:28.green/black}] {pos}/{len} {msg}")
        .expect("valid job progress template")
        .progress_chars("=> ")
}

fn stage_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {prefix} {msg}")
        .expect("valid stage progress template")
}

fn trim_label(value: impl Into<String>, max: usize) -> String {
    let value = value.into().replace(['\r', '\n', '\t'], " ");
    if value.chars().count() <= max {
        value
    } else {
        let keep = max.saturating_sub(3);
        format!("{}...", value.chars().take(keep).collect::<String>())
    }
}

fn to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
