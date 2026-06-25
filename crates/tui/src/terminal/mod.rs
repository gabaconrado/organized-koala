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

/// Coarse timer-session refresh cadence, in poll-loop ticks. At ~80 ms per tick, 750 ticks ≈ 60 s
/// (ADR-0006 §8.4): while a post-auth screen is shown and the timer is idle, re-`GetTimerSession`
/// this often so the server's running/completed verdict stays reasonably fresh — far above the
/// ~80 ms render tick that animates the local countdown, and well clear of per-second polling
/// (ADR-0002 §3, ADR-0006). Raised from ~5 s on human feedback (Board 0008): a coarser cadence
/// removes the flicker class and the running→completed verdict may lag up to ~1 min, which is
/// cosmetic (the local countdown already shows `00:00`).
const TIMER_REFRESH_TICKS: u64 = 750;

/// Whether the app is currently in a text-entry context, where letters are typed rather than
/// interpreted as commands. The duration-edit sub-flow (`editing_duration`) is a global text-entry
/// mode that overlays the active post-auth screen.
fn is_text_entry(screen: &Screen, editing_duration: bool) -> bool {
    if editing_duration {
        return true;
    }
    match screen {
        Screen::Auth(_) => true,
        Screen::TaskList(list) => list.adding.is_some() || list.editing.is_some(),
        Screen::Notes(notes) => notes.is_text_entry(),
        Screen::Offline { .. } => false,
    }
}

/// Translate a crossterm key into an app [`Event`] given the current screen and whether the
/// global duration-edit sub-flow is active.
///
/// Returns `None` for keys that are not bound in the current context. The mapping:
///
/// - `Esc` / `Ctrl+C` → [`Event::Quit`] (and in a sub-flow / while in flight `Esc` is
///   [`Event::Cancel`]).
/// - `Enter` → [`Event::Submit`].
/// - `Tab` / `Down` → [`Event::Next`]; `BackTab` / `Up` → [`Event::Prev`].
/// - `Backspace` → [`Event::Backspace`].
/// - `F2` (auth screen) → [`Event::ToggleAuthMode`].
/// - In a text-entry context, a printable key → [`Event::Char`].
/// - On the task list (not entering text): `a` → [`Event::BeginAddTask`], `e` →
///   [`Event::BeginEditTask`], `c` → [`Event::ToggleDone`], `x` → [`Event::DeleteSelected`],
///   `n` → [`Event::OpenNotes`], `r` → [`Event::Refresh`], `q` → [`Event::Quit`].
/// - On the notes list (idle, not entering text): `a` → [`Event::BeginAddNote`], `e` →
///   [`Event::BeginEditNote`], `x` → [`Event::BeginDeleteNote`], `Esc` → [`Event::Back`].
/// - On any post-auth screen (not entering text): `p` → [`Event::ToggleTimer`], `d` →
///   [`Event::BeginEditDuration`] (the global timer controls, ADR-0006 §8.2).
/// - On the offline screen: `r` → [`Event::Refresh`].
///
/// While a request is outstanding, `Esc` maps to [`Event::Cancel`] (abandon the request) rather
/// than `Quit`, so cancel stays live; `Ctrl+C` always quits.
#[must_use]
pub fn map_key(screen: &Screen, editing_duration: bool, key: KeyEvent) -> Option<Event> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Event::Quit);
    }

    let text_entry = is_text_entry(screen, editing_duration);
    let in_task_form = matches!(
        screen,
        Screen::TaskList(list) if list.adding.is_some() || list.editing.is_some()
    );
    let in_notes_sub_flow = matches!(screen, Screen::Notes(notes) if notes.in_sub_flow());
    let pending = is_pending(screen);
    // A sub-flow (add/edit-task, a notes sub-flow, or duration-edit) or an in-flight request
    // makes `Esc` mean cancel.
    let in_sub_flow = in_task_form || in_notes_sub_flow || editing_duration;
    let on_task_list = matches!(screen, Screen::TaskList(_));
    let on_notes = matches!(screen, Screen::Notes(_));
    let post_auth = on_task_list || on_notes;

    match key.code {
        KeyCode::Esc => {
            if in_sub_flow || pending {
                Some(Event::Cancel)
            } else if on_notes {
                // Idle notes list: `Esc` returns to the task list (Assumption A7).
                Some(Event::Back)
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
        KeyCode::Char('a') if on_task_list => Some(Event::BeginAddTask),
        KeyCode::Char('e') if on_task_list => Some(Event::BeginEditTask),
        KeyCode::Char('c') if on_task_list => Some(Event::ToggleDone),
        KeyCode::Char('x') if on_task_list => Some(Event::DeleteSelected),
        // `n` opens the notes view from the task list (Assumption A7).
        KeyCode::Char('n') if on_task_list => Some(Event::OpenNotes),
        // Notes-list commands (idle list, not a text-entry sub-flow): create / edit / delete.
        KeyCode::Char('a') if on_notes => Some(Event::BeginAddNote),
        KeyCode::Char('e') if on_notes => Some(Event::BeginEditNote),
        KeyCode::Char('x') if on_notes => Some(Event::BeginDeleteNote),
        // The global timer controls are live on every post-auth screen (not while a text-entry
        // sub-flow owns the keystroke — Assumption B4).
        KeyCode::Char('p') if post_auth => Some(Event::ToggleTimer),
        KeyCode::Char('d') if post_auth => Some(Event::BeginEditDuration),
        KeyCode::Char('r') => match screen {
            Screen::TaskList(_) | Screen::Notes(_) | Screen::Offline { .. } => Some(Event::Refresh),
            Screen::Auth(_) => None,
        },
        KeyCode::Char('q') if post_auth => Some(Event::Quit),
        _ => None,
    }
}

/// Whether the given screen has a request outstanding.
fn is_pending(screen: &Screen) -> bool {
    match screen {
        Screen::Auth(auth) => auth.is_pending(),
        Screen::TaskList(list) => list.is_pending(),
        Screen::Notes(notes) => notes.is_pending(),
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

        // The account-global timer loads once a post-auth screen is shown (ADR-0006 §8.1), so the
        // bottom-right widget reflects a server response from the first frame after login.
        if let Some(dispatch) = app.load_timer_if_needed() {
            send(&requests, dispatch)?;
        }

        // Input: poll with the tick timeout so the loop wakes to redraw even with no keypress.
        if event::poll(TICK)?
            && let CtEvent::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
            && let Some(mapped) = map_key(app.screen(), app.is_editing_duration(), key)
            && let Some(dispatch) = app.handle_event(mapped)
        {
            send(&requests, dispatch)?;
        }

        // Coarse timer-session refresh: while a post-auth screen is shown and the timer is idle,
        // re-pull the session every `TIMER_REFRESH_TICKS` so the server's running/completed
        // verdict stays current (the local countdown already animates each tick). Never per
        // second (ADR-0006 §8.4).
        if tick != 0
            && tick.is_multiple_of(TIMER_REFRESH_TICKS)
            && let Some(dispatch) = app.refresh_timer()
        {
            send(&requests, dispatch)?;
        }

        // Drain any completed worker responses, re-dispatching chained follow-ups.
        loop {
            match responses.try_recv() {
                Ok(response) => {
                    if let Some(dispatch) = app.apply_response(response) {
                        send(&requests, dispatch)?;
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

/// Send a dispatch to the worker, treating a closed channel as a fatal transport failure (the
/// worker outliving the UI is impossible to recover from).
fn send(requests: &Sender<Dispatch>, dispatch: Dispatch) -> anyhow::Result<()> {
    requests
        .send(dispatch)
        .map_err(|_| anyhow::anyhow!("request worker stopped unexpectedly"))
}
