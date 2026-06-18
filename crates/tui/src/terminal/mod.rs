//! The crossterm driver: raw-mode setup/teardown and the blocking input loop.
//!
//! This is the only layer that touches the real terminal. It translates crossterm key events
//! into the app core's transport-agnostic [`Event`]s (the mapping is context-sensitive: a
//! letter is a command on the task list but typed text in a form) and renders each frame via
//! [`crate::ui::draw`]. The mapping function [`map_key`] is pure so the keybindings can be
//! pinned by tests.

use std::io::{self, Stdout};

use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, Event, Screen};
use crate::client::Client;
use crate::ui;

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
#[must_use]
pub fn map_key(screen: &Screen, key: KeyEvent) -> Option<Event> {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Event::Quit);
    }

    let text_entry = is_text_entry(screen);
    let in_add_task = matches!(screen, Screen::TaskList(list) if list.adding.is_some());

    match key.code {
        KeyCode::Esc => {
            if in_add_task {
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

/// Runs the interactive event loop until the app requests a quit, restoring the terminal on
/// exit (including on error, via the guard's `Drop`).
///
/// # Errors
///
/// Returns an error if terminal setup, drawing, or reading input fails.
pub fn run<C: Client>(mut app: App<C>) -> anyhow::Result<()> {
    let mut guard = TerminalGuard::enter()?;
    while !app.should_quit() {
        let _frame = guard.terminal.draw(|frame| ui::draw(frame, &app))?;
        if let CtEvent::Key(key) = event::read()?
            && key.kind == event::KeyEventKind::Press
            && let Some(mapped) = map_key(app.screen(), key)
        {
            app.handle_event(mapped);
        }
    }
    Ok(())
}
