//! Bounded blocking producer/worker/consumer pipelines.
//!
//! This module owns the generic, project-agnostic concurrency primitive:
//! a blocking producer feeds bounded worker threads, workers emit bounded
//! outputs, and the caller consumes outputs on the current thread. Engine or
//! tool layers can wrap it with their own task kinds, progress, and service
//! policy without reimplementing the worker/channel mechanics.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, SendTimeoutError, Sender, bounded};

use crate::Cancellation;

const DEFAULT_PIPELINE_CHANNEL_CAPACITY: usize = 1024;
const CHANNEL_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Configuration for [`try_for_each_bounded_blocking`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingPipelineConfig {
    worker_count: usize,
    input_capacity: usize,
    output_capacity: usize,
    thread_name_prefix: String,
}

impl Default for BlockingPipelineConfig {
    fn default() -> Self {
        Self::automatic()
    }
}

impl BlockingPipelineConfig {
    /// Construct a pipeline config with a specific worker count.
    #[must_use]
    pub fn new(worker_count: usize) -> Self {
        Self::automatic().with_worker_count(worker_count)
    }

    /// Construct a pipeline config sized from host parallelism.
    #[must_use]
    pub fn automatic() -> Self {
        Self {
            worker_count: std::thread::available_parallelism().map_or(1, |workers| workers.get()),
            input_capacity: DEFAULT_PIPELINE_CHANNEL_CAPACITY,
            output_capacity: DEFAULT_PIPELINE_CHANNEL_CAPACITY,
            thread_name_prefix: "nw-jobs-pipeline".to_string(),
        }
    }

    /// Set the exact number of worker threads. Values below 1 are clamped to 1.
    #[must_use]
    pub fn with_worker_count(mut self, worker_count: usize) -> Self {
        self.worker_count = worker_count.max(1);
        self
    }

    /// Cap the worker count without changing lower values.
    #[must_use]
    pub fn with_max_workers(mut self, max_workers: usize) -> Self {
        self.worker_count = self.worker_count.min(max_workers.max(1));
        self
    }

    /// Set the bounded input-channel capacity. Values below 1 are clamped to 1.
    #[must_use]
    pub fn with_input_capacity(mut self, input_capacity: usize) -> Self {
        self.input_capacity = input_capacity.max(1);
        self
    }

    /// Set the bounded output-channel capacity. Values below 1 are clamped to 1.
    #[must_use]
    pub fn with_output_capacity(mut self, output_capacity: usize) -> Self {
        self.output_capacity = output_capacity.max(1);
        self
    }

    /// Set the OS thread name prefix used for producer and worker threads.
    #[must_use]
    pub fn with_thread_name_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.thread_name_prefix = prefix.into();
        if self.thread_name_prefix.is_empty() {
            self.thread_name_prefix = "nw-jobs-pipeline".to_string();
        }
        self
    }

    /// Effective worker count after clamping.
    #[must_use]
    pub fn worker_count(&self) -> usize {
        self.worker_count.max(1)
    }

    /// Bounded input-channel capacity after clamping.
    #[must_use]
    pub fn input_capacity(&self) -> usize {
        self.input_capacity.max(1)
    }

    /// Bounded output-channel capacity after clamping.
    #[must_use]
    pub fn output_capacity(&self) -> usize {
        self.output_capacity.max(1)
    }

    /// Thread name prefix.
    #[must_use]
    pub fn thread_name_prefix(&self) -> &str {
        &self.thread_name_prefix
    }
}

/// Logical stage that produced a pipeline error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingPipelineStage {
    /// Input iterator/producer stage.
    Producer,
    /// Worker transform stage.
    Worker,
    /// Caller-owned output consumer stage.
    Consumer,
}

impl fmt::Display for BlockingPipelineStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Producer => f.write_str("producer"),
            Self::Worker => f.write_str("worker"),
            Self::Consumer => f.write_str("consumer"),
        }
    }
}

/// Error returned by a blocking pipeline.
#[derive(Debug)]
pub enum BlockingPipelineError<E> {
    /// Domain error from one of the caller-provided stages.
    Stage {
        stage: BlockingPipelineStage,
        source: E,
    },
    /// OS thread creation failed.
    ThreadSpawn {
        stage: BlockingPipelineStage,
        source: std::io::Error,
    },
    /// A pipeline thread panicked.
    ThreadPanicked { stage: BlockingPipelineStage },
}

impl<E: fmt::Display> fmt::Display for BlockingPipelineError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stage { stage, source } => {
                write!(f, "blocking pipeline {stage} failed: {source}")
            }
            Self::ThreadSpawn { stage, source } => {
                write!(f, "failed to spawn blocking pipeline {stage}: {source}")
            }
            Self::ThreadPanicked { stage } => {
                write!(f, "blocking pipeline {stage} thread panicked")
            }
        }
    }
}

impl<E> Error for BlockingPipelineError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Stage { source, .. } => Some(source),
            Self::ThreadSpawn { source, .. } => Some(source),
            Self::ThreadPanicked { .. } => None,
        }
    }
}

/// Counts collected during a completed or cancelled pipeline run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockingPipelineSummary {
    pub produced: usize,
    pub consumed: usize,
    pub workers: usize,
    pub cancelled: bool,
}

enum PipelineOutput<O, E> {
    Item(O),
    Error {
        stage: BlockingPipelineStage,
        source: E,
    },
}

/// Stream an input iterator through bounded worker threads and consume outputs
/// on the current thread.
///
/// `inputs` is moved to a producer thread. `work` runs on worker threads and may
/// return `Ok(None)` to drop an item. `consume` runs on the caller thread; this
/// is intentional so the caller can perform serialized side effects such as DB
/// writes without sharing that state across workers.
///
/// # Errors
///
/// Returns the first domain, thread-spawn, or panic error observed. On error the
/// supplied cancellation token is set; in-flight work may finish, but no new
/// work is started after cancellation is observed.
pub fn try_for_each_bounded_blocking<I, O, E, Inputs, Work, Consume, C>(
    config: BlockingPipelineConfig,
    cancel: C,
    inputs: Inputs,
    work: Work,
    mut consume: Consume,
) -> Result<BlockingPipelineSummary, BlockingPipelineError<E>>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
    Inputs: IntoIterator<Item = Result<I, E>> + Send + 'static,
    Inputs::IntoIter: Send,
    Work: Fn(I, &C) -> Result<Option<O>, E> + Send + Sync + 'static,
    Consume: FnMut(O) -> Result<(), E>,
    C: Cancellation,
{
    let worker_count = config.worker_count();
    let produced = Arc::new(AtomicUsize::new(0));

    let (input_tx, input_rx) = bounded::<I>(config.input_capacity());
    let (output_tx, output_rx) = bounded::<PipelineOutput<O, E>>(config.output_capacity());

    let producer = spawn_producer(
        &config,
        cancel.clone(),
        inputs,
        input_tx,
        output_tx.clone(),
        Arc::clone(&produced),
    )?;

    let work = Arc::new(work);
    let mut workers = Vec::with_capacity(worker_count);
    for index in 0..worker_count {
        match spawn_worker(
            &config,
            index,
            cancel.clone(),
            input_rx.clone(),
            output_tx.clone(),
            Arc::clone(&work),
        ) {
            Ok(worker) => workers.push(worker),
            Err(source) => {
                cancel.cancel();
                drop(input_rx);
                drop(output_tx);
                drop(output_rx);
                let _ = producer.join();
                join_all(workers);
                return Err(BlockingPipelineError::ThreadSpawn {
                    stage: BlockingPipelineStage::Worker,
                    source,
                });
            }
        }
    }
    drop(input_rx);
    drop(output_tx);

    let mut consumed = 0usize;
    let mut first_error = None;
    while let Ok(output) = output_rx.recv() {
        match output {
            PipelineOutput::Item(item) => match consume(item) {
                Ok(()) => consumed += 1,
                Err(source) => {
                    cancel.cancel();
                    first_error = Some(BlockingPipelineError::Stage {
                        stage: BlockingPipelineStage::Consumer,
                        source,
                    });
                    break;
                }
            },
            PipelineOutput::Error { stage, source } => {
                cancel.cancel();
                first_error = Some(BlockingPipelineError::Stage { stage, source });
                break;
            }
        }
    }
    drop(output_rx);

    let panic_error = join_pipeline_threads(producer, workers);
    if let Some(error) = first_error {
        return Err(error);
    }
    if let Some(error) = panic_error {
        return Err(error);
    }

    Ok(BlockingPipelineSummary {
        produced: produced.load(Ordering::Relaxed),
        consumed,
        workers: worker_count,
        cancelled: cancel.is_cancelled(),
    })
}

fn spawn_producer<I, O, E, Inputs, C>(
    config: &BlockingPipelineConfig,
    cancel: C,
    inputs: Inputs,
    input_tx: Sender<I>,
    output_tx: Sender<PipelineOutput<O, E>>,
    produced: Arc<AtomicUsize>,
) -> Result<JoinHandle<()>, BlockingPipelineError<E>>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
    Inputs: IntoIterator<Item = Result<I, E>> + Send + 'static,
    Inputs::IntoIter: Send,
    C: Cancellation,
{
    thread::Builder::new()
        .name(format!("{}-producer", config.thread_name_prefix()))
        .spawn(move || {
            for input in inputs {
                if cancel.is_cancelled() {
                    break;
                }
                let item = match input {
                    Ok(item) => item,
                    Err(source) => {
                        cancel.cancel();
                        let _ = output_tx.send(PipelineOutput::Error {
                            stage: BlockingPipelineStage::Producer,
                            source,
                        });
                        break;
                    }
                };
                if !send_input(&cancel, &input_tx, item) {
                    break;
                }
                produced.fetch_add(1, Ordering::Relaxed);
            }
        })
        .map_err(|source| BlockingPipelineError::ThreadSpawn {
            stage: BlockingPipelineStage::Producer,
            source,
        })
}

fn spawn_worker<I, O, E, Work, C>(
    config: &BlockingPipelineConfig,
    index: usize,
    cancel: C,
    input_rx: Receiver<I>,
    output_tx: Sender<PipelineOutput<O, E>>,
    work: Arc<Work>,
) -> Result<JoinHandle<()>, std::io::Error>
where
    I: Send + 'static,
    O: Send + 'static,
    E: Send + 'static,
    Work: Fn(I, &C) -> Result<Option<O>, E> + Send + Sync + 'static,
    C: Cancellation,
{
    thread::Builder::new()
        .name(format!("{}-worker-{index}", config.thread_name_prefix()))
        .spawn(move || {
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                let item = match recv_input(&cancel, &input_rx) {
                    Some(item) => item,
                    None => break,
                };
                match work(item, &cancel) {
                    Ok(Some(output)) => {
                        if !send_output(&cancel, &output_tx, PipelineOutput::Item(output)) {
                            break;
                        }
                    }
                    Ok(None) => {}
                    Err(source) => {
                        cancel.cancel();
                        let _ = output_tx.send(PipelineOutput::Error {
                            stage: BlockingPipelineStage::Worker,
                            source,
                        });
                        break;
                    }
                }
            }
        })
}

fn send_input<I, C>(cancel: &C, tx: &Sender<I>, item: I) -> bool
where
    C: Cancellation,
{
    let mut pending = item;
    loop {
        if cancel.is_cancelled() {
            return false;
        }
        match tx.send_timeout(pending, CHANNEL_POLL_INTERVAL) {
            Ok(()) => return true,
            Err(SendTimeoutError::Timeout(item)) => pending = item,
            Err(SendTimeoutError::Disconnected(_)) => return false,
        }
    }
}

fn send_output<O, E, C>(
    cancel: &C,
    tx: &Sender<PipelineOutput<O, E>>,
    output: PipelineOutput<O, E>,
) -> bool
where
    C: Cancellation,
{
    let mut pending = output;
    loop {
        if cancel.is_cancelled() {
            return false;
        }
        match tx.send_timeout(pending, CHANNEL_POLL_INTERVAL) {
            Ok(()) => return true,
            Err(SendTimeoutError::Timeout(output)) => pending = output,
            Err(SendTimeoutError::Disconnected(_)) => return false,
        }
    }
}

fn recv_input<I, C>(cancel: &C, rx: &Receiver<I>) -> Option<I>
where
    C: Cancellation,
{
    loop {
        if cancel.is_cancelled() {
            return None;
        }
        match rx.recv_timeout(CHANNEL_POLL_INTERVAL) {
            Ok(item) => return Some(item),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return None,
        }
    }
}

fn join_pipeline_threads<E>(
    producer: JoinHandle<()>,
    workers: Vec<JoinHandle<()>>,
) -> Option<BlockingPipelineError<E>> {
    let mut panic_error = if producer.join().is_err() {
        Some(BlockingPipelineError::ThreadPanicked {
            stage: BlockingPipelineStage::Producer,
        })
    } else {
        None
    };
    for worker in workers {
        if worker.join().is_err() && panic_error.is_none() {
            panic_error = Some(BlockingPipelineError::ThreadPanicked {
                stage: BlockingPipelineStage::Worker,
            });
        }
    }
    panic_error
}

fn join_all(workers: Vec<JoinHandle<()>>) {
    for worker in workers {
        let _ = worker.join();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    use super::*;
    use crate::CancellationToken;

    #[test]
    fn blocking_pipeline_streams_outputs_to_consumer() {
        let cancel = CancellationToken::new();
        let mut outputs = Vec::new();
        let summary = try_for_each_bounded_blocking(
            BlockingPipelineConfig::new(2)
                .with_input_capacity(2)
                .with_output_capacity(2)
                .with_thread_name_prefix("nw-jobs-test"),
            cancel,
            (0..8).map(Ok::<_, &'static str>),
            |item, _cancel| Ok(Some(item * 2)),
            |item| {
                outputs.push(item);
                Ok(())
            },
        )
        .unwrap();

        outputs.sort_unstable();
        assert_eq!(outputs, vec![0, 2, 4, 6, 8, 10, 12, 14]);
        assert_eq!(summary.produced, 8);
        assert_eq!(summary.consumed, 8);
        assert_eq!(summary.workers, 2);
    }

    #[test]
    fn blocking_pipeline_reports_worker_errors_and_cancels() {
        let cancel = CancellationToken::new();
        let observed = cancel.clone();
        let error = try_for_each_bounded_blocking(
            BlockingPipelineConfig::new(2),
            cancel,
            (0..10).map(Ok::<_, &'static str>),
            |item, _cancel| {
                if item == 3 {
                    Err("boom")
                } else {
                    Ok(Some(item))
                }
            },
            |_item| Ok(()),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            BlockingPipelineError::Stage {
                stage: BlockingPipelineStage::Worker,
                source: "boom"
            }
        ));
        assert!(observed.is_cancelled());
    }

    #[test]
    fn blocking_pipeline_reports_consumer_errors_and_unblocks_workers() {
        let cancel = CancellationToken::new();
        let observed = cancel.clone();
        let error = try_for_each_bounded_blocking(
            BlockingPipelineConfig::new(4)
                .with_input_capacity(1)
                .with_output_capacity(1),
            cancel,
            (0..10_000).map(Ok::<_, &'static str>),
            |item, _cancel| Ok(Some(item)),
            |_item| Err("consumer failed"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            BlockingPipelineError::Stage {
                stage: BlockingPipelineStage::Consumer,
                source: "consumer failed"
            }
        ));
        assert!(observed.is_cancelled());
    }

    #[test]
    fn blocking_pipeline_uses_each_worker() {
        let cancel = CancellationToken::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let seen_workers = Arc::clone(&seen);
        let summary = try_for_each_bounded_blocking(
            BlockingPipelineConfig::new(3),
            cancel,
            (0..64).map(Ok::<_, &'static str>),
            move |item, _cancel| {
                let name = thread::current().name().unwrap_or("unnamed").to_string();
                seen_workers.lock().unwrap().push(name);
                Ok(Some(item))
            },
            |_item| Ok(()),
        )
        .unwrap();

        assert_eq!(summary.workers, 3);
        let workers = seen.lock().unwrap();
        assert!(
            workers
                .iter()
                .all(|name| name.starts_with("nw-jobs-pipeline-worker-"))
        );
    }

    #[test]
    fn blocking_pipeline_drops_items_returned_as_none() {
        let cancel = CancellationToken::new();
        let consumed = AtomicUsize::new(0);
        let summary = try_for_each_bounded_blocking(
            BlockingPipelineConfig::new(2),
            cancel,
            (0..10).map(Ok::<_, &'static str>),
            |item, _cancel| Ok((item % 2 == 0).then_some(item)),
            |_item| {
                consumed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(summary.produced, 10);
        assert_eq!(summary.consumed, 5);
        assert_eq!(consumed.load(Ordering::Relaxed), 5);
    }
}
