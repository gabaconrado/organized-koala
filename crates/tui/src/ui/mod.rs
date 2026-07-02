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
    App, AuthField, AuthMode, AuthState, MainState, NoteDetail, NotePane, NotesMode, NotesState,
    ProfilesMode, ProfilesState, Screen, Tab, TaskDetail, TaskListState, TaskPane, Timer,
};
use contract::{Note, TaskStatus, TimerSession};

/// The frames of the in-flight spinner, cycled by the poll loop's tick counter.
const SPINNER_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

/// The single trimmed footer caption for every post-auth pane (ADR-0010 §3, criterion 2): the
/// essentials only — movement, tab switch, quit, and help. The full per-pane hotkey reference now
/// lives in the `?` help overlay ([`draw_help`]), so the footer no longer enumerates the action
/// keys. A single non-wrapping line, ` | `-separated; while a request is outstanding the in-flight
/// spinner glyph (only the glyph — no textual affordance) is appended, so the footer stays a single
/// flush row (`BOTTOM_BAND_ROWS == 1`). The `Esc`-cancels-an-in-flight-request affordance now lives
/// in the `?` help modal rather than the caption (ADR-0006 §8.3, amended 2026-06-26).
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
                width: DIALOG_WIDTH,
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
                width: DIALOG_WIDTH,
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
                width: DIALOG_WIDTH,
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
    } else if let Some(add) = &list.adding_subtask {
        draw_dialog(
            frame,
            &Dialog {
                title: "Add sub-task",
                width: DIALOG_WIDTH,
                fields: vec![DialogField {
                    label: "Title",
                    value: &add.title,
                    focused: true,
                    masked: false,
                }],
                body: Vec::new(),
                error: add.error.as_deref(),
                hint: "Enter: save | Esc: cancel",
            },
        );
    } else if let Some(edit) = &list.editing_subtask {
        draw_dialog(
            frame,
            &Dialog {
                title: "Edit sub-task",
                width: DIALOG_WIDTH,
                fields: vec![DialogField {
                    label: "Title",
                    value: &edit.title,
                    focused: true,
                    masked: false,
                }],
                body: Vec::new(),
                error: edit.error.as_deref(),
                hint: "Enter: save | Esc: cancel",
            },
        );
    } else if list.confirming_delete.is_some() {
        draw_dialog(
            frame,
            &Dialog {
                title: "Delete task",
                width: DIALOG_WIDTH,
                fields: Vec::new(),
                body: vec![Line::from("Delete this task?")],
                error: None,
                hint: "Enter: confirm delete | Esc: cancel",
            },
        );
    }
}

/// The note add / edit / delete-confirm dialog, if one is open. The per-field detail view
/// (`NotesMode::Detail`) renders in the main content area, **not** as a dialog (ADR-0010 §4).
fn draw_note_dialog(frame: &mut Frame, notes: &NotesState) {
    match &notes.mode {
        NotesMode::Creating(form) => draw_dialog(
            frame,
            &Dialog {
                title: "New note",
                width: DIALOG_WIDTH,
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
                width: DIALOG_WIDTH,
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
                    width: DIALOG_WIDTH,
                    fields: Vec::new(),
                    body: vec![Line::from(prompt)],
                    error: None,
                    hint: "Enter: confirm delete | Esc: cancel",
                },
            );
        }
        NotesMode::List | NotesMode::Detail(_) => {}
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
                width: DIALOG_WIDTH,
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
                width: DIALOG_WIDTH,
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
                    width: DIALOG_WIDTH,
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
/// the trimmed footer no longer enumerates), in the final hotkey scheme (ADR-0010 §4). Closed with
/// `Esc` / `?`.
fn draw_help(frame: &mut Frame) {
    let body = vec![
        Line::from("Global"),
        Line::from("  Tab / Shift+Tab   switch tab (Tasks / Notes / Profiles)"),
        Line::from("  Up / Down         move the selection"),
        Line::from("  t                 start / stop the focus timer"),
        Line::from("  T                 set the timer duration"),
        Line::from("  r                 refresh the current view"),
        Line::from("  Esc               cancel an in-flight / loading request"),
        Line::from("  q                 quit"),
        Line::from("  ? / Esc           close help"),
        Line::from(""),
        Line::from("Tasks    a add · A add sub-task · e edit · Space done · d delete"),
        Line::from("         x collapse/expand sub-tasks · Enter detail · h hide older"),
        Line::from("Notes    a add · e edit · d delete · Enter detail"),
        Line::from("Profiles Enter switch · a add · e rename · d delete"),
        Line::from(""),
        Line::from("Detail   Tab panes · e edit · Enter commit · Esc back"),
        Line::from("         Content: Enter inserts a newline, Ctrl+S commits"),
    ];
    draw_dialog(
        frame,
        &Dialog {
            title: "Help — hotkeys",
            width: HELP_DIALOG_WIDTH,
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
    /// The box width in columns. The five form/confirm/timer dialogs pass [`DIALOG_WIDTH`]; the
    /// `?` help overlay passes the wider [`HELP_DIALOG_WIDTH`] so its reference lines do not wrap.
    width: u16,
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

/// The width of a centred dialog box. Wide enough for the field hints without dominating the
/// 80-column test viewport. Shared by the five form/confirm/timer dialog kinds; the `?` help
/// overlay uses the wider [`HELP_DIALOG_WIDTH`] so its reference lines fit on one row each.
const DIALOG_WIDTH: u16 = 64;

/// The width of the `?` help overlay box, decoupled from [`DIALOG_WIDTH`] so widening the help
/// reference does not move the other five dialogs' snapshots. Wide enough that the longest Tasks
/// reference line (the 64-char `a add · A add sub-task · …`) fits inside the bordered box's inner
/// area (width − 2) with headroom for future hotkeys, while staying centred within the 80-column
/// test viewport with comfortable side margin.
const HELP_DIALOG_WIDTH: u16 = 72;

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
    let area = centered_rect(dialog.width, box_height, frame.area());

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

/// A pane of a detail view: a label, its (snapshot or in-edit) value, whether it is focused
/// (purple border), whether it is editable (read-only panes show a plain border even when focused
/// so the user sees `e` is inert), and whether it `fill`s the remaining height (a multiline pane
/// that grows + wraps; default `false`, a fixed 3-row box).
struct DetailPane<'a> {
    label: &'a str,
    value: String,
    focused: bool,
    editable: bool,
    /// Whether this pane takes the remaining vertical space (`Constraint::Min`) and renders its
    /// value with newline + wrap support, rather than a fixed single-line 3-row box. Opt-in per
    /// pane (the note Content pane; ADR-0011) so the task detail layout is unchanged.
    fill: bool,
}

/// Floor for a fill pane's height so a short value still shows a usable box matching the others'
/// 3-row boxes, then grows to fill the remaining space (Assumption A4).
const FILL_PANE_MIN_ROWS: u16 = 3;

/// Draw a vertical stack of detail-view panes in `area`. Fixed panes are 3-row bordered boxes; a
/// `fill` pane takes the remaining height (`Constraint::Min`) and renders with newline + wrap. The
/// focused editable pane gets the purple focus border (ADR-0010 §4, reusing the dialog/`draw_field`
/// cue); a focused read-only pane is bordered but not purple, signalling `e` is inert there. The
/// detail view renders in the main content area, **not** as a floating dialog.
fn draw_detail_panes(frame: &mut Frame, area: Rect, title: &str, panes: &[DetailPane]) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title.to_owned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let constraints: Vec<Constraint> = panes
        .iter()
        .map(|pane| {
            if pane.fill {
                Constraint::Min(FILL_PANE_MIN_ROWS)
            } else {
                Constraint::Length(3)
            }
        })
        .collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);
    for (i, pane) in panes.iter().enumerate() {
        let Some(slot) = chunks.get(i) else { continue };
        let mut field = Block::default()
            .borders(Borders::ALL)
            .title(pane.label.to_owned());
        if pane.focused && pane.editable {
            field = field.border_style(Style::default().fg(Color::Magenta));
        }
        let mut paragraph = Paragraph::new(pane.value.clone()).block(field);
        if pane.fill {
            // Multiline: honour embedded '\n' and wrap long lines so Content displays fully.
            paragraph = paragraph.wrap(Wrap { trim: false });
        }
        frame.render_widget(paragraph, *slot);
    }
}

/// Render the task detail view: Title/Description editable, Status/Created/Closed read-only, with
/// the focused editable pane purple-bordered, and a read-only "Sub-tasks" section below listing
/// each sub-task's title + status (ADR-0012 §1 — sub-task rows here are **not** focusable panes).
/// An in-progress edit shows the live buffer value.
fn draw_task_detail(frame: &mut Frame, area: Rect, detail: &TaskDetail) {
    let task = &detail.task;
    let panes: Vec<DetailPane> = detail
        .panes
        .iter()
        .enumerate()
        .map(|(i, pane)| {
            let focused = i == detail.focused;
            let editing = focused && detail.is_editing();
            let value = match pane {
                TaskPane::Title => task_editing_or(detail, editing, task.title.clone()),
                TaskPane::Description => task_editing_or(detail, editing, task.description.clone()),
                TaskPane::Status => match task.status {
                    TaskStatus::Open => "open".to_owned(),
                    TaskStatus::Done => "done".to_owned(),
                },
                TaskPane::Created => task.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
                TaskPane::Closed => task
                    .closed_at
                    .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_default(),
            };
            DetailPane {
                label: task_pane_label(*pane),
                value,
                focused,
                editable: pane.is_editable(),
                fill: false,
            }
        })
        .collect();
    // Reserve the upper part of the area for the per-field panes and the lower part for the
    // read-only "Sub-tasks" section. The panes take their fixed 3-row boxes; the section fills the
    // remainder, with a minimum so it is always visible.
    let pane_rows = u16::try_from(detail.panes.len())
        .unwrap_or(0)
        .saturating_mul(3)
        .saturating_add(2); // the surrounding "Task" block's top+bottom border
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(pane_rows), Constraint::Min(3)])
        .split(area);
    if let Some(slot) = chunks.first() {
        draw_detail_panes(frame, *slot, "Task", &panes);
    }
    if let Some(slot) = chunks.get(1) {
        draw_task_subtasks_section(frame, *slot, &detail.subtasks);
    }
}

/// Render the read-only "Sub-tasks" section of the task detail: one line per sub-task, each its
/// status marker + title (ADR-0012 §1 — no per-field view, not focusable). An empty section shows
/// a placeholder line.
fn draw_task_subtasks_section(frame: &mut Frame, area: Rect, subtasks: &[contract::Subtask]) {
    let block = Block::default().borders(Borders::ALL).title("Sub-tasks");
    let lines: Vec<Line> = if subtasks.is_empty() {
        vec![Line::from(Span::raw("(no sub-tasks)"))]
    } else {
        subtasks
            .iter()
            .map(|subtask| {
                Line::from(format!(
                    "{} {}",
                    status_marker(subtask.status),
                    subtask.title
                ))
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// The live edit buffer when this pane is being edited, else the snapshot `value`.
fn task_editing_or(detail: &TaskDetail, editing: bool, value: String) -> String {
    if editing {
        detail.edit.clone().unwrap_or(value)
    } else {
        value
    }
}

/// The display label for a task detail pane.
fn task_pane_label(pane: TaskPane) -> &'static str {
    match pane {
        TaskPane::Title => "Title",
        TaskPane::Description => "Description",
        TaskPane::Status => "Status",
        TaskPane::Created => "Created",
        TaskPane::Closed => "Closed",
    }
}

/// The status marker for a task or sub-task: `[x]` done, `[ ]` open.
fn status_marker(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Done => "[x]",
        TaskStatus::Open => "[ ]",
    }
}

/// Render the Tasks pane (the active profile's task list, or the open detail view) into `area`. The
/// title, tab bar, message line, and footer are owned by [`draw_main`].
///
/// A human-readable **today** date renders as the first list row: a full-width, non-selectable bold
/// separator inside this pane only (acceptance #2, amended). Below it the list interleaves task rows
/// and (indented one level) sub-task rows, walking only the **visible** rows for the current day
/// (completed-last within each group, created-today above a full-width "Older tasks" separator, older
/// tasks defaulting collapsed; ADR-0014 §4–5). A task's leading indicator is `+` when it has
/// sub-tasks **and** they are collapsed, else `>`; the selection highlight is the reversed style on
/// the selected visible row.
fn draw_task_pane(frame: &mut Frame, area: Rect, list: &TaskListState) {
    if let Some(detail) = &list.detail {
        draw_task_detail(frame, area, detail);
        return;
    }
    let today_day = crate::app::current_day_number();
    let block = Block::default().borders(Borders::ALL).title("Tasks");
    // The list's inner (content) width, inside the two border columns; both separator rows span it.
    let inner_width = usize::from(block.inner(area).width);
    // The date row is prepended at draw and is NOT part of `visible_rows`; a bold, non-selectable
    // full-width separator carrying today's date.
    let mut items: Vec<ListItem> = vec![ListItem::new(Line::from(Span::styled(
        separator_line(&today_header(today_day), inner_width),
        Style::default().add_modifier(Modifier::BOLD),
    )))];
    items.extend(list.visible_rows(today_day).into_iter().filter_map(|row| {
        match row {
            crate::app::VisibleRow::Task { task_idx } => list.tasks.get(task_idx).map(|task| {
                // `+` when the task has sub-tasks AND they resolve collapsed for this day (the
                // group-aware resolution matching `visible_rows`, ADR-0014 §5); otherwise `>`.
                let indicator =
                    if list.has_subtasks(task) && list.resolve_collapsed(task, today_day) {
                        '+'
                    } else {
                        '>'
                    };
                ListItem::new(Line::from(format!(
                    "{indicator} {} {}",
                    status_marker(task.status),
                    task.title
                )))
            }),
            crate::app::VisibleRow::Subtask { subtask_idx } => {
                list.subtasks.get(subtask_idx).map(|subtask| {
                    // Sub-task rows are indented one level under their parent task.
                    ListItem::new(Line::from(format!(
                        "    {} {}",
                        status_marker(subtask.status),
                        subtask.title
                    )))
                })
            }
            // The non-selectable full-width "Older tasks" separator between the two groups.
            crate::app::VisibleRow::OlderSeparator => {
                Some(ListItem::new(Line::from(Span::styled(
                    separator_line(crate::app::OLDER_SEPARATOR_LABEL, inner_width),
                    Style::default().add_modifier(Modifier::DIM),
                ))))
            }
        }
    }));
    let widget = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    // The date row is prepended (index 0) and is not in `visible_rows`, so offset the selection by
    // one: `list.selected` indexes the rows after it, keeping selection/`visible_rows` untouched and
    // the date row unselectable.
    let mut state = ListState::default();
    state.select(list.selected.map(|i| i + 1));
    frame.render_stateful_widget(widget, area, &mut state);
}

/// Center `label` on a `─`-filled line of exactly `inner_width` display columns, e.g.
/// `── … Tuesday, July 2nd, 2026 … ──`. Used for both the today date row and the "Older tasks"
/// separator so each spans the full pane inner width. A label at least as wide as `inner_width` is
/// returned unpadded (it already fills or overflows the row).
fn separator_line(label: &str, inner_width: usize) -> String {
    let label_width = label.chars().count();
    if label_width + 2 > inner_width {
        return label.to_owned();
    }
    // One space of breathing room each side of the label, the rest filled with `─`.
    let fill = inner_width - label_width - 2;
    let left = fill / 2;
    let right = fill - left;
    format!("{} {label} {}", "─".repeat(left), "─".repeat(right))
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

/// Render the note detail view: Title/Content editable, Created read-only, with the focused
/// editable pane purple-bordered. An in-progress edit shows the live buffer value.
fn draw_note_detail(frame: &mut Frame, area: Rect, detail: &NoteDetail) {
    let note = &detail.note;
    let panes: Vec<DetailPane> = NotePane::ALL
        .iter()
        .enumerate()
        .map(|(i, pane)| {
            let focused = i == detail.focused;
            let editing = focused && detail.is_editing();
            let value = match pane {
                NotePane::Title => note_editing_or(detail, editing, note.title.clone()),
                NotePane::Content => note_editing_or(detail, editing, note.content.clone()),
                NotePane::Created => format_created_at(note),
            };
            DetailPane {
                label: note_pane_label(*pane),
                value,
                focused,
                editable: pane.is_editable(),
                // The multiline Content pane fills the remaining height and wraps (ADR-0011);
                // Title/Created stay fixed 3-row boxes.
                fill: matches!(pane, NotePane::Content),
            }
        })
        .collect();
    draw_detail_panes(frame, area, "Note", &panes);
}

/// The live note edit buffer when this pane is being edited, else the snapshot `value`.
fn note_editing_or(detail: &NoteDetail, editing: bool, value: String) -> String {
    if editing {
        detail.edit.clone().unwrap_or(value)
    } else {
        value
    }
}

/// The display label for a note detail pane. The multiline `Content` label carries the terse
/// commit/newline hint so the `Ctrl+S` affordance is discoverable at the point of use (ADR-0011).
fn note_pane_label(pane: NotePane) -> &'static str {
    match pane {
        NotePane::Title => "Title",
        NotePane::Content => "Content (Enter: newline · Ctrl+S: commit)",
        NotePane::Created => "Created",
    }
}

/// Render the Notes pane (the active profile's notes, or the open per-field detail view) into
/// `area`. The title, tab bar, message line, and footer are owned by [`draw_main`].
fn draw_notes_pane(frame: &mut Frame, area: Rect, notes: &NotesState) {
    // The detail sub-flow replaces the list with the note's per-field panes; otherwise the list of
    // notes is shown with its selection highlight.
    if let NotesMode::Detail(detail) = &notes.mode {
        draw_note_detail(frame, area, detail);
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
/// sub-flows render as dialogs now (ADR-0010 §3); the per-field detail view renders in the content
/// area (ADR-0010 §4) and shows no message-band text.
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

/// The three-letter month names, index 1..=12.
const MONTH_NAMES: [&str; 13] = [
    "",
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The weekday names, index 0 = Sunday .. 6 = Saturday.
const WEEKDAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// The civil `(year, month, day)` for a day number (days since 1970-01-01), via Howard Hinnant's
/// `civil_from_days`. Pure integer math so the today-header formatting is unit-testable and the
/// `tui` crate stays free of a `chrono`/timezone dependency (A8).
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    let month = u32::try_from(m).unwrap_or(1);
    let day = u32::try_from(d).unwrap_or(1);
    (year, month, day)
}

/// The weekday index (0 = Sunday .. 6 = Saturday) for a day number. 1970-01-01 was a Thursday
/// (index 4), so `(days + 4) mod 7`.
#[must_use]
pub fn weekday_index(days: i64) -> usize {
    usize::try_from((days + 4).rem_euclid(7)).unwrap_or(0)
}

/// The English ordinal suffix for a day of month (`st`/`nd`/`rd`/`th`), with the 11–13 → `th`
/// exception. Pure and unit-testable (ADR-0014 R5).
#[must_use]
pub fn ordinal_suffix(day: u32) -> &'static str {
    if (11..=13).contains(&(day % 100)) {
        "th"
    } else {
        match day % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    }
}

/// The human-readable today date header shown top-center in the Tasks pane (acceptance #2), e.g.
/// `Tuesday, July 2nd, 2026`: weekday, month, ordinal day, year, from the current day number.
#[must_use]
pub fn today_header(day_number: i64) -> String {
    let (year, month, day) = civil_from_days(day_number);
    let weekday = WEEKDAY_NAMES.get(weekday_index(day_number)).unwrap_or(&"");
    let month_name = MONTH_NAMES
        .get(usize::try_from(month).unwrap_or(0))
        .unwrap_or(&"");
    format!(
        "{weekday}, {month_name} {day}{}, {year}",
        ordinal_suffix(day)
    )
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
