//! Rendering: pure draw functions from an [`App`] onto a `ratatui` frame.
//!
//! No state lives here — every widget is derived from the current [`Screen`] and the global
//! [`Timer`]. Splitting rendering out from the app core lets the same draw path run against a
//! `TestBackend` for buffer-snapshot assertions (ADR-0003).
//!
//! Every add/edit/delete-confirm sub-flow, the timer duration edit, and the `?` help reference
//! render as a **centred floating dialog** ([`draw_dialog`], drawn after the main panes so it
//! overlays them; ADR-0010 §3). The post-auth message band then carries only the active pane's
//! transient status/error message. A focused field's border is drawn **purple**
//! ([`Color::Magenta`]) to signal focus, on the auth form and in every dialog.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{
    App, AuthField, AuthMode, AuthState, MainState, NotesMode, NotesState, ProfilesMode,
    ProfilesState, Screen, Tab, TaskListState, Timer,
};
use contract::{Note, TaskStatus, TimerSession};

/// The frames of the in-flight spinner, cycled by the poll loop's tick counter.
const SPINNER_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

/// The single trimmed footer caption for every post-auth pane (ADR-0010 §3, criterion 2): the
/// essentials only — movement, tab switch, quit, and help. The full per-pane hotkey reference now
/// lives in the `?` help overlay ([`draw_help`]), so the footer no longer enumerates the action
/// keys. ` | `-separated so wrap points fall on separators; kept short so that — once the in-flight
/// spinner + ` (Esc to cancel)` affordance is appended — the caption stays within the bottom band
/// at the 80×24 test viewport without clipping the affordance (ADR-0006 §8.3, learned 0010).
const FOOTER_CAPTION: &str = "↑↓: move | Tab/Shift+Tab: switch tab | ?: help | q: quit";

/// Height (rows) of the bottom band on a post-auth screen: the hotkey caption on the left and the
/// global timer widget on the right. A single row — the caption is a single trimmed line (post-0015)
/// and no longer carries the textual cancel affordance, which moved into the `?` help modal
/// (ADR-0006 §8.3, amended 2026-06-26), so the multi-row reservation that existed only to keep the
/// wrapping affordance from being clipped is no longer needed. One row pulled flush to the bottom
/// satisfies ADR-0010 §2's "tight footer" goal; the band does not grow.
const BOTTOM_BAND_ROWS: u16 = 1;

/// The spinner glyph for the given tick, or empty when not pending. Pure so the spinner cadence
/// is testable independently of the real loop's timing.
#[must_use]
pub fn spinner_frame(tick: u64) -> &'static str {
    // `SPINNER_FRAMES.len()` is 4; index via the u64 tick without a lossy `as` conversion.
    let i = usize::try_from(tick % 4).unwrap_or(0);
    SPINNER_FRAMES.get(i).copied().unwrap_or("|")
}

/// The hotkey caption with the in-flight spinner **appended** (ADR-0006 §8.3, amended
/// 2026-06-26): while a request is outstanding, a trailing spinner glyph is added to the end of
/// the stable caption rather than replacing it, so the caption never flickers. The textual cancel
/// affordance is no longer appended here — `Esc` still cancels an in-flight request, but that hint
/// now lives in the `?` help modal so the footer stays a single flush row. When idle, the base
/// caption is returned unchanged.
#[must_use]
pub fn caption_with_spinner(base: &str, pending: bool, tick: u64) -> String {
    if pending {
        format!("{base}   {}", spinner_frame(tick))
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
        Screen::Main(main) => draw_main(frame, main, app, tick),
        Screen::Offline { message, pending } => {
            draw_offline(frame, message, pending.is_some(), tick);
        }
    }
}

/// The post-auth tabbed view (ADR-0010 §1–2): a centred contextual title, the
/// `Tasks | Notes | Profiles` tab bar, the active pane as the main content, a message line, and
/// the footer (caption + timer) pulled flush to the bottom row.
fn draw_main(frame: &mut Frame, main: &MainState, app: &App, tick: u64) {
    // Keep the left/right/top inset but pull the footer flush to the bottom row: a uniform
    // `margin(1)` would leave a blank row below the footer, so the bottom margin is dropped to 0
    // while the top + sides keep their 1-row inset (ADR-0010 §2 "tight footer").
    let full = frame.area();
    let area = Rect {
        x: full.x.saturating_add(1),
        y: full.y.saturating_add(1),
        width: full.width.saturating_sub(2),
        height: full.height.saturating_sub(1),
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            // Title (centred), tab bar, the active pane, the message line, then the flush footer.
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(2),
            Constraint::Length(BOTTOM_BAND_ROWS),
        ])
        .split(area);

    if let Some(slot) = chunks.first() {
        let account = app.session().map_or("", |s| s.account.as_str());
        let profile = app.session().map_or("", |s| s.profile_name.as_str());
        // The exact, load-bearing title format (literal brackets, a hyphen, `organized koala` with
        // a space; ADR-0010 §2): `organized koala - <user> @ [<profile>]`.
        let title = format!("organized koala - {account} @ [{profile}]");
        frame.render_widget(
            Paragraph::new(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(1) {
        frame.render_widget(
            Paragraph::new(tab_bar_line(main.active_tab)).alignment(Alignment::Center),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(2) {
        match main.active_tab {
            Tab::Tasks => draw_task_pane(frame, *slot, &main.tasks),
            Tab::Notes => draw_notes_pane(frame, *slot, &main.notes),
            Tab::Profiles => {
                let active = app.session().map_or("", |s| s.profile_id.as_str());
                draw_profiles_pane(frame, *slot, &main.profiles, active);
            }
        }
    }

    if let Some(slot) = chunks.get(3) {
        let text = main_message_line(main, app.timer());
        frame.render_widget(
            Paragraph::new(Span::raw(text)).wrap(Wrap { trim: true }),
            *slot,
        );
    }

    if let Some(slot) = chunks.get(4) {
        let pending = pane_pending(main) || app.timer().is_pending();
        let caption = caption_with_spinner(FOOTER_CAPTION, pending, tick);
        draw_bottom_row(frame, *slot, &caption, app.timer());
    }

    // Dialogs and the help overlay are drawn last so they float over the panes (ADR-0010 §3).
    if app.help_open() {
        draw_help(frame);
    } else {
        draw_active_dialog(frame, main, app.timer());
    }
}

/// Draw the open overlay dialog (if any) over the active pane. The global duration edit takes
/// precedence (it overlays any pane, ADR-0006 §8); otherwise the active pane's add/edit/confirm
/// sub-flow renders as a dialog. A no-op when no sub-flow is open. Temporary display strings are
/// owned locally so each [`Dialog`] can borrow them.
fn draw_active_dialog(frame: &mut Frame, main: &MainState, timer: &Timer) {
    if let Some(edit) = &timer.editing {
        draw_dialog(
            frame,
            &Dialog {
                title: "Timer duration",
                fields: vec![DialogField {
                    label: "Duration (minutes)",
                    value: &edit.buffer,
                    focused: true,
                    masked: false,
                }],
                body: Vec::new(),
                error: edit.error.as_deref(),
                hint: "Enter: save | Esc: cancel",
            },
        );
        return;
    }
    match main.active_tab {
        Tab::Tasks => draw_task_dialog(frame, &main.tasks),
        Tab::Notes => draw_note_dialog(frame, &main.notes),
        Tab::Profiles => draw_profile_dialog(frame, &main.profiles),
    }
}

/// The task add / edit / delete-confirm dialog, if one is open.
fn draw_task_dialog(frame: &mut Frame, list: &TaskListState) {
    if let Some(add) = &list.adding {
        draw_dialog(
            frame,
            &Dialog {
                title: "Add task",
                fields: vec![
                    DialogField {
                        label: "Title",
                        value: &add.title,
                        focused: add.on_title,
                        masked: false,
                    },
                    DialogField {
                        label: "Description",
                        value: &add.description,
                        focused: !add.on_title,
                        masked: false,
                    },
                ],
                body: Vec::new(),
                error: add.error.as_deref(),
                hint: "Enter: save | Tab: switch field | Esc: cancel",
            },
        );
    } else if let Some(edit) = &list.editing {
        draw_dialog(
            frame,
            &Dialog {
                title: "Edit task",
                fields: vec![
                    DialogField {
                        label: "Title",
                        value: &edit.title,
                        focused: edit.on_title,
                        masked: false,
                    },
                    DialogField {
                        label: "Description",
                        value: &edit.description,
                        focused: !edit.on_title,
                        masked: false,
                    },
                ],
                body: Vec::new(),
                error: edit.error.as_deref(),
                hint: "Enter: save | Tab: switch field | Esc: cancel",
            },
        );
    } else if list.confirming_delete.is_some() {
        draw_dialog(
            frame,
            &Dialog {
                title: "Delete task",
                fields: Vec::new(),
                body: vec![Line::from("Delete this task?")],
                error: None,
                hint: "x: confirm delete | Esc: cancel",
            },
        );
    }
}

/// The note add / edit / delete-confirm dialog, if one is open. The read-only Viewing mode is the
/// 0016 detail view and is **not** a dialog (Assumption A6).
fn draw_note_dialog(frame: &mut Frame, notes: &NotesState) {
    match &notes.mode {
        NotesMode::Creating(form) => draw_dialog(
            frame,
            &Dialog {
                title: "New note",
                fields: vec![
                    DialogField {
                        label: "Title",
                        value: &form.title,
                        focused: form.on_title,
                        masked: false,
                    },
                    DialogField {
                        label: "Content",
                        value: &form.content,
                        focused: !form.on_title,
                        masked: false,
                    },
                ],
                body: Vec::new(),
                error: form.error.as_deref(),
                hint: "Enter: save | Tab: switch field | Esc: cancel",
            },
        ),
        NotesMode::Editing { form, .. } => draw_dialog(
            frame,
            &Dialog {
                title: "Edit note",
                fields: vec![
                    DialogField {
                        label: "Title",
                        value: &form.title,
                        focused: form.on_title,
                        masked: false,
                    },
                    DialogField {
                        label: "Content",
                        value: &form.content,
                        focused: !form.on_title,
                        masked: false,
                    },
                ],
                body: Vec::new(),
                error: form.error.as_deref(),
                hint: "Enter: save | Tab: switch field | Esc: cancel",
            },
        ),
        NotesMode::ConfirmingDelete { title, .. } => {
            let prompt = format!("Delete note '{title}'?");
            draw_dialog(
                frame,
                &Dialog {
                    title: "Delete note",
                    fields: Vec::new(),
                    body: vec![Line::from(prompt)],
                    error: None,
                    hint: "Enter: confirm delete | Esc: cancel",
                },
            );
        }
        NotesMode::List | NotesMode::Viewing(_) => {}
    }
}

/// The profile add / rename / delete-confirm dialog, if one is open. The 0012/ADR-0009
/// last-profile delete guard is preserved: its server refusal still surfaces on the pane message
/// band, unchanged.
fn draw_profile_dialog(frame: &mut Frame, profiles: &ProfilesState) {
    match &profiles.mode {
        ProfilesMode::Creating(form) => draw_dialog(
            frame,
            &Dialog {
                title: "New profile",
                fields: vec![DialogField {
                    label: "Name",
                    value: &form.name,
                    focused: true,
                    masked: false,
                }],
                body: Vec::new(),
                error: form.error.as_deref(),
                hint: "Enter: save | Esc: cancel",
            },
        ),
        ProfilesMode::Renaming { form, .. } => draw_dialog(
            frame,
            &Dialog {
                title: "Rename profile",
                fields: vec![DialogField {
                    label: "Name",
                    value: &form.name,
                    focused: true,
                    masked: false,
                }],
                body: Vec::new(),
                error: form.error.as_deref(),
                hint: "Enter: save | Esc: cancel",
            },
        ),
        ProfilesMode::ConfirmingDelete { name, .. } => {
            let prompt = format!("Delete profile '{name}'?");
            draw_dialog(
                frame,
                &Dialog {
                    title: "Delete profile",
                    fields: Vec::new(),
                    body: vec![Line::from(prompt)],
                    error: None,
                    hint: "Enter: confirm delete | Esc: cancel",
                },
            );
        }
        ProfilesMode::List => {}
    }
}

/// The `?` help overlay: a centred dialog listing the full post-auth hotkey reference (the keys
/// the trimmed footer no longer enumerates), derived from the action keys 0014 documents plus the
/// globals (Assumption A7). Closed with `Esc` / `?`.
fn draw_help(frame: &mut Frame) {
    let body = vec![
        Line::from("Global"),
        Line::from("  Tab / Shift+Tab   switch tab (Tasks / Notes / Profiles)"),
        Line::from("  Up / Down         move the selection"),
        Line::from("  p                 start / stop the focus timer"),
        Line::from("  d                 set the timer duration"),
        Line::from("  r                 refresh the current view"),
        Line::from("  Esc               cancel an in-flight / loading request"),
        Line::from("  ? / Esc  close help    q  quit"),
        Line::from(""),
        Line::from("Tasks    a add · e edit · c toggle done · x delete"),
        Line::from("Notes    a add · e edit · x delete · Enter open"),
        Line::from("Profiles Enter switch · a add · e rename · x delete"),
    ];
    draw_dialog(
        frame,
        &Dialog {
            title: "Help — hotkeys",
            fields: Vec::new(),
            body,
            error: None,
            hint: "?/Esc: close",
        },
    );
}

/// The `Tasks | Notes | Profiles` tab bar with the active tab bold-highlighted.
fn tab_bar_line(active: Tab) -> Line<'static> {
    let tab_span = |label: &'static str, tab: Tab| {
        if tab == active {
            Span::styled(label, Style::default().add_modifier(Modifier::REVERSED))
        } else {
            Span::raw(label)
        }
    };
    Line::from(vec![
        tab_span("Tasks", Tab::Tasks),
        Span::raw(" | "),
        tab_span("Notes", Tab::Notes),
        Span::raw(" | "),
        tab_span("Profiles", Tab::Profiles),
    ])
}

/// Whether the active pane has a request outstanding (drives the footer spinner).
fn pane_pending(main: &MainState) -> bool {
    match main.active_tab {
        Tab::Tasks => main.tasks.is_pending(),
        Tab::Notes => main.notes.is_pending(),
        Tab::Profiles => main.profiles.is_pending(),
    }
}

/// The message line for the active pane: the active pane's transient status/error message (e.g. a
/// list-load error or the `last_profile` refusal) plus a timer status message. The add/edit/delete
/// and duration sub-flows no longer render here — they are dialogs (ADR-0010 §3).
fn main_message_line(main: &MainState, timer: &Timer) -> String {
    match main.active_tab {
        Tab::Tasks => task_message_line(&main.tasks, timer),
        Tab::Notes => note_message_line(&main.notes),
        Tab::Profiles => profile_message_line(&main.profiles),
    }
}

/// The width of the centred auth box. Wide enough for the longest hint without dominating the
/// terminal; the box is centred on both axes (ADR-0010 §2).
const AUTH_BOX_WIDTH: u16 = 60;

fn draw_auth(frame: &mut Frame, auth: &AuthState, tick: u64) {
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

    // The box is just tall enough for the title, every field (3 rows each), the error band, and
    // the hint — then centred on both axes (Flex::Center).
    let field_rows = u16::try_from(fields.len()).unwrap_or(0).saturating_mul(3);
    let box_height = field_rows.saturating_add(1 + 2 + 2);
    let box_area = centered_rect(AUTH_BOX_WIDTH, box_height, frame.area());

    let mut constraints = vec![Constraint::Length(1)];
    constraints.extend(std::iter::repeat_n(Constraint::Length(3), fields.len()));
    constraints.push(Constraint::Length(2));
    constraints.push(Constraint::Length(2));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(box_area);

    if let Some(slot) = chunks.first() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!("organized koala - {title}"),
                Style::default().add_modifier(Modifier::BOLD),
            ))
            .alignment(Alignment::Center),
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

/// A `width × height` rectangle centred on both axes within `area` (clamped to `area`). Used for
/// the centred auth box (ADR-0010 §2).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [row] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [cell] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(row);
    cell
}

/// A single editable field within a [`Dialog`].
struct DialogField<'a> {
    /// The field's label, shown as the bordered box title.
    label: &'a str,
    /// The current field value.
    value: &'a str,
    /// Whether this field has focus (drawn with a purple border).
    focused: bool,
    /// Whether the value is rendered masked (e.g. a password). Always `false` for dialogs today.
    masked: bool,
}

/// A centred floating modal: a titled, bordered box overlaying the active view, carrying any
/// editable fields, an optional confirmation prompt, an optional inline error line, and a footer
/// hint. The single widget all six dialog kinds (task/note/profile add-edit, the three
/// delete-confirms, the timer duration edit) and the help overlay feed (ADR-0010 §3).
struct Dialog<'a> {
    /// The dialog title shown on the border.
    title: &'a str,
    /// The editable fields, in focus order. Empty for a confirmation dialog.
    fields: Vec<DialogField<'a>>,
    /// Free-form body lines (a confirmation question, or the help reference). Rendered above the
    /// hint, below the fields.
    body: Vec<Line<'a>>,
    /// An inline error to surface inside the dialog, if any.
    error: Option<&'a str>,
    /// The footer hint (e.g. `Enter: save | Tab: switch field | Esc: cancel`).
    hint: &'a str,
}

/// The width of a centred dialog box. Wide enough for the field hints and the help reference
/// without dominating the 80-column test viewport.
const DIALOG_WIDTH: u16 = 64;

/// Draw a centred floating [`Dialog`] over the current view: clear its footprint, render the
/// bordered titled box, then lay out fields / body / error / hint inside it. A deep, narrow helper
/// — one function the six dialog kinds and the help overlay all feed (coding-standards).
fn draw_dialog(frame: &mut Frame, dialog: &Dialog) {
    let field_rows = u16::try_from(dialog.fields.len())
        .unwrap_or(0)
        .saturating_mul(3);
    let body_rows = u16::try_from(dialog.body.len()).unwrap_or(0);
    // Box: top+bottom border (2), fields (3 each), body lines, a blank+error row, the hint row.
    let inner = field_rows
        .saturating_add(body_rows)
        .saturating_add(1) // error line
        .saturating_add(1); // hint line
    let box_height = inner.saturating_add(2);
    let area = centered_rect(DIALOG_WIDTH, box_height, frame.area());

    // Clear the footprint so the underlying panes do not bleed through the modal.
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .title(Span::styled(
            dialog.title.to_owned(),
            Style::default().add_modifier(Modifier::BOLD),
        ));
    let body_area = block.inner(area);
    frame.render_widget(block, area);

    let mut constraints = vec![Constraint::Length(3); dialog.fields.len()];
    if body_rows > 0 {
        constraints.push(Constraint::Length(body_rows));
    }
    constraints.push(Constraint::Length(1)); // error
    constraints.push(Constraint::Length(1)); // hint
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(body_area);

    let mut idx = 0;
    for field in &dialog.fields {
        if let Some(slot) = chunks.get(idx) {
            draw_field(
                frame,
                *slot,
                field.label,
                field.value,
                field.focused,
                field.masked,
            );
        }
        idx += 1;
    }
    if body_rows > 0 {
        if let Some(slot) = chunks.get(idx) {
            frame.render_widget(
                Paragraph::new(dialog.body.clone()).wrap(Wrap { trim: false }),
                *slot,
            );
        }
        idx += 1;
    }
    if let Some(slot) = chunks.get(idx)
        && let Some(err) = dialog.error
    {
        frame.render_widget(
            Paragraph::new(Span::styled(
                err.to_owned(),
                Style::default().fg(Color::Red),
            ))
            .wrap(Wrap { trim: true }),
            *slot,
        );
    }
    idx += 1;
    if let Some(slot) = chunks.get(idx) {
        frame.render_widget(
            Paragraph::new(Span::raw(dialog.hint.to_owned())).wrap(Wrap { trim: true }),
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
        // A focused field is signalled by a purple border (ADR-0010 §3, criterion 6), replacing
        // the former bold-border cue. Applied uniformly to auth fields and dialog fields.
        block = block.border_style(Style::default().fg(Color::Magenta));
    }
    frame.render_widget(Paragraph::new(shown).block(block), area);
}

/// Render the Tasks pane (the active profile's task list) into `area`. The title, tab bar, message
/// line, and footer are owned by [`draw_main`].
fn draw_task_pane(frame: &mut Frame, area: Rect, list: &TaskListState) {
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
    frame.render_stateful_widget(widget, area, &mut state);
}

/// The message line for the Tasks pane: the timer message, or the pane's own transient message.
/// The add/edit/delete sub-flows render as dialogs now, not here (ADR-0010 §3).
fn task_message_line(list: &TaskListState, timer: &Timer) -> String {
    if let Some(msg) = &timer.message {
        msg.clone()
    } else {
        list.message.clone().unwrap_or_default()
    }
}

/// Render the Notes pane (the active profile's notes, or the open view sub-flow) into `area`. The
/// title, tab bar, message line, and footer are owned by [`draw_main`].
fn draw_notes_pane(frame: &mut Frame, area: Rect, notes: &NotesState) {
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
            area,
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
        frame.render_stateful_widget(widget, area, &mut state);
    }
}

/// The message line for the notes screen: the screen's transient message. The create/edit/delete
/// sub-flows render as dialogs now (ADR-0010 §3); the read-only Viewing mode is unchanged
/// (Assumption A6) and shows no message-band text.
fn note_message_line(notes: &NotesState) -> String {
    notes.message.clone().unwrap_or_default()
}

/// Render the profile switcher: the account's profiles (the active one marked), or the open
/// create/rename/delete sub-flow, plus the shared bottom row. `active_id` marks which row is the
/// currently-scoped profile.
/// Render the Profiles pane (the account's profiles, the active one marked) into `area`.
/// `active_id` marks the currently-scoped profile. The title, tab bar, message line, and footer
/// are owned by [`draw_main`].
fn draw_profiles_pane(frame: &mut Frame, area: Rect, profiles: &ProfilesState, active_id: &str) {
    let items: Vec<ListItem> = profiles
        .profiles
        .iter()
        .map(|profile| {
            // The active (currently-scoped) profile is marked so the switch target is clear.
            let marker = if profile.id == active_id { "* " } else { "  " };
            ListItem::new(Line::from(format!("{marker}{}", profile.name)))
        })
        .collect();
    let widget = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Profiles"))
        .highlight_symbol("> ")
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    state.select(profiles.selected);
    frame.render_stateful_widget(widget, area, &mut state);
}

/// The message line for the switcher: the screen's transient message (e.g. the `last_profile`
/// refusal, ADR-0009 §4). The create/rename/delete sub-flows render as dialogs now (ADR-0010 §3).
fn profile_message_line(profiles: &ProfilesState) -> String {
    profiles.message.clone().unwrap_or_default()
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
