//! Rendering: pure draw functions from an [`App`] onto a `ratatui` frame.
//!
//! No state lives here — every widget is derived from the current [`Screen`]. Splitting
//! rendering out from the app core lets the same draw path run against a `TestBackend` for
//! buffer-snapshot assertions (ADR-0003).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, AuthField, AuthMode, AuthState, Screen, TaskListState, TimerState};
use contract::{TaskStatus, TimerSession};

/// The frames of the in-flight spinner, cycled by the poll loop's tick counter.
const SPINNER_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

/// The spinner glyph for the given tick, or empty when not pending. Pure so the spinner cadence
/// is testable independently of the real loop's timing.
#[must_use]
pub fn spinner_frame(tick: u64) -> &'static str {
    // `SPINNER_FRAMES.len()` is 4; index via the u64 tick without a lossy `as` conversion.
    let i = usize::try_from(tick % 4).unwrap_or(0);
    SPINNER_FRAMES.get(i).copied().unwrap_or("|")
}

/// Draws the whole application for the current frame, dispatching on the active screen. `tick`
/// drives the in-flight spinner animation; it is ignored when no request is outstanding.
pub fn draw(frame: &mut Frame, app: &App, tick: u64) {
    match app.screen() {
        Screen::Auth(auth) => draw_auth(frame, auth, tick),
        Screen::TaskList(list) => {
            let profile = app.session().map_or("", |s| s.profile_name.as_str());
            draw_task_list(frame, list, profile, tick);
        }
        Screen::Timer(timer) => draw_timer(frame, timer, tick),
        Screen::Offline { message, pending } => {
            draw_offline(frame, message, pending.is_some(), tick);
        }
    }
}

/// The "working…" hint shown while a request is outstanding, with the animated spinner glyph.
fn working_hint(tick: u64) -> String {
    format!("{} working… (Esc to cancel)", spinner_frame(tick))
}

fn draw_auth(frame: &mut Frame, auth: &AuthState, tick: u64) {
    let area = frame.area();
    let working = working_hint(tick);
    let (title, hint) = match auth.mode {
        AuthMode::Login => (
            "Login",
            if auth.is_pending() {
                working.as_str()
            } else {
                "Enter: submit  Tab: next field  F2: switch to register  Esc/Ctrl+C: quit"
            },
        ),
        AuthMode::Register => (
            "Register",
            if auth.is_pending() {
                working.as_str()
            } else {
                "Enter: submit  Tab: next field  F2: switch to login  Esc/Ctrl+C: quit"
            },
        ),
    };

    let fields: Vec<(&str, &str, bool, bool)> = match auth.mode {
        AuthMode::Login => vec![
            (
                "Identifier",
                auth.identifier.as_str(),
                auth.focus == AuthField::Identifier,
                false,
            ),
            (
                "Password",
                auth.password.as_str(),
                auth.focus == AuthField::Password,
                true,
            ),
        ],
        AuthMode::Register => vec![
            (
                "Username",
                auth.username.as_str(),
                auth.focus == AuthField::Username,
                false,
            ),
            (
                "Email",
                auth.email.as_str(),
                auth.focus == AuthField::Email,
                false,
            ),
            (
                "Password",
                auth.password.as_str(),
                auth.focus == AuthField::Password,
                true,
            ),
            (
                "Profile name",
                auth.profile_name.as_str(),
                auth.focus == AuthField::ProfileName,
                false,
            ),
        ],
    };

    let mut constraints = vec![Constraint::Length(1)];
    constraints.extend(std::iter::repeat_n(Constraint::Length(3), fields.len()));
    constraints.push(Constraint::Length(2));
    constraints.push(Constraint::Min(0));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(area);

    if let Some(slot) = chunks.first() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("organized-koala — {title}"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            *slot,
        );
    }

    for (i, (label, value, focused, masked)) in fields.iter().enumerate() {
        let Some(slot) = chunks.get(i + 1) else {
            continue;
        };
        draw_field(frame, *slot, label, value, *focused, *masked);
    }

    let msg_idx = fields.len() + 1;
    if let Some(slot) = chunks.get(msg_idx)
        && let Some(err) = &auth.error
    {
        frame.render_widget(
            Paragraph::new(Span::raw(err.clone())).wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(msg_idx + 1) {
        frame.render_widget(
            Paragraph::new(Span::raw(hint)).wrap(Wrap { trim: true }),
            *slot,
        );
    }
}

fn draw_field(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    focused: bool,
    masked: bool,
) {
    let shown = if masked {
        "*".repeat(value.chars().count())
    } else {
        value.to_owned()
    };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(label.to_owned());
    if focused {
        block = block.border_style(Style::default().add_modifier(Modifier::BOLD));
    }
    frame.render_widget(Paragraph::new(shown).block(block), area);
}

fn draw_task_list(frame: &mut Frame, list: &TaskListState, profile: &str, tick: u64) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(area);

    if let Some(slot) = chunks.first() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("organized-koala — tasks [{profile}]"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(1) {
        let items: Vec<ListItem> = list
            .tasks
            .iter()
            .map(|task| {
                let marker = match task.status {
                    TaskStatus::Done => "[x]",
                    TaskStatus::Open => "[ ]",
                };
                ListItem::new(Line::from(format!("{marker} {}", task.title)))
            })
            .collect();
        let widget = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Tasks"))
            .highlight_symbol("> ")
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        let mut state = ListState::default();
        state.select(list.selected);
        frame.render_stateful_widget(widget, *slot, &mut state);
    }

    if let Some(slot) = chunks.get(2) {
        let text = if let Some(add) = &list.adding {
            let field = if add.on_title { "Title" } else { "Description" };
            let err = add.error.as_deref().unwrap_or("");
            format!(
                "Add task — {field}: title='{}' desc='{}'  {err}",
                add.title, add.description
            )
        } else {
            list.message.clone().unwrap_or_default()
        };
        frame.render_widget(
            Paragraph::new(Span::raw(text)).wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(3) {
        let working = working_hint(tick);
        let hint = if list.is_pending() {
            working.as_str()
        } else if list.adding.is_some() {
            "Enter: save  Tab: switch field  Esc: cancel"
        } else {
            "a: add  c: mark done  Up/Down: move  r: refresh  q: quit"
        };
        frame.render_widget(
            Paragraph::new(Span::raw(hint)).wrap(Wrap { trim: true }),
            *slot,
        );
    }
}

/// The live `MM:SS` countdown label for a running session, computed from Unix-epoch seconds.
///
/// Pure render derivation (ADR-0002 §2–3, hard-constraint #1): the remaining time is recomputed
/// from the absolute `ends_at_secs` and the server's `server_now_secs` advanced by
/// `since_response` (the whole seconds elapsed locally since the running response was applied) —
/// never stored. `remaining = ends_at − (server_now + since_response)`, floored at zero. When it
/// reaches zero the label is `00:00` and the caller shows a local "completed" hint until the
/// server's authoritative `Completed` verdict arrives on the next coarse refresh.
///
/// Taking epoch seconds (not a `chrono` type) keeps this fn — and the `tui` crate — free of a
/// direct `chrono` dependency; the caller derives the seconds from the `contract` DTO's
/// `DateTime` via `timestamp()`.
#[must_use]
pub fn countdown_label(ends_at_secs: i64, server_now_secs: i64, since_response: i64) -> String {
    let remaining = (ends_at_secs - server_now_secs - since_response).max(0);
    let minutes = remaining / 60;
    let seconds = remaining % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn draw_timer(frame: &mut Frame, timer: &TimerState, tick: u64) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(area);

    if let Some(slot) = chunks.first() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "organized-koala — focus timer",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(1) {
        let duration = format!("Duration: {} min", timer.config.duration_minutes);
        frame.render_widget(Paragraph::new(Span::raw(duration)), *slot);
    }

    if let Some(slot) = chunks.get(2) {
        let lines = timer_body(timer);
        frame.render_widget(
            Paragraph::new(lines)
                .block(Block::default().borders(Borders::ALL).title("Session"))
                .wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(3) {
        let text = if let Some(edit) = &timer.editing {
            let err = edit.error.as_deref().unwrap_or("");
            format!("Set duration (min): {}  {err}", edit.buffer)
        } else {
            timer.message.clone().unwrap_or_default()
        };
        frame.render_widget(
            Paragraph::new(Span::raw(text)).wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(4) {
        let working = working_hint(tick);
        let hint = if timer.is_pending() {
            working.as_str()
        } else if timer.editing.is_some() {
            "Enter: save  Esc: cancel"
        } else {
            "s: start  x: stop  d: set duration  r: refresh  Esc: back  Ctrl+C: quit"
        };
        frame.render_widget(
            Paragraph::new(Span::raw(hint)).wrap(Wrap { trim: true }),
            *slot,
        );
    }
}

/// The session-state lines for the timer body: idle, the live countdown while running, or the
/// completed verdict. The countdown advances from the locally-elapsed time since the session was
/// applied; reaching zero shows a local "completed" hint pending the server's verdict.
fn timer_body(timer: &TimerState) -> Vec<Line<'static>> {
    match &timer.session {
        TimerSession::Idle => vec![Line::from(Span::raw("Idle — no active session."))],
        TimerSession::Running {
            ends_at,
            server_now,
            ..
        } => {
            let since = timer.applied_at.map_or(0, |t| {
                i64::try_from(t.elapsed().as_secs()).unwrap_or(i64::MAX)
            });
            let label = countdown_label(ends_at.timestamp(), server_now.timestamp(), since);
            if label == "00:00" {
                vec![
                    Line::from(Span::styled(
                        "00:00",
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::raw("Completed (awaiting server confirmation).")),
                ]
            } else {
                vec![
                    Line::from(Span::styled(
                        label,
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::raw("Running.")),
                ]
            }
        }
        TimerSession::Completed { .. } => vec![
            Line::from(Span::styled(
                "00:00",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::raw("Completed.")),
        ],
    }
}

fn draw_offline(frame: &mut Frame, message: &str, pending: bool, tick: u64) {
    let area = frame.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Server unreachable");
    let action = if pending {
        working_hint(tick)
    } else {
        "Press r to retry, or Esc/Ctrl+C to quit.".to_owned()
    };
    let lines = vec![
        Line::from(Span::raw(message.to_owned())),
        Line::from(""),
        Line::from(Span::raw(action)),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}
