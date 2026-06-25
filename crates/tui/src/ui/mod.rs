//! Rendering: pure draw functions from an [`App`] onto a `ratatui` frame.
//!
//! No state lives here — every widget is derived from the current [`Screen`] and the global
//! [`Timer`]. Splitting rendering out from the app core lets the same draw path run against a
//! `TestBackend` for buffer-snapshot assertions (ADR-0003).

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{
    App, AuthField, AuthMode, AuthState, NotesMode, NotesState, Screen, TaskListState, Timer,
};
use contract::{Note, TaskStatus, TimerSession};

/// The frames of the in-flight spinner, cycled by the poll loop's tick counter.
const SPINNER_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

/// The base hotkey caption for the task-list screen (idle, not in a sub-flow). The global timer
/// keys (`p`, `d`) are shown on every post-auth screen so they are discoverable (ADR-0006 §8.2).
/// ` | `-separated so wrap points fall on separators. The phrasing is kept compact so that — once
/// the in-flight spinner + ` (Esc to cancel)` affordance is appended — the wrapped caption stays
/// within the bottom band at the 80×24 test viewport without clipping the cancel affordance
/// (ADR-0006 §8.3, learned 0010).
const TASK_LIST_CAPTION: &str =
    "a: add | e: edit | c: done | x: del | n: notes | p: timer | d: dur | r: refresh | q: quit";

/// The base hotkey caption for the notes screen (idle list). Mirrors the task-list caption with
/// the notes commands (`a` create, `e` edit, `x` delete, Enter open) and an `Esc` back-to-tasks.
/// Kept short enough that the caption plus the appended spinner + cancel affordance stays within
/// the bottom band at the 80×24 viewport (ADR-0006 §8.3).
const NOTES_CAPTION: &str = "a: add | e: edit | x: del | Enter: open | Esc: back | p: timer | d: dur | r: refresh | q: quit";

/// Height (rows) of the bottom band on a post-auth screen: the hotkey caption (which may wrap)
/// on the left and the global timer widget on the right. Three rows so the wrapped caption plus
/// the appended in-flight spinner + cancel affordance stays fully visible at the 80×24 viewport
/// without clipping the cancel affordance (ADR-0006 §8.3).
const BOTTOM_BAND_ROWS: u16 = 3;

/// The spinner glyph for the given tick, or empty when not pending. Pure so the spinner cadence
/// is testable independently of the real loop's timing.
#[must_use]
pub fn spinner_frame(tick: u64) -> &'static str {
    // `SPINNER_FRAMES.len()` is 4; index via the u64 tick without a lossy `as` conversion.
    let i = usize::try_from(tick % 4).unwrap_or(0);
    SPINNER_FRAMES.get(i).copied().unwrap_or("|")
}

/// The hotkey caption with the in-flight spinner **appended** (ADR-0006 §8.3): while a request is
/// outstanding, a trailing spinner glyph is added to the end of the stable caption rather than
/// replacing it, so the caption never flickers. The "Esc to cancel" affordance is appended
/// alongside the spinner. When idle, the base caption is returned unchanged.
#[must_use]
pub fn caption_with_spinner(base: &str, pending: bool, tick: u64) -> String {
    if pending {
        format!("{base}   {} (Esc to cancel)", spinner_frame(tick))
    } else {
        base.to_owned()
    }
}

/// The single-line label for the global timer widget rendered bottom-right on every post-auth
/// screen (ADR-0006 §8.1): `idle` + configured duration, the live `MM:SS` countdown when running
/// (recomputed each render tick from the absolute `ends_at` — render, not state, so #1 holds), or
/// `completed`.
#[must_use]
pub fn timer_widget_label(timer: &Timer) -> String {
    match &timer.session {
        TimerSession::Idle => format!("timer idle · {} min", timer.config.duration_minutes),
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
                "timer 00:00 (completing…)".to_owned()
            } else {
                format!("timer {label}")
            }
        }
        TimerSession::Completed { .. } => "timer completed".to_owned(),
    }
}

/// Draws the whole application for the current frame, dispatching on the active screen. `tick`
/// drives the in-flight spinner animation; it is ignored when no request is outstanding.
pub fn draw(frame: &mut Frame, app: &App, tick: u64) {
    match app.screen() {
        Screen::Auth(auth) => draw_auth(frame, auth, tick),
        Screen::TaskList(list) => {
            let profile = app.session().map_or("", |s| s.profile_name.as_str());
            draw_task_list(frame, list, app.timer(), profile, tick);
        }
        Screen::Notes(notes) => {
            let profile = app.session().map_or("", |s| s.profile_name.as_str());
            draw_notes(frame, notes, app.timer(), profile, tick);
        }
        Screen::Offline { message, pending } => {
            draw_offline(frame, message, pending.is_some(), tick);
        }
    }
}

fn draw_auth(frame: &mut Frame, auth: &AuthState, tick: u64) {
    let area = frame.area();
    let (title, base_hint) = match auth.mode {
        AuthMode::Login => (
            "Login",
            "Enter: submit  Tab: next field  F2: switch to register  Esc/Ctrl+C: quit",
        ),
        AuthMode::Register => (
            "Register",
            "Enter: submit  Tab: next field  F2: switch to login  Esc/Ctrl+C: quit",
        ),
    };
    let hint = caption_with_spinner(base_hint, auth.is_pending(), tick);

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

fn draw_task_list(
    frame: &mut Frame,
    list: &TaskListState,
    timer: &Timer,
    profile: &str,
    tick: u64,
) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(BOTTOM_BAND_ROWS),
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
        let text = if let Some(edit) = &timer.editing {
            // The duration-edit sub-flow overlays the active screen's message line (ADR-0006 §8).
            let err = edit.error.as_deref().unwrap_or("");
            format!("Set duration (min): {}  {err}", edit.buffer)
        } else if let Some(add) = &list.adding {
            let field = if add.on_title { "Title" } else { "Description" };
            let err = add.error.as_deref().unwrap_or("");
            format!(
                "Add task — {field}: title='{}' desc='{}'  {err}",
                add.title, add.description
            )
        } else if let Some(edit) = &list.editing {
            let field = if edit.on_title {
                "Title"
            } else {
                "Description"
            };
            let err = edit.error.as_deref().unwrap_or("");
            format!(
                "Edit task — {field}: title='{}' desc='{}'  {err}",
                edit.title, edit.description
            )
        } else if list.confirming_delete.is_some() {
            "Delete this task? Press x again to confirm, any other key to cancel.".to_owned()
        } else if let Some(msg) = &timer.message {
            msg.clone()
        } else {
            list.message.clone().unwrap_or_default()
        };
        frame.render_widget(
            Paragraph::new(Span::raw(text)).wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(3) {
        let base = if timer.editing.is_some() {
            "Enter: save  Esc: cancel"
        } else if list.adding.is_some() || list.editing.is_some() {
            "Enter: save  Tab: switch field  Esc: cancel"
        } else {
            TASK_LIST_CAPTION
        };
        // The spinner is appended (never replaces the caption) and reflects either the screen's
        // request or the global timer's request being in flight (ADR-0006 §8.3).
        let pending = list.is_pending() || timer.is_pending();
        let caption = caption_with_spinner(base, pending, tick);
        draw_bottom_row(frame, *slot, &caption, timer);
    }
}

/// Render the active profile's notes view: the list with title + created_at (newest-first as
/// returned), or the open create/edit/view/delete sub-flow, plus the shared bottom row.
fn draw_notes(frame: &mut Frame, notes: &NotesState, timer: &Timer, profile: &str, tick: u64) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(BOTTOM_BAND_ROWS),
        ])
        .split(area);

    if let Some(slot) = chunks.first() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("organized-koala — notes [{profile}]"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(1) {
        // The viewing sub-flow replaces the list with the single note's title + body; otherwise the
        // list of notes is shown with its selection highlight.
        if let NotesMode::Viewing(note) = &notes.mode {
            let body = vec![
                Line::from(Span::styled(
                    note.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::raw(format_created_at(note))),
                Line::from(""),
                Line::from(Span::raw(note.content.clone())),
            ];
            frame.render_widget(
                Paragraph::new(body)
                    .block(Block::default().borders(Borders::ALL).title("Note"))
                    .wrap(Wrap { trim: false }),
                *slot,
            );
        } else {
            let items: Vec<ListItem> = notes
                .notes
                .iter()
                .map(|note| {
                    ListItem::new(Line::from(format!(
                        "{}  ({})",
                        note.title,
                        format_created_at(note)
                    )))
                })
                .collect();
            let widget = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Notes"))
                .highlight_symbol("> ")
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            let mut state = ListState::default();
            state.select(notes.selected);
            frame.render_stateful_widget(widget, *slot, &mut state);
        }
    }

    if let Some(slot) = chunks.get(2) {
        let text = if let Some(edit) = &timer.editing {
            // The duration-edit sub-flow overlays the active screen's message line (ADR-0006 §8).
            let err = edit.error.as_deref().unwrap_or("");
            format!("Set duration (min): {}  {err}", edit.buffer)
        } else {
            note_message_line(notes)
        };
        frame.render_widget(
            Paragraph::new(Span::raw(text)).wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(3) {
        let base = notes_caption_base(notes, timer);
        let pending = notes.is_pending() || timer.is_pending();
        let caption = caption_with_spinner(base, pending, tick);
        draw_bottom_row(frame, *slot, &caption, timer);
    }
}

/// The message line for the notes screen: the open create/edit form's fields + inline error, the
/// delete confirmation prompt, the timer message, or the screen's transient message.
fn note_message_line(notes: &NotesState) -> String {
    match &notes.mode {
        NotesMode::Creating(form) => {
            let field = if form.on_title { "Title" } else { "Content" };
            let err = form.error.as_deref().unwrap_or("");
            format!(
                "New note — {field}: title='{}' content='{}'  {err}",
                form.title, form.content
            )
        }
        NotesMode::Editing { form, .. } => {
            let field = if form.on_title { "Title" } else { "Content" };
            let err = form.error.as_deref().unwrap_or("");
            format!(
                "Edit note — {field}: title='{}' content='{}'  {err}",
                form.title, form.content
            )
        }
        NotesMode::ConfirmingDelete { title, .. } => {
            format!("Delete note '{title}'? Enter: confirm  Esc: cancel")
        }
        NotesMode::List | NotesMode::Viewing(_) => notes.message.clone().unwrap_or_default(),
    }
}

/// The base hotkey caption for the notes screen, varying by the open sub-flow.
fn notes_caption_base(notes: &NotesState, timer: &Timer) -> &'static str {
    if timer.editing.is_some() {
        "Enter: save  Esc: cancel"
    } else {
        match &notes.mode {
            NotesMode::Creating(_) | NotesMode::Editing { .. } => {
                "Enter: save  Tab: switch field  Esc: cancel"
            }
            NotesMode::ConfirmingDelete { .. } => "Enter: confirm delete  Esc: cancel",
            NotesMode::Viewing(_) => "Esc: back to list",
            NotesMode::List => NOTES_CAPTION,
        }
    }
}

/// Format a note's `created_at` for display, at the render seam, from the DTO's `DateTime`
/// (hard-constraint A8: the `tui` crate keeps no direct `chrono` dependency — this calls a method
/// on the contract DTO's type, exactly as the timer countdown derives epoch seconds).
fn format_created_at(note: &Note) -> String {
    note.created_at.format("%Y-%m-%d %H:%M UTC").to_string()
}

/// The bottom row of a post-auth screen: the hotkey caption on the left and the persistent global
/// timer widget on the right (ADR-0006 §8.1).
fn draw_bottom_row(frame: &mut Frame, area: Rect, caption: &str, timer: &Timer) {
    let label = timer_widget_label(timer);
    // Reserve the right column for the timer label (+2 padding); the caption takes the rest.
    let right = u16::try_from(label.chars().count() + 2).unwrap_or(u16::MAX);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right)])
        .split(area);

    if let Some(slot) = columns.first() {
        frame.render_widget(
            Paragraph::new(Span::raw(caption.to_owned())).wrap(Wrap { trim: true }),
            *slot,
        );
    }
    if let Some(slot) = columns.get(1) {
        frame.render_widget(
            Paragraph::new(Span::styled(
                label,
                Style::default().add_modifier(Modifier::BOLD),
            )),
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
/// reaches zero the label is `00:00` and the caller shows a local "completing" hint until the
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

fn draw_offline(frame: &mut Frame, message: &str, pending: bool, tick: u64) {
    let area = frame.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Server unreachable");
    let base = "Press r to retry, or Esc/Ctrl+C to quit.";
    let action = caption_with_spinner(base, pending, tick);
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
