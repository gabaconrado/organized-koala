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

use crate::app::{App, AuthField, AuthMode, AuthState, Screen, TaskListState};
use contract::TaskStatus;

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
