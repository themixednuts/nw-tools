//! Full-screen lifecycle and event loop shared by every interactive browser.
//! Sets up raw mode + the alternate screen, pumps key events into a [`View`],
//! and restores the terminal on exit (including on error or panic-free quit).

use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// What a [`View`] wants the event loop to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// An interactive screen driven by [`run`].
pub trait View {
    /// Draw the current state.
    fn render(&mut self, frame: &mut Frame);
    /// Handle a key press; return [`Flow::Quit`] to exit.
    fn on_key(&mut self, key: KeyEvent) -> Flow;
    /// A line to print to stdout after the browser closes (e.g. a selection).
    fn take_result(&mut self) -> Option<String> {
        None
    }

    /// Whether the view wants periodic ticks (poll-based redraws) — used while
    /// background work (e.g. a cross-sheet index) is still building.
    fn ticking(&self) -> bool {
        false
    }

    /// Called on each poll timeout while [`View::ticking`] is true.
    fn tick(&mut self) {}

    /// How long to wait for input before [`View::tick`] while [`View::ticking`].
    /// A view animating (e.g. a sprite) returns a shorter interval to redraw at
    /// its frame rate; the default suits background-progress polling.
    fn poll_interval(&self) -> Duration {
        Duration::from_millis(120)
    }

    /// A region that must be force-repainted on the next frame (not the whole
    /// screen). Needed when a graphics-protocol image changes in place — e.g.
    /// cycling a DDS texture's mip — since those cells would otherwise leave
    /// residue. Returns the area to repaint, or `None`.
    fn needs_clear(&mut self) -> Option<ratatui::layout::Rect> {
        None
    }
}

/// Run `view` to completion on the alternate screen, restoring the terminal
/// afterwards. The caller decides what to do with any recorded selection. The
/// terminal is restored even if the view panics (via [`Session`]'s `Drop`).
pub fn run<V: View>(view: &mut V) -> io::Result<()> {
    let mut session = session()?;
    session.run(view)
}

/// A held-open alternate-screen session. Run several views back-to-back through
/// one session and the terminal is entered/left exactly once, so switching views
/// (e.g. picker → grid → picker) repaints in place with no flicker. The terminal
/// is restored when the session is dropped.
pub struct Session {
    terminal: Terminal<Backend>,
}

/// Open an interactive session. Restored on drop.
pub fn session() -> io::Result<Session> {
    Ok(Session { terminal: enter()? })
}

impl Session {
    /// Drive `view` until it quits, keeping the screen alive afterwards.
    pub fn run<V: View>(&mut self, view: &mut V) -> io::Result<()> {
        event_loop(&mut self.terminal, view)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = leave(&mut self.terminal);
    }
}

type Backend = CrosstermBackend<Stdout>;

fn enter() -> io::Result<Terminal<Backend>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide) {
        let _ = disable_raw_mode();
        return Err(error);
    }
    Terminal::new(CrosstermBackend::new(stdout))
}

fn leave(terminal: &mut Terminal<Backend>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    terminal.show_cursor()
}

fn event_loop<V: View>(terminal: &mut Terminal<Backend>, view: &mut V) -> io::Result<()> {
    loop {
        // Force just this region to repaint next frame (resetting the prior buffer
        // there), so an in-place image change doesn't leave graphics residue —
        // without the flicker of clearing the whole screen.
        if let Some(area) = view.needs_clear() {
            let buffer = terminal.current_buffer_mut();
            for y in area.top()..area.bottom() {
                for x in area.left()..area.right() {
                    if let Some(cell) = buffer.cell_mut((x, y)) {
                        cell.reset();
                    }
                }
            }
        }
        terminal.draw(|frame| view.render(frame))?;
        if view.ticking() {
            // Wake at the view's cadence so background progress and animations render.
            if event::poll(view.poll_interval())? {
                if handle(view, event::read()?) == Flow::Quit {
                    return Ok(());
                }
            } else {
                view.tick();
            }
        } else if handle(view, event::read()?) == Flow::Quit {
            return Ok(());
        }
    }
}

fn handle<V: View>(view: &mut V, event: Event) -> Flow {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => view.on_key(key),
        _ => Flow::Continue,
    }
}
