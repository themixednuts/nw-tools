use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use serde::Serialize;

const UNKNOWN_PERCENT: u8 = u8::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodegenStatusKind {
    Started,
    Progress,
    Finished,
}

impl CodegenStatusKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Progress => "progress",
            Self::Finished => "finished",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodegenStatusEvent {
    pub kind: CodegenStatusKind,
    pub phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
}

pub trait CodegenStatusSink: Send + Sync + 'static {
    fn emit(&self, event: &CodegenStatusEvent);
}

#[derive(Clone)]
pub struct CodegenStatus {
    sink: Arc<dyn CodegenStatusSink>,
}

impl CodegenStatus {
    #[must_use]
    pub fn new(sink: impl CodegenStatusSink) -> Self {
        Self {
            sink: Arc::new(sink),
        }
    }

    #[must_use]
    pub fn phase(&self, name: impl Into<String>, total: Option<u64>) -> CodegenStatusPhase {
        let phase = CodegenStatusPhase {
            status: self.clone(),
            state: Arc::new(CodegenStatusPhaseState {
                name: name.into(),
                total,
                current: AtomicU64::new(0),
                last_percent: AtomicU8::new(UNKNOWN_PERCENT),
            }),
        };
        phase.emit(CodegenStatusKind::Started, 0, None, true);
        phase
    }

    pub fn event(&self, kind: CodegenStatusKind, phase: impl Into<String>) {
        self.emit(CodegenStatusEvent {
            kind,
            phase: phase.into(),
            message: None,
            current: None,
            total: None,
            percent: None,
        });
    }

    fn emit(&self, event: CodegenStatusEvent) {
        let kind = event.kind.as_str();
        let phase = event.phase.as_str();
        let message = event.message.as_deref().unwrap_or("");
        tracing::info!(
            target: "nw_serialize_codegen::status",
            kind,
            phase,
            message,
            current = event.current,
            total = event.total,
            percent = event.percent,
        );
        self.sink.emit(&event);
    }
}

impl Default for CodegenStatus {
    fn default() -> Self {
        Self::new(NoopStatusSink)
    }
}

impl fmt::Debug for CodegenStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodegenStatus").finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct NoopStatusSink;

impl CodegenStatusSink for NoopStatusSink {
    fn emit(&self, _event: &CodegenStatusEvent) {}
}

#[derive(Debug, Clone)]
pub struct CodegenStatusPhase {
    status: CodegenStatus,
    state: Arc<CodegenStatusPhaseState>,
}

impl CodegenStatusPhase {
    pub fn advance(&self, delta: u64) {
        self.advance_with_message(delta, None::<String>);
    }

    pub fn advance_with_message(&self, delta: u64, message: impl Into<Option<String>>) {
        let current = self.state.current.fetch_add(delta, Ordering::Relaxed) + delta;
        self.emit(CodegenStatusKind::Progress, current, message.into(), false);
    }

    pub fn finish(&self) {
        self.finish_with_message(None::<String>);
    }

    pub fn finish_with_message(&self, message: impl Into<Option<String>>) {
        let current = self
            .state
            .total
            .unwrap_or_else(|| self.state.current.load(Ordering::Relaxed));
        self.state.current.store(current, Ordering::Relaxed);
        self.emit(CodegenStatusKind::Finished, current, message.into(), true);
    }

    fn emit(&self, kind: CodegenStatusKind, current: u64, message: Option<String>, force: bool) {
        let percent = self.state.total.map(|total| percent(current, total));
        if !force && let Some(percent) = percent {
            let previous = self.state.last_percent.swap(percent, Ordering::Relaxed);
            if previous == percent {
                return;
            }
        }

        self.status.emit(CodegenStatusEvent {
            kind,
            phase: self.state.name.clone(),
            message,
            current: self.state.total.map(|_| current),
            total: self.state.total,
            percent,
        });
    }
}

#[derive(Debug)]
struct CodegenStatusPhaseState {
    name: String,
    total: Option<u64>,
    current: AtomicU64,
    last_percent: AtomicU8,
}

fn percent(current: u64, total: u64) -> u8 {
    if total == 0 {
        return 100;
    }
    let current = current.min(total);
    ((current.saturating_mul(100)) / total).min(100) as u8
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Debug, Default)]
    struct CaptureSink {
        events: Mutex<Vec<CodegenStatusEvent>>,
    }

    impl CodegenStatusSink for Arc<CaptureSink> {
        fn emit(&self, event: &CodegenStatusEvent) {
            self.events
                .lock()
                .expect("events mutex")
                .push(event.clone());
        }
    }

    #[test]
    fn phase_events_are_emitted_at_percent_boundaries() {
        let sink = Arc::new(CaptureSink::default());
        let status = CodegenStatus::new(Arc::clone(&sink));
        let phase = status.phase("network rust", Some(4));

        phase.advance(1);
        phase.advance(1);
        phase.advance(0);
        phase.finish();

        let events = sink.events.lock().expect("events mutex");
        assert_eq!(
            events.iter().map(|event| event.kind).collect::<Vec<_>>(),
            vec![
                CodegenStatusKind::Started,
                CodegenStatusKind::Progress,
                CodegenStatusKind::Progress,
                CodegenStatusKind::Finished,
            ]
        );
        assert_eq!(events[0].percent, Some(0));
        assert_eq!(events[1].percent, Some(25));
        assert_eq!(events[2].percent, Some(50));
        assert_eq!(events[3].percent, Some(100));
    }
}
