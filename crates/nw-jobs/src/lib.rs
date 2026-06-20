//! Local parallel work and cooperative cancellation primitives.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;
use rayon::{ThreadPool, ThreadPoolBuilder};
use thiserror::Error;

#[derive(Debug, Clone, Default)]
pub struct JobRunner {
    execution: Execution,
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    flag: Arc<AtomicBool>,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.flag)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobBatch<R> {
    completed: Vec<R>,
    skipped: usize,
    cancelled: bool,
}

impl<R> JobBatch<R> {
    fn new(completed: Vec<R>, skipped: usize, cancelled: bool) -> Self {
        Self {
            completed,
            skipped,
            cancelled,
        }
    }

    #[must_use]
    pub fn completed(&self) -> &[R] {
        &self.completed
    }

    #[must_use]
    pub const fn skipped(&self) -> usize {
        self.skipped
    }

    #[must_use]
    pub const fn was_cancelled(&self) -> bool {
        self.cancelled
    }

    #[must_use]
    pub fn into_completed(self) -> Vec<R> {
        self.completed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobRunnerPolicy {
    Inline,
    Automatic,
    Workers(usize),
}

impl fmt::Display for JobRunnerPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inline => f.write_str("caller thread"),
            Self::Automatic => f.write_str("automatic worker pool"),
            Self::Workers(workers) => write!(f, "{workers} worker(s)"),
        }
    }
}

#[derive(Debug, Clone, Default)]
enum Execution {
    Inline,
    #[default]
    Global,
    Pool(Arc<ThreadPool>),
}

impl JobRunner {
    #[must_use]
    pub fn automatic() -> Self {
        Self {
            execution: Execution::Global,
        }
    }

    #[must_use]
    pub fn inline() -> Self {
        Self {
            execution: Execution::Inline,
        }
    }

    /// Build a runner from an optional CLI-style worker count.
    ///
    /// `None` uses the global Rayon pool, `Some(0)` runs on the caller thread,
    /// and any other value creates a private worker pool.
    ///
    /// # Errors
    ///
    /// Returns [`JobRunnerBuildError`] if Rayon cannot create the requested
    /// private worker pool.
    pub fn from_jobs(jobs: Option<usize>) -> Result<Self, JobRunnerBuildError> {
        match jobs {
            None => Ok(Self::automatic()),
            Some(0) => Ok(Self::inline()),
            Some(workers) => Self::with_workers(workers),
        }
    }

    /// Build a runner backed by a private worker pool.
    ///
    /// `workers == 0` returns an inline runner.
    ///
    /// # Errors
    ///
    /// Returns [`JobRunnerBuildError`] if Rayon cannot create the requested
    /// private worker pool.
    pub fn with_workers(workers: usize) -> Result<Self, JobRunnerBuildError> {
        if workers == 0 {
            return Ok(Self::inline());
        }

        ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .map(|pool| Self {
                execution: Execution::Pool(Arc::new(pool)),
            })
            .map_err(JobRunnerBuildError)
    }

    #[must_use]
    pub fn is_inline(&self) -> bool {
        matches!(self.execution, Execution::Inline)
    }

    #[must_use]
    pub fn policy(&self) -> JobRunnerPolicy {
        match &self.execution {
            Execution::Inline => JobRunnerPolicy::Inline,
            Execution::Global => JobRunnerPolicy::Automatic,
            Execution::Pool(pool) => JobRunnerPolicy::Workers(pool.current_num_threads()),
        }
    }

    pub fn map<T, R, F>(&self, items: &[T], f: F) -> Vec<R>
    where
        T: Sync,
        R: Send,
        F: Fn(&T) -> R + Send + Sync,
    {
        match &self.execution {
            Execution::Inline => items.iter().map(f).collect(),
            Execution::Global => items.par_iter().map(f).collect(),
            Execution::Pool(pool) => pool.install(|| items.par_iter().map(f).collect()),
        }
    }

    pub fn map_until_cancelled<T, R, F>(
        &self,
        items: &[T],
        cancel: &CancellationToken,
        f: F,
    ) -> JobBatch<R>
    where
        T: Sync,
        R: Send,
        F: Fn(&T) -> R + Send + Sync,
    {
        let mapped: Vec<Option<R>> = match &self.execution {
            Execution::Inline => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(if cancel.is_cancelled() {
                        None
                    } else {
                        Some(f(item))
                    });
                }
                out
            }
            Execution::Global => items
                .par_iter()
                .map(|item| {
                    if cancel.is_cancelled() {
                        None
                    } else {
                        Some(f(item))
                    }
                })
                .collect(),
            Execution::Pool(pool) => pool.install(|| {
                items
                    .par_iter()
                    .map(|item| {
                        if cancel.is_cancelled() {
                            None
                        } else {
                            Some(f(item))
                        }
                    })
                    .collect()
            }),
        };
        collect_job_batch(mapped, cancel.is_cancelled())
    }

    pub fn map_init<T, S, R, Init, F>(&self, items: &[T], init: Init, f: F) -> Vec<R>
    where
        T: Sync,
        S: Send,
        R: Send,
        Init: Fn() -> S + Send + Sync,
        F: Fn(&mut S, &T) -> R + Send + Sync,
    {
        match &self.execution {
            Execution::Inline => {
                let mut state = init();
                items.iter().map(|item| f(&mut state, item)).collect()
            }
            Execution::Global => items.par_iter().map_init(init, f).collect(),
            Execution::Pool(pool) => pool.install(|| items.par_iter().map_init(init, f).collect()),
        }
    }

    pub fn map_init_until_cancelled<T, S, R, Init, F>(
        &self,
        items: &[T],
        cancel: &CancellationToken,
        init: Init,
        f: F,
    ) -> JobBatch<R>
    where
        T: Sync,
        S: Send,
        R: Send,
        Init: Fn() -> S + Send + Sync,
        F: Fn(&mut S, &T) -> R + Send + Sync,
    {
        let mapped: Vec<Option<R>> = match &self.execution {
            Execution::Inline => {
                let mut state = init();
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(if cancel.is_cancelled() {
                        None
                    } else {
                        Some(f(&mut state, item))
                    });
                }
                out
            }
            Execution::Global => items
                .par_iter()
                .map_init(init, |state, item| {
                    if cancel.is_cancelled() {
                        None
                    } else {
                        Some(f(state, item))
                    }
                })
                .collect(),
            Execution::Pool(pool) => pool.install(|| {
                items
                    .par_iter()
                    .map_init(init, |state, item| {
                        if cancel.is_cancelled() {
                            None
                        } else {
                            Some(f(state, item))
                        }
                    })
                    .collect()
            }),
        };
        collect_job_batch(mapped, cancel.is_cancelled())
    }
}

/// Register SIGINT/SIGTERM handlers that cancel `cancel`.
///
/// # Errors
///
/// Returns [`SignalInstallError`] if the platform refuses a signal
/// registration.
pub fn install_signal_handlers(cancel: &CancellationToken) -> Result<(), SignalInstallError> {
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, cancel.shared_flag())
            .map_err(|source| SignalInstallError { signal, source })?;
    }
    Ok(())
}

pub fn cancellation_token_with_signal_handlers() -> CancellationToken {
    let cancel = CancellationToken::new();
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        if let Err(error) = signal_hook::flag::register(signal, cancel.shared_flag()) {
            tracing::debug!("failed to register cancellation signal {signal}: {error}");
        }
    }
    cancel
}

fn collect_job_batch<R>(mapped: Vec<Option<R>>, cancelled: bool) -> JobBatch<R> {
    let skipped = mapped.iter().filter(|item| item.is_none()).count();
    let completed = mapped.into_iter().flatten().collect();
    JobBatch::new(completed, skipped, cancelled || skipped > 0)
}

#[derive(Debug)]
pub struct JobRunnerBuildError(rayon::ThreadPoolBuildError);

impl fmt::Display for JobRunnerBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for JobRunnerBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

#[derive(Debug, Error)]
#[error("failed to register cancellation signal {signal}: {source}")]
pub struct SignalInstallError {
    signal: i32,
    source: std::io::Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_map_keeps_order() {
        let runner = JobRunner::inline();
        assert_eq!(runner.map(&[1, 2, 3], |value| value * 2), vec![2, 4, 6]);
    }

    #[test]
    fn cancellation_skips_inline_items_after_flag() {
        let runner = JobRunner::inline();
        let cancel = CancellationToken::new();
        let batch = runner.map_until_cancelled(&[1, 2, 3], &cancel, |value| {
            if *value == 1 {
                cancel.cancel();
            }
            *value
        });

        assert_eq!(batch.completed(), &[1]);
        assert_eq!(batch.skipped(), 2);
        assert!(batch.was_cancelled());
    }

    #[test]
    fn explicit_zero_jobs_is_inline() {
        let runner = JobRunner::from_jobs(Some(0)).unwrap();
        assert!(runner.is_inline());
    }
}
