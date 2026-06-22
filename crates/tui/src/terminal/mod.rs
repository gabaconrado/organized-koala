//! The crossterm driver: raw-mode setup/teardown and the non-blocking poll loop.
//!
//! This is the only layer that touches the real terminal. It translates crossterm key events
//! into the app core's transport-agnostic [`Event`]s (the mapping is context-sensitive: a
//! letter is a command on the task list but typed text in a form) and renders each frame via
//! [`crate::ui::draw`]. The mapping function [`map_key`] is pure so the keybindings can be
//! pinned by tests.
//!
//! The loop never blocks on I/O (ADR-0006 Model A): it polls the terminal for input with a short
//! tick timeout, drains the worker thread's response channel, and redraws every tick so a
//! spinner animates and cancel/quit stay live while a request is outstanding. All request
//! execution happens on the [`worker`](crate::client::worker) thread.

use std::io::{self, Stdout};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;

use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, ClientResponse, Dispatch, Event, Screen};
use crate::ui;

/// The poll-loop tick: how long each iteration waits for input before redrawing. Bounds input
/// latency and sets the spinner cadence. `tui-dev`'s call (ADR-0006 assumptions); ~12.5 fps.
const TICK: Duration = Duration::from_millis(80);

/// Whether the app is currently in a text-entry context, where letters are typed rather than
/// interpreted as commands.
fn is_text_entry(screen: &Screen) -> bool {
    match screen {
        Screen::Auth(_) => true,
        Screen::TaskList(list) => list.adding.is_some(),
        Screen::Offline { .. } => false,
    }
}

/// Translate a crossterm key into an app [`Event`] given the current screen.
///
/// Returns `None` for keys that are not bound in the current context. The mapping:
///
/// - `Esc` / `Ctrl+C` → [`Event::Quit`] (and in the add-task flow `Esc` is [`Event::Cancel`]).
/// - `Enter` → [`Event::Submit`].
/// - `Tab` / `Down` → [`Event::Next`]; `BackTab` / `Up` → [`Event::Prev`].
/// - `Backspace` → [`Event::Backspace`].
/// - `F2` (auth screen) → [`Event::ToggleAuthMode`].
/// - In a text-entry context, a printable key → [`Event::Char`].
/// - On the task list (not entering text): `a` → [`Event::BeginAddTask`], `c` →
///   [`Event::CloseSelected`], `r` → [`Event::Refresh`], `q` → [`Event::Quit`].
/// - On the offline screen: `r` → [`Event::Refresh`].
///
/// While a request is outstanding, `Esc` maps to [`Event::Cancel`] (abandon the request) rather
/// than `Quit`, so cancel stays live; `Ctrl+C` always quits.
#[must_use]
pub fn map_key(screen: &Screen, key: KeyEvent) -> Option<Event> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Event::Quit);
    }

    let text_entry = is_text_entry(screen);
    let in_add_task = matches!(screen, Screen::TaskList(list) if list.adding.is_some());
    let pending = is_pending(screen);

    match key.code {
        KeyCode::Esc => {
            if in_add_task || pending {
                Some(Event::Cancel)
            } else {
                Some(Event::Quit)
            }
        }
        KeyCode::Enter => Some(Event::Submit),
        KeyCode::Tab | KeyCode::Down => Some(Event::Next),
        KeyCode::BackTab | KeyCode::Up => Some(Event::Prev),
        KeyCode::Backspace => Some(Event::Backspace),
        KeyCode::F(2) if matches!(screen, Screen::Auth(_)) => Some(Event::ToggleAuthMode),
        KeyCode::Char(c) if text_entry => Some(Event::Char(c)),
        KeyCode::Char('a') if matches!(screen, Screen::TaskList(_)) => Some(Event::BeginAddTask),
        KeyCode::Char('c') if matches!(screen, Screen::TaskList(_)) => Some(Event::CloseSelected),
        KeyCode::Char('r') => match screen {
            Screen::TaskList(_) | Screen::Offline { .. } => Some(Event::Refresh),
            Screen::Auth(_) => None,
        },
        KeyCode::Char('q') if matches!(screen, Screen::TaskList(_)) => Some(Event::Quit),
        _ => None,
    }
}

/// Whether the given screen has a request outstanding.
fn is_pending(screen: &Screen) -> bool {
    match screen {
        Screen::Auth(auth) => auth.is_pending(),
        Screen::TaskList(list) => list.is_pending(),
        Screen::Offline { pending, .. } => pending.is_some(),
    }
}

/// A live terminal handle owning raw mode and the alternate screen; restores both on drop.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn enter() -> anyhow::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Runs the interactive poll loop until the app requests a quit, restoring the terminal on exit
/// (including on error, via the guard's `Drop`).
///
/// Requests are dispatched to the worker over `requests`; completed responses are drained from
/// `responses`. The loop never blocks on I/O: each tick it polls input, applies any worker
/// responses (re-dispatching any chained follow-up), and redraws — so the UI stays live and the
/// spinner animates while a request is outstanding. On quit, the worker thread is detached and
/// the process exits (the worker holds no state needing flush — hard-constraint #1).
///
/// # Errors
///
/// Returns an error if terminal setup, drawing, or reading input fails.
pub fn run(
    mut app: App,
    requests: Sender<Dispatch>,
    responses: Receiver<ClientResponse>,
) -> anyhow::Result<()> {
    let mut guard = TerminalGuard::enter()?;
    let mut tick: u64 = 0;
    while !app.should_quit() {
        let _frame = guard.terminal.draw(|frame| ui::draw(frame, &app, tick))?;
        tick = tick.wrapping_add(1);

        // Input: poll with the tick timeout so the loop wakes to redraw even with no keypress.
        if event::poll(TICK)?
            && let CtEvent::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
            && let Some(mapped) = map_key(app.screen(), key)
            && let Some(dispatch) = app.handle_event(mapped)
        {
            // The worker outliving the UI is impossible to recover from; treat a closed
            // request channel as a fatal transport failure.
            requests
                .send(dispatch)
                .map_err(|_| anyhow::anyhow!("request worker stopped unexpectedly"))?;
        }

        // Drain any completed worker responses, re-dispatching chained follow-ups.
        loop {
            match responses.try_recv() {
                Ok(response) => {
                    if let Some(dispatch) = app.apply_response(response) {
                        requests
                            .send(dispatch)
                            .map_err(|_| anyhow::anyhow!("request worker stopped unexpectedly"))?;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    anyhow::bail!("request worker stopped unexpectedly");
                }
            }
        }
    }
    Ok(())
}
