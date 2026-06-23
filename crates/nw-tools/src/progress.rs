use std::fmt;
use std::io::{self, IsTerminal};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph, Widget};

const DRAW_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone)]
pub(crate) struct Progress {
    enabled: bool,
}

pub(crate) struct Batch {
    session: Option<Arc<Session>>,
    show_jobs: bool,
}

#[derive(Clone)]
pub(crate) struct Job {
    session: Option<Arc<Session>>,
    id: usize,
    show_jobs: bool,
    finished: Arc<AtomicBool>,
}

pub(crate) struct Stage {
    session: Option<Arc<Session>>,
}

struct Session {
    terminal: Mutex<Terminal<CrosstermBackend<io::Stderr>>>,
    model: Mutex<Model>,
    last_draw: Mutex<Instant>,
    active: AtomicBool,
}

enum Model {
    Batch(BatchModel),
    Stage(StageModel),
}

struct BatchModel {
    label: String,
    total: usize,
    done: usize,
    failed: usize,
    current: String,
    jobs: Vec<JobRow>,
}

struct JobRow {
    id: usize,
    label: String,
    len: usize,
    pos: usize,
    state: JobState,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum JobState {
    Queued,
    Running,
    Done,
    Failed,
}

struct StageModel {
    label: String,
    message: String,
}

#[derive(Debug, Clone, Copy)]
struct JobColumns {
    state: usize,
    progress: usize,
    files: usize,
    path: usize,
}

impl Progress {
    pub(crate) fn auto(enabled: bool) -> Self {
        Self {
            enabled: enabled && io::stderr().is_terminal(),
        }
    }

    pub(crate) fn batch(&self, label: impl Into<String>, total: usize) -> Batch {
        self.batch_with_detail(label, total, true)
    }

    pub(crate) fn batch_compact(&self, label: impl Into<String>, total: usize) -> Batch {
        self.batch_with_detail(label, total, false)
    }

    fn batch_with_detail(&self, label: impl Into<String>, total: usize, show_jobs: bool) -> Batch {
        if !self.enabled {
            return Batch::disabled();
        }

        let model = Model::Batch(BatchModel {
            label: clean(label),
            total,
            done: 0,
            failed: 0,
            current: "queued".to_string(),
            jobs: Vec::new(),
        });

        let session = Session::open(model).ok();
        if let Some(session) = &session {
            session.draw_now(show_jobs);
        }

        Batch { session, show_jobs }
    }

    pub(crate) fn stage(&self, label: impl Into<String>) -> Stage {
        if !self.enabled {
            return Stage::disabled();
        }

        let model = Model::Stage(StageModel {
            label: clean(label),
            message: "running".to_string(),
        });

        let session = Session::open(model).ok();
        if let Some(session) = &session {
            session.draw_now(false);
        }

        Stage { session }
    }
}

impl Batch {
    fn disabled() -> Self {
        Self {
            session: None,
            show_jobs: false,
        }
    }

    pub(crate) fn job(&self, label: impl Into<String>) -> Job {
        let Some(session) = &self.session else {
            return Job::disabled();
        };

        let id = session.add_job(clean(label));
        session.draw(self.show_jobs);

        Job {
            session: Some(Arc::clone(session)),
            id,
            show_jobs: self.show_jobs,
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn finish(&self) {
        if let Some(session) = &self.session {
            session.finish("done", self.show_jobs);
        }
    }
}

impl Job {
    fn disabled() -> Self {
        Self {
            session: None,
            id: 0,
            show_jobs: false,
            finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn set_len(&self, len: usize) {
        if let Some(session) = &self.session {
            session.update_job(self.id, self.show_jobs, |job| {
                job.len = len;
                job.state = JobState::Running;
            });
        }
    }

    pub(crate) fn inc(&self, delta: u64) {
        let delta = usize::try_from(delta).unwrap_or(usize::MAX);
        if let Some(session) = &self.session {
            session.update_job(self.id, self.show_jobs, |job| {
                job.pos = job.pos.saturating_add(delta).min(job.len);
                job.state = JobState::Running;
            });
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

        if let Some(session) = &self.session {
            let state = if message == "failed" {
                JobState::Failed
            } else {
                JobState::Done
            };
            session.finish_job(self.id, self.show_jobs, state);
        }
    }
}

impl Stage {
    fn disabled() -> Self {
        Self { session: None }
    }

    pub(crate) fn finish(&self, message: &'static str) {
        if let Some(session) = &self.session {
            session.finish(message, false);
        }
    }
}

impl Session {
    fn open(model: Model) -> io::Result<Arc<Self>> {
        let mut stderr = io::stderr();
        execute!(stderr, EnterAlternateScreen, Hide)?;
        let backend = CrosstermBackend::new(stderr);
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => {
                let mut stderr = io::stderr();
                let _ = execute!(stderr, Show, LeaveAlternateScreen);
                return Err(error);
            }
        };
        if let Err(error) = terminal.clear() {
            let _ = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen);
            return Err(error);
        }

        Ok(Arc::new(Self {
            terminal: Mutex::new(terminal),
            model: Mutex::new(model),
            last_draw: Mutex::new(Instant::now() - DRAW_INTERVAL),
            active: AtomicBool::new(true),
        }))
    }

    fn add_job(&self, label: String) -> usize {
        let mut model = self.model();
        let Model::Batch(batch) = &mut *model else {
            return 0;
        };

        let id = batch.jobs.len();
        batch.current.clone_from(&label);
        batch.jobs.push(JobRow {
            id,
            label,
            len: 0,
            pos: 0,
            state: JobState::Queued,
        });
        id
    }

    fn update_job(&self, id: usize, show_jobs: bool, update: impl FnOnce(&mut JobRow)) {
        {
            let mut model = self.model();
            if let Model::Batch(batch) = &mut *model
                && let Some(job) = batch.jobs.iter_mut().find(|job| job.id == id)
            {
                update(job);
                batch.current.clone_from(&job.label);
            }
        }
        self.draw(show_jobs);
    }

    fn finish_job(&self, id: usize, show_jobs: bool, state: JobState) {
        {
            let mut model = self.model();
            if let Model::Batch(batch) = &mut *model
                && let Some(job) = batch.jobs.iter_mut().find(|job| job.id == id)
            {
                if job.len == 0 {
                    job.len = 1;
                }
                job.pos = job.len;
                job.state = state;
                batch.done = batch.done.saturating_add(1).min(batch.total);
                if state == JobState::Failed {
                    batch.failed = batch.failed.saturating_add(1);
                }
                batch.current.clone_from(&job.label);
            }
        }
        self.draw(show_jobs);
    }

    fn finish(&self, message: &str, show_jobs: bool) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }

        {
            let mut model = self.model();
            match &mut *model {
                Model::Batch(batch) => batch.current = message.to_string(),
                Model::Stage(stage) => stage.message = message.to_string(),
            }
        }
        self.draw_now(show_jobs);
        if !self.active.swap(false, Ordering::AcqRel) {
            return;
        }
        self.restore();
    }

    fn draw(&self, show_jobs: bool) {
        let mut last_draw = self
            .last_draw
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if last_draw.elapsed() < DRAW_INTERVAL {
            return;
        }
        *last_draw = Instant::now();
        drop(last_draw);

        self.draw_now(show_jobs);
    }

    fn draw_now(&self, show_jobs: bool) {
        if !self.active.load(Ordering::Acquire) {
            return;
        }

        let model = self.model();
        let mut terminal = self.terminal();
        let _ = terminal.draw(|frame| match &*model {
            Model::Batch(batch) => draw_batch(frame.area(), frame.buffer_mut(), batch, show_jobs),
            Model::Stage(stage) => draw_stage(frame.area(), frame.buffer_mut(), stage),
        });
    }

    fn restore(&self) {
        let mut terminal = self.terminal();
        let _ = terminal.clear();
        let _ = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = terminal.show_cursor();
    }

    fn model(&self) -> MutexGuard<'_, Model> {
        self.model.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn terminal(&self) -> MutexGuard<'_, Terminal<CrosstermBackend<io::Stderr>>> {
        self.terminal
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.active.swap(false, Ordering::AcqRel)
            && let Ok(mut terminal) = self.terminal.lock()
        {
            let _ = terminal.clear();
            let _ = execute!(terminal.backend_mut(), Show, LeaveAlternateScreen);
            let _ = terminal.show_cursor();
        }
    }
}

impl fmt::Debug for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Progress")
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl fmt::Debug for Batch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Batch")
            .field("enabled", &self.session.is_some())
            .finish()
    }
}

impl fmt::Debug for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Job")
            .field("enabled", &self.session.is_some())
            .finish()
    }
}

impl fmt::Debug for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stage")
            .field("enabled", &self.session.is_some())
            .finish()
    }
}

fn draw_batch(area: Rect, buf: &mut ratatui::buffer::Buffer, batch: &BatchModel, show_jobs: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(area);

    let title = Line::from(vec![
        Span::styled("nw-tools ", Style::default().fg(Color::Cyan)),
        Span::styled(
            batch.label.as_str(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]);
    let block = Block::default().title(title).borders(Borders::ALL);
    let ratio = ratio(batch.done, batch.total);
    Gauge::default()
        .block(block)
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ratio)
        .label(format!(
            "{:.0}%  {}/{}",
            ratio * 100.0,
            batch.done,
            batch.total
        ))
        .render(chunks[0], buf);

    let running = batch
        .jobs
        .iter()
        .filter(|job| job.state == JobState::Running)
        .count();
    let queued = batch.total.saturating_sub(batch.jobs.len()).saturating_add(
        batch
            .jobs
            .iter()
            .filter(|job| job.state == JobState::Queued)
            .count(),
    );
    let status = status_line(
        chunks[1].width.saturating_sub(2).into(),
        running,
        queued,
        batch.done,
        batch.failed,
        &batch.current,
    );
    Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .render(chunks[1], buf);

    if show_jobs {
        draw_jobs(chunks[2], buf, batch);
    } else {
        Paragraph::new("per-file rows hidden for compact progress")
            .block(Block::default().borders(Borders::ALL).title("Jobs"))
            .render(chunks[2], buf);
    }
}

fn draw_jobs(area: Rect, buf: &mut ratatui::buffer::Buffer, batch: &BatchModel) {
    let visible = usize::from(area.height.saturating_sub(3));
    let start = batch.jobs.len().saturating_sub(visible);
    let columns = JobColumns::new(area.width.saturating_sub(2).into());
    let mut lines = Vec::with_capacity(visible.saturating_add(1));
    lines.push(Line::styled(
        job_line(columns, "State", "Progress", "Files", "Path"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    lines.extend(batch.jobs[start..].iter().map(|job| {
        Line::from(job_line(
            columns,
            job_state(job.state),
            &job_bar(job, columns.progress),
            &format!("{}/{}", job.pos, job.len),
            &job.label,
        ))
    }));

    Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Jobs"))
        .render(area, buf);
}

fn draw_stage(area: Rect, buf: &mut ratatui::buffer::Buffer, stage: &StageModel) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);
    Paragraph::new(stage.message.as_str())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("nw-tools {}", stage.label)),
        )
        .render(chunks[0], buf);
}

impl JobColumns {
    fn new(width: usize) -> Self {
        let state = 7;
        let files = 11;
        let mut progress = if width >= 86 {
            22
        } else if width >= 72 {
            18
        } else {
            12
        };
        let gaps = 6;
        let path = width.saturating_sub(state + progress + files + gaps);
        if path < 18 {
            let recovered = (18 - path).min(progress.saturating_sub(8));
            progress -= recovered;
        }
        let path = width.saturating_sub(state + progress + files + gaps);

        Self {
            state,
            progress,
            files,
            path,
        }
    }
}

fn status_line(
    width: usize,
    running: usize,
    queued: usize,
    done: usize,
    failed: usize,
    current: &str,
) -> String {
    let base = format!("running {running}  pending {queued}  done {done}  failed {failed}");
    let prefix = format!("{base}  current ");
    if prefix.chars().count() >= width {
        return fit_end(&base, width);
    }

    let current_width = width.saturating_sub(prefix.chars().count());
    if current_width < 8 {
        fit_end(&base, width)
    } else {
        format!("{prefix}{}", fit_middle(current, current_width))
    }
}

fn job_line(columns: JobColumns, state: &str, progress: &str, files: &str, path: &str) -> String {
    format!(
        "{:<state_width$}  {:<progress_width$}  {:>files_width$}  {}",
        fit_end(state, columns.state),
        fit_end(progress, columns.progress),
        fit_end(files, columns.files),
        fit_middle(path, columns.path),
        state_width = columns.state,
        progress_width = columns.progress,
        files_width = columns.files,
    )
}

fn job_bar(job: &JobRow, width: usize) -> String {
    if width < 7 {
        return fit_end(&format!("{:.0}%", ratio(job.pos, job.len) * 100.0), width);
    }
    let inner_width = width.saturating_sub(2);
    if job.len == 0 {
        return format!("[{}]", ".".repeat(inner_width));
    }

    let filled = job.pos.saturating_mul(inner_width) / job.len.max(1);
    format!(
        "[{}{}]",
        "#".repeat(filled.min(inner_width)),
        ".".repeat(inner_width.saturating_sub(filled))
    )
}

fn job_state(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "queued",
        JobState::Running => "running",
        JobState::Done => "done",
        JobState::Failed => "failed",
    }
}

fn ratio(done: usize, total: usize) -> f64 {
    if total == 0 {
        1.0
    } else {
        done as f64 / total as f64
    }
}

fn clean(value: impl Into<String>) -> String {
    value.into().replace(['\r', '\n', '\t'], " ")
}

fn fit_end(value: &str, width: usize) -> String {
    let value = clean(value);
    let count = value.chars().count();
    if count <= width {
        return value;
    }

    match width {
        0 => String::new(),
        1 => ".".to_string(),
        2 => "..".to_string(),
        3 => "...".to_string(),
        _ => {
            let keep = width - 3;
            format!("{}...", value.chars().take(keep).collect::<String>())
        }
    }
}

fn fit_middle(value: &str, width: usize) -> String {
    let value = clean(value);
    let count = value.chars().count();
    if count <= width {
        return value;
    }

    match width {
        0 => String::new(),
        1 => ".".to_string(),
        2 => "..".to_string(),
        3 => "...".to_string(),
        _ => {
            let content = width - 3;
            let head = content / 2;
            let tail = content - head;
            let start = value.chars().take(head).collect::<String>();
            let end = value
                .chars()
                .rev()
                .take(tail)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>();
            format!("{start}...{end}")
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    #[test]
    #[ignore = "visual dump for checking terminal progress layout"]
    fn dump_progress_layout() {
        println!("\nwide:\n{}", render(96, 18, true).join("\n"));
        println!("\nnarrow:\n{}", render(64, 14, true).join("\n"));
        println!("\ncompact:\n{}", render(80, 10, false).join("\n"));
    }

    #[test]
    fn progress_layout_renders_at_common_sizes() {
        for (width, height, show_jobs) in [
            (96, 18, true),
            (80, 12, true),
            (64, 14, true),
            (64, 8, false),
        ] {
            let view = render(width, height, show_jobs);
            assert_eq!(view.len(), usize::from(height));
            assert!(
                view.iter()
                    .all(|line| line.chars().count() == usize::from(width))
            );
            assert!(view.iter().any(|line| line.contains("nw-tools")));
            assert!(view.iter().any(|line| line.contains("Status")));
        }
    }

    fn render(width: u16, height: u16, show_jobs: bool) -> Vec<String> {
        let mut terminal =
            Terminal::new(TestBackend::new(width, height)).expect("test terminal opens");
        let batch = sample_batch();
        terminal
            .draw(|frame| draw_batch(frame.area(), frame.buffer_mut(), &batch, show_jobs))
            .expect("draw succeeds");
        buffer_lines(terminal.backend(), width, height)
    }

    fn buffer_lines(backend: &TestBackend, width: u16, height: u16) -> Vec<String> {
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| backend.buffer()[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    fn sample_batch() -> BatchModel {
        BatchModel {
            label: "pak extract".to_string(),
            total: 42,
            done: 19,
            failed: 1,
            current: "sharedassets/coatlicue/newworld_vitaeeterna/slices/extremely/long/path/with/objectstream.asset".to_string(),
            jobs: vec![
                JobRow {
                    id: 0,
                    label: "assets/pakchunk0.pak".to_string(),
                    len: 1200,
                    pos: 1200,
                    state: JobState::Done,
                },
                JobRow {
                    id: 1,
                    label: "sharedassets/coatlicue/newworld_vitaeeterna/slices/territories/everfall/structure/slice_01.datasheet".to_string(),
                    len: 2400,
                    pos: 992,
                    state: JobState::Running,
                },
                JobRow {
                    id: 2,
                    label: "objects/weapons/sword/textures/long_material_name_diff.dds.3".to_string(),
                    len: 301,
                    pos: 48,
                    state: JobState::Running,
                },
                JobRow {
                    id: 3,
                    label: "objects/characters/player/male/animations/idle.motion".to_string(),
                    len: 1,
                    pos: 0,
                    state: JobState::Queued,
                },
                JobRow {
                    id: 4,
                    label: "bad/file/objectstream.asset".to_string(),
                    len: 1,
                    pos: 1,
                    state: JobState::Failed,
                },
            ],
        }
    }
}
