use anyhow::Result as AnyResult;
use clap::Args;
use nw_jobs::{CancellationToken, JobBatch, JobRunner};

use crate::progress::{Batch, Job, Progress};

#[derive(Debug, Clone, Default, Args)]
pub struct JobArgs {
    /// Worker count. Omit for Rayon default; use 0 to run on the caller thread.
    #[arg(long)]
    pub jobs: Option<usize>,

    /// Disable live progress rendering.
    #[arg(long)]
    pub no_progress: bool,
}

impl JobArgs {
    pub fn ctx(&self) -> AnyResult<RunCtx> {
        Ok(RunCtx {
            runner: JobRunner::from_jobs(self.jobs)?,
            cancel: nw_jobs::cancellation_token_with_signal_handlers(),
            progress: Progress::auto(!self.no_progress),
        })
    }
}

#[derive(Debug)]
pub struct RunCtx {
    pub runner: JobRunner,
    pub cancel: CancellationToken,
    pub progress: Progress,
}

impl RunCtx {
    pub fn map_results<T, R, E, N, F>(
        &self,
        label: &'static str,
        items: &[T],
        name: N,
        f: F,
    ) -> JobBatch<Result<R, E>>
    where
        T: Sync,
        R: Send,
        E: Send,
        N: Fn(&T) -> String + Send + Sync,
        F: Fn(&T, Job) -> Result<R, E> + Send + Sync,
    {
        let progress = self.progress.batch(label, items.len());
        self.map_with_progress(progress, items, name, f)
    }

    pub fn map_results_compact<T, R, E, N, F>(
        &self,
        label: &'static str,
        items: &[T],
        name: N,
        f: F,
    ) -> JobBatch<Result<R, E>>
    where
        T: Sync,
        R: Send,
        E: Send,
        N: Fn(&T) -> String + Send + Sync,
        F: Fn(&T, Job) -> Result<R, E> + Send + Sync,
    {
        let progress = self.progress.batch_compact(label, items.len());
        self.map_with_progress(progress, items, name, f)
    }

    fn map_with_progress<T, R, E, N, F>(
        &self,
        progress: Batch,
        items: &[T],
        name: N,
        f: F,
    ) -> JobBatch<Result<R, E>>
    where
        T: Sync,
        R: Send,
        E: Send,
        N: Fn(&T) -> String + Send + Sync,
        F: Fn(&T, Job) -> Result<R, E> + Send + Sync,
    {
        let batch = self
            .runner
            .map_until_cancelled(items, &self.cancel, |item| {
                let job = progress.job(name(item));
                let result = f(item, job.clone());
                if result.is_ok() {
                    job.finish("done");
                } else {
                    job.finish("failed");
                }
                result
            });
        progress.finish();
        batch
    }
}
