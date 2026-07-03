use nw_jobs::{CancellationToken, JobRunner, JobRunnerBuildError};

use crate::status::CodegenStatus;

#[derive(Debug, Clone)]
pub struct CodegenContext {
    runner: JobRunner,
    cancel: CancellationToken,
    status: CodegenStatus,
}

impl CodegenContext {
    #[must_use]
    pub fn automatic() -> Self {
        Self {
            runner: JobRunner::automatic(),
            cancel: CancellationToken::new(),
            status: CodegenStatus::default(),
        }
    }

    #[must_use]
    pub fn inline() -> Self {
        Self {
            runner: JobRunner::inline(),
            cancel: CancellationToken::new(),
            status: CodegenStatus::default(),
        }
    }

    pub fn from_jobs(jobs: Option<usize>) -> Result<Self, JobRunnerBuildError> {
        Ok(Self {
            runner: JobRunner::from_jobs(jobs)?,
            cancel: nw_jobs::cancellation_token_with_signal_handlers(),
            status: CodegenStatus::default(),
        })
    }

    #[must_use]
    pub fn new(runner: JobRunner, cancel: CancellationToken) -> Self {
        Self {
            runner,
            cancel,
            status: CodegenStatus::default(),
        }
    }

    #[must_use]
    pub fn with_status(mut self, status: CodegenStatus) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub fn runner(&self) -> &JobRunner {
        &self.runner
    }

    #[must_use]
    pub fn cancel(&self) -> &CancellationToken {
        &self.cancel
    }

    #[must_use]
    pub fn status(&self) -> &CodegenStatus {
        &self.status
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    pub fn install<R, F>(&self, f: F) -> R
    where
        R: Send,
        F: FnOnce(&Self) -> R + Send,
    {
        let context = self.clone();
        self.runner.install(move || f(&context))
    }
}

impl Default for CodegenContext {
    fn default() -> Self {
        Self::automatic()
    }
}
