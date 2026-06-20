use anyhow::Result;
use clap::Args;
use nw_jobs::{CancellationToken, JobRunner};

#[derive(Debug, Clone, Default, Args)]
pub struct JobArgs {
    /// Worker count. Omit for Rayon default; use 0 to run on the caller thread.
    #[arg(long)]
    pub jobs: Option<usize>,
}

impl JobArgs {
    pub fn ctx(&self) -> Result<RunCtx> {
        Ok(RunCtx {
            runner: JobRunner::from_jobs(self.jobs)?,
            cancel: nw_jobs::cancellation_token_with_signal_handlers(),
        })
    }
}

#[derive(Debug)]
pub struct RunCtx {
    pub runner: JobRunner,
    pub cancel: CancellationToken,
}
