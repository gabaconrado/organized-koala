//! The task-list screen for the active profile: the task vector, the profile's sub-tasks, the
//! visible-row selection, the optional add/edit sub-flows, the in-flight marker, and the pure
//! event handler producing [`ClientRequest`]s.
//!
//! The list interleaves task rows and (indented) sub-task rows. A parent's sub-tasks are grouped
//! under it by `task_id` (defensively — an orphan sub-task whose parent is absent is ignored,
//! never panics; ADR-0013 Risk R3) and are shown expanded or collapsed. **Collapse is derived,
//! transient presentation state** (#1 / ADR-0012 §5): the default derives *each render* from the
//! task's group — a **today** task follows its status (open → expanded, done → collapsed), an
//! **older** task defaults collapsed regardless of status (acceptance #3); the user's `x` toggle
//! records an in-session, process-lifetime override keyed by task id (applying to either group),
//! never persisted, dropped on a fresh load for a task no longer present.

use std::collections::HashMap;

use contract::{
    CreateSubtaskRequest, CreateTaskRequest, Subtask, Task, TaskStatus, UpdateSubtaskRequest,
    UpdateTaskRequest,
};

use super::Session;
use super::protocol::{ClientRequest, RequestId};
use super::task_add::{AddSubtaskState, AddTaskState, EditSubtaskState, EditTaskState};
use super::task_detail::{TaskDetail, TaskPane};
use super::text_input::{self, TextInput};
use crate::app::Event;

/// A row in the rendered task list: a top-level task, one of its sub-tasks, or the non-selectable
/// "Older tasks" separator between the created-today and created-before-today groups (ADR-0014 §5).
/// The row list is the shared source of truth for both the render and the selection cursor, so the
/// two never diverge; selection **skips** the separator and never lands on a hidden row (ADR-0013
/// Risk R2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleRow {
    /// A top-level task at this index in `tasks`.
    Task {
        /// Index into [`TaskListState::tasks`].
        task_idx: usize,
    },
    /// A sub-task at this index in `subtasks`, rendered indented under its parent task.
    Subtask {
        /// Index into [`TaskListState::subtasks`].
        subtask_idx: usize,
    },
    /// The "Older tasks" separator row between the today and older groups. Rendered but never
    /// selectable (navigation skips it); present only when the older group is shown and non-empty.
    OlderSeparator,
}

/// The label rendered on the [`VisibleRow::OlderSeparator`] row (acceptance #3), before it is
/// padded to the full pane inner width at draw time. Retained as the pre-0023 default label; the
/// live separator now renders the dynamic `Last {X} days` text (ADR-0015), see the render layer.
pub const OLDER_SEPARATOR_LABEL: &str = "Older tasks";

/// Default size of the "hide tasks older than X days" window (ADR-0015): the anchor day plus the
/// previous `X` days. Ephemeral in-session view-state, reset to this on every restart (#1).
pub const DEFAULT_HIDE_WINDOW_DAYS: u32 = 3;

/// Minimum accepted window size for the `F` editor: `1` yields a 2-day window `[anchor − 1, anchor]`
/// (operator, 2026-07-08). `0` is rejected inline — today-only is reached via the `h` toggle, not an
/// `X = 0` mode.
pub const MIN_HIDE_WINDOW_DAYS: u32 = 1;

/// Lower bound on the `f` date editor's year component (ADR-0015 / feature 0023): `year ≥ 1970`.
pub const MIN_FILTER_YEAR: i64 = 1970;

/// What a two-step delete confirmation is armed against: the selected **task** or the selected
/// **sub-task** (which carries its parent task id, since the delete wire is task-scoped). Resolving
/// the armed target on `arm_delete` lets `confirm_delete` dispatch the matching
/// [`ClientRequest::DeleteTask`] / [`ClientRequest::DeleteSubtask`] without re-reading the selection.
#[derive(Debug, Clone)]
pub enum DeleteTarget {
    /// A top-level task, deleted via [`ClientRequest::DeleteTask`].
    Task {
        /// The task to delete.
        task_id: String,
    },
    /// A sub-task, deleted via [`ClientRequest::DeleteSubtask`] (task-scoped by its parent).
    Subtask {
        /// The parent task owning the sub-task.
        task_id: String,
        /// The sub-task to delete.
        subtask_id: String,
    },
}

/// Seconds in a day, for the epoch-seconds → civil-day-number derivation (ADR-0014 §4 today/older
/// split). Kept in epoch seconds — like the timer countdown — so the `tui` crate stays free of a
/// direct `chrono` dependency (hard-constraint A8): the caller derives seconds from the DTO's
/// `DateTime` via `timestamp()`, and grouping compares whole-day numbers.
pub(crate) const SECS_PER_DAY: i64 = 86_400;

/// The whole-day number (days since the Unix epoch) of a UTC timestamp in epoch **seconds**. Two
/// timestamps map to the same day iff they share this number. Pure integer math (no `chrono`), so
/// the today/older split is unit-testable with injected day numbers.
#[must_use]
pub fn day_number(epoch_secs: i64) -> i64 {
    epoch_secs.div_euclid(SECS_PER_DAY)
}

/// The `F` window-size editor: a single numeric input buffer for the new "hide older than X days"
/// window. The same transient text-entry sub-flow category as [`AddTaskState`](super::AddTaskState)
/// and the timer [`DurationEditState`](super::DurationEditState); reset on restart, never persisted
/// (#1).
#[derive(Debug, Clone)]
pub struct WindowEditState {
    /// The entered window text (digits only; parsed on submit).
    pub buffer: TextInput,
    /// Inline error (a `0`/non-numeric value rejected on submit), if any.
    pub error: Option<String>,
}

impl WindowEditState {
    fn new(current: u32) -> Self {
        Self {
            buffer: TextInput::new(current.to_string()),
            error: None,
        }
    }

    fn push_char(&mut self, c: char) {
        // Digit filtering stays at the call site (ADR-0015 numeric buffer); the caret still moves.
        if c.is_ascii_digit() {
            self.buffer.insert_char(c);
        }
    }

    fn backspace(&mut self) {
        self.buffer.backspace();
    }

    fn motion(&mut self, event: &Event) -> bool {
        text_input::apply_motion(&mut self.buffer, event)
    }
}

/// Which component of the [`DateFilterState`] editor the focus is on. `Tab` cycles
/// day → month → year; Up/Down adjust the focused component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateComponent {
    /// The day-of-month component (bounded 1–31, wraps in place).
    Day,
    /// The month component (bounded 1–12, wraps in place).
    Month,
    /// The year component (bounded `≥ 1970`).
    Year,
}

/// The `f` date-filter editor: a three-component `DD/MM/YYYY` selector seeded to today. Up/Down
/// adjust the focused component with **wrap-in-place, no carry** (`month 1 −1 → 12` without touching
/// the year; `day 1 −1 → 31`; `day 31 +1 → 1`); no calendar validation (28/30/31 not checked). On
/// submit the `(day, month, year)` maps to a civil day-number. Transient in-session view-state (#1).
#[derive(Debug, Clone)]
pub struct DateFilterState {
    /// Selected day of month, 1–31 (not calendar-validated against the month).
    pub day: u32,
    /// Selected month, 1–12.
    pub month: u32,
    /// Selected year, `≥ 1970`.
    pub year: i64,
    /// Which component the focus is on.
    pub focus: DateComponent,
}

impl DateFilterState {
    /// Seed the editor to `today_day` (a civil day-number), focus on the day component.
    fn new(today_day: i64) -> Self {
        let (year, month, day) = crate::ui::civil_from_days(today_day);
        Self {
            day,
            month,
            year,
            focus: DateComponent::Day,
        }
    }

    /// Cycle the focused component forward (`Tab`) or backward (`Shift+Tab`): day → month → year.
    fn cycle(&mut self, forward: bool) {
        self.focus = match (self.focus, forward) {
            (DateComponent::Day, true) | (DateComponent::Year, false) => DateComponent::Month,
            (DateComponent::Month, true) | (DateComponent::Day, false) => DateComponent::Year,
            (DateComponent::Year, true) | (DateComponent::Month, false) => DateComponent::Day,
        };
    }

    /// Increment the focused component with wrap-in-place (no carry): day 31 → 1, month 12 → 1, year
    /// unbounded upward.
    fn increment(&mut self) {
        match self.focus {
            DateComponent::Day => self.day = if self.day >= 31 { 1 } else { self.day + 1 },
            DateComponent::Month => self.month = if self.month >= 12 { 1 } else { self.month + 1 },
            DateComponent::Year => self.year += 1,
        }
    }

    /// Decrement the focused component with wrap-in-place (no carry): day 1 → 31, month 1 → 12, year
    /// clamped at [`MIN_FILTER_YEAR`].
    fn decrement(&mut self) {
        match self.focus {
            DateComponent::Day => self.day = if self.day <= 1 { 31 } else { self.day - 1 },
            DateComponent::Month => self.month = if self.month <= 1 { 12 } else { self.month - 1 },
            DateComponent::Year => self.year = (self.year - 1).max(MIN_FILTER_YEAR),
        }
    }
}

/// State of the task-list screen for the active profile.
#[derive(Debug, Clone)]
pub struct TaskListState {
    /// Tasks as returned by the server, newest-first.
    pub tasks: Vec<Task>,
    /// The profile's sub-tasks as returned by the server (the two-call tree load, ADR-0013 §3),
    /// grouped under their parent task by `task_id` when rendering.
    pub subtasks: Vec<Subtask>,
    /// Index of the selected **visible row** (task or sub-task), if any. Indexes into the row list
    /// produced by [`Self::visible_rows`], not directly into `tasks`/`subtasks`.
    pub selected: Option<usize>,
    /// Per-parent in-session collapse override, keyed by task id: `true` collapses that parent's
    /// sub-tasks, `false` expands them — overriding the status-derived default until a fresh load
    /// drops the entry for an absent task (ADR-0012 §5, A4). Transient process-lifetime UI state
    /// (#1); never persisted.
    pub collapse_overrides: HashMap<String, bool>,
    /// Active add-task sub-flow, if open.
    pub adding: Option<AddTaskState>,
    /// Active edit-task sub-flow, if open.
    pub editing: Option<EditTaskState>,
    /// Active add-sub-task sub-flow, if open (the `A` key; carries the parent task id).
    pub adding_subtask: Option<AddSubtaskState>,
    /// Active edit-sub-task-title sub-flow, if open (the `e` key on a sub-task row).
    pub editing_subtask: Option<EditSubtaskState>,
    /// The open per-field detail view, if any (ADR-0010 §4). Transient process-lifetime UI state
    /// (#1); the snapshot re-derives from the server after every commit.
    pub detail: Option<TaskDetail>,
    /// The target awaiting a delete confirmation (the two-step delete affordance): set on the
    /// first delete key by the selected **row kind** (task or sub-task), cleared on confirm or on
    /// any other navigation. `None` when not armed.
    pub confirming_delete: Option<DeleteTarget>,
    /// A transient status/error message shown to the user, if any.
    pub message: Option<String>,
    /// The in-flight request id while a list/create/update/delete call is outstanding; `None` when
    /// idle. Transient process-lifetime UI state (hard-constraint #1).
    pub pending: Option<RequestId>,
    /// Whether the created-before-today ("older") group and its "Older tasks" separator are hidden
    /// (the `h` toggle, acceptance #4). Default `false` (older shown). Ephemeral process-lifetime
    /// view state, never persisted (#1).
    pub hide_older: bool,
    /// Size of the "hide tasks older than X days" window (ADR-0015): the list shows the anchor day
    /// plus the previous `hide_window_days` days. Default [`DEFAULT_HIDE_WINDOW_DAYS`]. Ephemeral
    /// in-session query input, reset on restart (#1); the `F` editor changes it and re-fetches.
    pub hide_window_days: u32,
    /// The selected filter day as a civil day-number (day granularity), or `None` to anchor on
    /// today. Set by the `f` date editor; re-anchors the window and the today/older split on the
    /// selected day (ADR-0015). Ephemeral in-session query input, never persisted (#1).
    pub filter_date: Option<i64>,
    /// Active `F` window-size editor, if open. While set, the tasks pane owns keystrokes.
    pub editing_window: Option<WindowEditState>,
    /// Active `f` date-filter editor, if open. While set, the tasks pane owns keystrokes.
    pub filtering_date: Option<DateFilterState>,
}

impl TaskListState {
    pub(crate) fn new(tasks: Vec<Task>) -> Self {
        let selected = if tasks.is_empty() { None } else { Some(0) };
        Self {
            tasks,
            subtasks: Vec::new(),
            selected,
            collapse_overrides: HashMap::new(),
            adding: None,
            editing: None,
            adding_subtask: None,
            editing_subtask: None,
            detail: None,
            confirming_delete: None,
            message: None,
            pending: None,
            hide_older: false,
            hide_window_days: DEFAULT_HIDE_WINDOW_DAYS,
            filter_date: None,
            editing_window: None,
            filtering_date: None,
        }
    }

    /// The anchor civil day-number the today/older split, date header, and window query are anchored
    /// on: the selected [`Self::filter_date`] if set, else the wall-clock `today_day` (ADR-0015).
    #[must_use]
    pub fn anchor_day(&self, today_day: i64) -> i64 {
        self.filter_date.unwrap_or(today_day)
    }

    /// Whether the task list currently has a request outstanding.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Whether the detail view is open with a field edit in progress (text-entry context).
    #[must_use]
    pub fn detail_editing(&self) -> bool {
        self.detail.as_ref().is_some_and(TaskDetail::is_editing)
    }

    /// Whether the task pane is in an input-capturing sub-flow (add/edit task **or** add/edit
    /// sub-task, **or** an open detail view). Drives the terminal layer's overlay suppression and
    /// the `Tab`/`Esc` routing in [`crate::app::App`].
    #[must_use]
    pub fn in_sub_flow(&self) -> bool {
        self.adding.is_some()
            || self.editing.is_some()
            || self.adding_subtask.is_some()
            || self.editing_subtask.is_some()
            || self.editing_window.is_some()
            || self.filtering_date.is_some()
            || self.detail.is_some()
    }

    /// Whether a sub-task add/edit form is the active text-entry context (drives `is_text_entry`
    /// at the terminal layer so letters are typed, not interpreted as commands).
    #[must_use]
    pub fn subtask_text_entry(&self) -> bool {
        self.adding_subtask.is_some() || self.editing_subtask.is_some()
    }

    /// Whether collapse for `task` resolves to collapsed for `today_day`: the in-session override if
    /// present, else the group-derived default. In the **today** group the default follows status (a
    /// **done** parent collapses, an **open** parent expands; ADR-0012 §5); in the **older** group the
    /// default is collapsed regardless of status (acceptance #3, amended: default collapsed but the
    /// `x` override still applies). This is the single collapse resolution consulted by both the row
    /// assembly and the render indicator, so the two never diverge; it never mutates
    /// `collapse_overrides` (A7 — the older default stays render-time-derived).
    #[must_use]
    pub fn resolve_collapsed(&self, task: &Task, today_day: i64) -> bool {
        self.collapse_overrides
            .get(&task.id)
            .copied()
            .unwrap_or_else(|| {
                self.is_older(task, today_day) || matches!(task.status, TaskStatus::Done)
            })
    }

    /// Whether `task` has at least one sub-task in the loaded set (groups defensively by `task_id`).
    #[must_use]
    pub fn has_subtasks(&self, task: &Task) -> bool {
        self.subtasks.iter().any(|s| s.task_id == task.id)
    }

    /// Whether `task` belongs to the created-before-today ("older") group for `today_day`. Older
    /// tasks are forced collapsed at render regardless of status or per-task override (ADR-0014 §5);
    /// this lets the render show the collapsed `+` indicator without consulting `collapse_overrides`.
    #[must_use]
    pub fn is_older(&self, task: &Task, today_day: i64) -> bool {
        !Self::is_today(today_day, task.created_at.timestamp())
    }

    /// Whether `epoch_secs` (a task's `created_at` timestamp) falls on the `today_day` civil day.
    fn is_today(today_day: i64, epoch_secs: i64) -> bool {
        day_number(epoch_secs) == today_day
    }

    /// The `tasks` indices, partitioned into (created-today, created-before-today), each **stably
    /// sorted completed-last** (open before done). The partition preserves the server's
    /// `created_at DESC` order within each status group (a stable sort keyed only on status;
    /// ADR-0014 §4). Re-derived per call so a state change re-orders on the next render (#1).
    fn grouped_task_indices(&self, today_day: i64) -> (Vec<usize>, Vec<usize>) {
        let (mut today, mut older): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for (idx, task) in self.tasks.iter().enumerate() {
            if Self::is_today(today_day, task.created_at.timestamp()) {
                today.push(idx);
            } else {
                older.push(idx);
            }
        }
        let done_last = |group: &mut Vec<usize>| {
            group.sort_by_key(|&i| {
                matches!(self.tasks.get(i).map(|t| t.status), Some(TaskStatus::Done))
            });
        };
        done_last(&mut today);
        done_last(&mut older);
        (today, older)
    }

    /// The `subtasks` indices belonging to `task_id`, **stably sorted completed-last** (open before
    /// done), preserving the server's creation order within each status group (ADR-0014 §4).
    fn sorted_subtask_indices(&self, task_id: &str) -> Vec<usize> {
        let mut idxs: Vec<usize> = self
            .subtasks
            .iter()
            .enumerate()
            .filter(|(_, s)| s.task_id == task_id)
            .map(|(i, _)| i)
            .collect();
        idxs.sort_by_key(|&i| {
            matches!(
                self.subtasks.get(i).map(|s| s.status),
                Some(TaskStatus::Done)
            )
        });
        idxs
    }

    /// Push a task row and — unless it resolves collapsed for `today_day` ([`Self::resolve_collapsed`])
    /// — its sub-task rows (completed-last), into `rows`. Shared by both the today and older groups so
    /// collapse resolution is identical either side of the separator.
    fn push_task_rows(&self, rows: &mut Vec<VisibleRow>, task_idx: usize, today_day: i64) {
        rows.push(VisibleRow::Task { task_idx });
        let Some(task) = self.tasks.get(task_idx) else {
            return;
        };
        if self.resolve_collapsed(task, today_day) {
            return;
        }
        for subtask_idx in self.sorted_subtask_indices(&task.id) {
            rows.push(VisibleRow::Subtask { subtask_idx });
        }
    }

    /// The list of **visible rows**, in render order, for the given `today_day` civil day. The
    /// created-today group renders first (completed-last), each task followed by its sub-tasks
    /// (completed-last) unless collapsed; then — when the older group is non-empty and not hidden
    /// (`hide_older`) — the "Older tasks" separator, then the created-before-today tasks. Older
    /// tasks default to **collapsed regardless of status** but honor the per-task `x` override like
    /// the today group ([`Self::resolve_collapsed`], acceptance #3 amended): the older default is a
    /// render-time forcing, never a mutation of `collapse_overrides` (A7). A sub-task whose parent
    /// task is absent is silently dropped (Risk R3). This row list is the single source of truth
    /// shared by the render and the selection cursor.
    #[must_use]
    pub fn visible_rows(&self, today_day: i64) -> Vec<VisibleRow> {
        let (today, older) = self.grouped_task_indices(today_day);
        let mut rows = Vec::new();
        for &task_idx in &today {
            self.push_task_rows(&mut rows, task_idx, today_day);
        }
        if !older.is_empty() && !self.hide_older {
            rows.push(VisibleRow::OlderSeparator);
            for &task_idx in &older {
                self.push_task_rows(&mut rows, task_idx, today_day);
            }
        }
        rows
    }

    /// The currently-selected visible row, if any, for `today_day`.
    #[must_use]
    pub fn selected_row(&self, today_day: i64) -> Option<VisibleRow> {
        let rows = self.visible_rows(today_day);
        self.selected.and_then(|i| rows.get(i).copied())
    }

    /// The selected task, if a task row is selected.
    fn selected_task(&self, today_day: i64) -> Option<&Task> {
        match self.selected_row(today_day)? {
            VisibleRow::Task { task_idx } => self.tasks.get(task_idx),
            VisibleRow::Subtask { .. } | VisibleRow::OlderSeparator => None,
        }
    }

    /// The selected sub-task, if a sub-task row is selected.
    fn selected_subtask(&self, today_day: i64) -> Option<&Subtask> {
        match self.selected_row(today_day)? {
            VisibleRow::Subtask { subtask_idx } => self.subtasks.get(subtask_idx),
            VisibleRow::Task { .. } | VisibleRow::OlderSeparator => None,
        }
    }

    /// The id of the parent task of the current selection: the selected task itself, or the
    /// selected sub-task's parent. `A` always adds a sub-task to this task. `None` when nothing is
    /// selected (or the separator is somehow selected).
    fn parent_task_id_of_selection(&self, today_day: i64) -> Option<String> {
        match self.selected_row(today_day)? {
            VisibleRow::Task { task_idx } => self.tasks.get(task_idx).map(|t| t.id.clone()),
            VisibleRow::Subtask { subtask_idx } => {
                self.subtasks.get(subtask_idx).map(|s| s.task_id.clone())
            }
            VisibleRow::OlderSeparator => None,
        }
    }

    /// Move the selection to the next/previous **selectable** row, skipping the non-selectable
    /// "Older tasks" separator so the cursor never rests on it.
    fn move_selection(&mut self, today_day: i64, forward: bool) {
        let rows = self.visible_rows(today_day);
        let len = rows.len();
        if len == 0 {
            self.selected = None;
            return;
        }
        let mut next = self.selected.unwrap_or(0).min(len - 1);
        // Step at least once, then keep stepping while landing on a separator (bounded by `len`).
        for _ in 0..len {
            next = if forward {
                (next + 1) % len
            } else {
                (next + len - 1) % len
            };
            if !matches!(rows.get(next), Some(VisibleRow::OlderSeparator)) {
                break;
            }
        }
        self.selected = Some(next);
    }

    /// Pure update for the task-list screen. Returns the [`ClientRequest`] a request-triggering
    /// event produces (add submit, edit submit, toggle-done, delete-confirm, refresh), or `None`
    /// for a local edit or any event while a request is outstanding. `Cancel`/`Quit` are handled
    /// by the caller before reaching here. The `session` supplies the token + profile namespace
    /// for the request payloads; `today_day` (the current local civil day, [`day_number`]) resolves
    /// the today/older row layout the selection navigates.
    pub(crate) fn handle_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
        today_day: i64,
    ) -> Option<ClientRequest> {
        if self.is_pending() {
            // One request in flight: ignore request-triggering and edit events alike.
            return None;
        }
        if self.adding.is_some() {
            return self.handle_add_event(event, session);
        }
        if self.editing.is_some() {
            return self.handle_edit_event(event, session);
        }
        if self.adding_subtask.is_some() {
            return self.handle_add_subtask_event(event, session);
        }
        if self.editing_subtask.is_some() {
            return self.handle_edit_subtask_event(event, session);
        }
        // The `F` window-size / `f` date-filter editors own keystrokes while open, like the other
        // sub-flows. Both re-fetch on submit (X or D changed), so they need the wall-clock
        // `today_day` for the query's default anchor when no filter day is set (ADR-0015).
        if self.editing_window.is_some() {
            return self.handle_window_edit_event(event, session, today_day);
        }
        if self.filtering_date.is_some() {
            return self.handle_date_filter_event(event, session, today_day);
        }
        if self.detail.is_some() {
            return self.handle_detail_event(event, session);
        }
        // A delete confirmation captures input until confirmed (`Submit`) or cancelled; any other
        // list action disarms it so a stray keypress can never delete.
        if self.confirming_delete.is_some() {
            return self.handle_delete_confirm_event(event, session);
        }
        // The today/older split and every selection operation anchor on `A` (the selected filter
        // day, else today; ADR-0015), not the raw wall-clock day.
        let anchor = self.anchor_day(today_day);
        match event {
            Event::Next => self.move_selection(anchor, true),
            Event::Prev => self.move_selection(anchor, false),
            Event::BeginAddTask => {
                self.message = None;
                self.adding = Some(AddTaskState::new());
            }
            Event::BeginAddSubtask => self.begin_add_subtask(anchor),
            // `e` edits the selected sub-task's title when a sub-task row is selected, else the
            // selected task (the existing task edit sub-flow).
            Event::BeginEditTask => {
                if self.selected_subtask(anchor).is_some() {
                    self.begin_edit_subtask(anchor);
                } else {
                    self.begin_edit(anchor);
                }
            }
            // `Space` toggles the selected sub-task when a sub-task row is selected, else the task.
            Event::ToggleDone => {
                if self.selected_subtask(anchor).is_some() {
                    return self.toggle_subtask_done(session, anchor);
                }
                return self.toggle_done(session, anchor);
            }
            // `x` toggles collapse for the parent task of the current selection (ADR-0012 §5).
            Event::ToggleCollapse => self.toggle_collapse(anchor),
            // `h` hides / shows the created-before-today ("older") group and its separator
            // (acceptance #4); re-clamps the selection so it never points past the new row count.
            Event::ToggleHideOlder => self.toggle_hide_older(anchor),
            // `F` opens the window-size editor; `f` opens the date-filter editor seeded to today
            // (ADR-0015). The date editor seeds to the real wall-clock day, not the anchor.
            Event::BeginEditWindow => self.begin_edit_window(),
            Event::BeginFilterDate => self.begin_filter_date(today_day),
            // `Enter` on a task row opens its detail view (chaining a per-task sub-task load for
            // the "Sub-tasks" section); on a sub-task row it is inert (a sub-task has no detail
            // view, ADR-0012 §1 / A8).
            Event::Submit => return self.open_detail(session, anchor),
            Event::DeleteSelected => self.arm_delete(anchor),
            Event::Refresh => return self.refresh(session, today_day),
            _ => {}
        }
        None
    }

    /// Begin the `F` window-size editor, seeded with the current window size.
    fn begin_edit_window(&mut self) {
        self.message = None;
        self.editing_window = Some(WindowEditState::new(self.hide_window_days));
    }

    /// Begin the `f` date-filter editor, seeded to `today_day` (the wall-clock day; feature 0023 /
    /// acceptance #3 — "opens with today's date selected").
    fn begin_filter_date(&mut self, today_day: i64) {
        self.message = None;
        self.filtering_date = Some(DateFilterState::new(today_day));
    }

    /// Handle a key while the `F` window-size editor is open: `Submit` parses + applies the new
    /// window and re-fetches; `Cancel` abandons it; `Char`/`Backspace` edit the numeric buffer.
    fn handle_window_edit_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
        today_day: i64,
    ) -> Option<ClientRequest> {
        let Some(edit) = &mut self.editing_window else {
            return None;
        };
        match event {
            Event::Char(c) => edit.push_char(c),
            Event::Backspace => edit.backspace(),
            Event::Cancel => self.editing_window = None,
            Event::Submit => return self.submit_window_edit(session, today_day),
            other => {
                let _ = edit.motion(&other);
            }
        }
        None
    }

    /// Parse and apply the window-size buffer, re-fetching with the new window. A non-numeric value
    /// or one below [`MIN_HIDE_WINDOW_DAYS`] (`0`) surfaces an inline error and issues no request —
    /// today-only is reached with the `h` toggle, not an `X = 0` mode (operator, 2026-07-08).
    fn submit_window_edit(
        &mut self,
        session: Option<&Session>,
        today_day: i64,
    ) -> Option<ClientRequest> {
        let buffer = self
            .editing_window
            .as_ref()?
            .buffer
            .as_str()
            .trim()
            .to_owned();
        match buffer.parse::<u32>() {
            Ok(days) if days >= MIN_HIDE_WINDOW_DAYS => {
                self.hide_window_days = days;
                self.editing_window = None;
                self.refresh(session, today_day)
            }
            Ok(_) => {
                self.set_window_error("window must be at least 1 day");
                None
            }
            Err(_) => {
                self.set_window_error("window must be a whole number of days");
                None
            }
        }
    }

    /// Set the inline error on the open window editor (a no-op if it closed).
    fn set_window_error(&mut self, message: &str) {
        if let Some(edit) = &mut self.editing_window {
            edit.error = Some(message.to_owned());
        }
    }

    /// Handle a key while the `f` date-filter editor is open: `Next`/`Prev` (Tab/Shift+Tab) cycle
    /// the focused component; `IncrementField`/`DecrementField` (Up/Down) adjust it with
    /// wrap-in-place; `Submit` applies the selected day and re-fetches; `Cancel` abandons it.
    fn handle_date_filter_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
        today_day: i64,
    ) -> Option<ClientRequest> {
        let Some(filter) = &mut self.filtering_date else {
            return None;
        };
        match event {
            Event::Next => filter.cycle(true),
            Event::Prev => filter.cycle(false),
            Event::IncrementField => filter.increment(),
            Event::DecrementField => filter.decrement(),
            Event::Cancel => self.filtering_date = None,
            Event::Submit => return self.submit_date_filter(session, today_day),
            _ => {}
        }
        None
    }

    /// Apply the selected `(day, month, year)` as the filter day-number, re-anchoring the window and
    /// re-fetching (ADR-0015). No calendar validation (an out-of-range day/month still yields a
    /// deterministic day-number via [`crate::ui::days_from_civil`]).
    fn submit_date_filter(
        &mut self,
        session: Option<&Session>,
        today_day: i64,
    ) -> Option<ClientRequest> {
        let day_number = {
            let filter = self.filtering_date.as_ref()?;
            crate::ui::days_from_civil(filter.year, filter.month, filter.day)
        };
        self.filter_date = Some(day_number);
        self.filtering_date = None;
        self.refresh(session, today_day)
    }

    /// Toggle whether the older group + separator are shown (`h`, acceptance #4). Hiding rows can
    /// leave the selection index past the shortened row list, so re-clamp it (and skip onto a
    /// selectable row) afterwards.
    fn toggle_hide_older(&mut self, today_day: i64) {
        self.hide_older = !self.hide_older;
        self.clamp_selection(today_day);
    }

    /// Re-clamp the selection into the current row list, moving off the separator onto a selectable
    /// row if it lands there. Used after a row-count change (the `h` toggle).
    fn clamp_selection(&mut self, today_day: i64) {
        let rows = self.visible_rows(today_day);
        match rows.len() {
            0 => self.selected = None,
            len => {
                let mut i = self.selected.unwrap_or(0).min(len - 1);
                if matches!(rows.get(i), Some(VisibleRow::OlderSeparator)) {
                    // The separator is the last row only when the older group is shown; stepping
                    // back lands on the final today row (or wraps to a selectable row).
                    i = i.saturating_sub(1);
                }
                self.selected = Some(i);
            }
        }
    }

    /// Open the per-field detail view for the selected **task** (a no-op on a sub-task row — a
    /// sub-task has no detail view, ADR-0012 §1). The list is itself server-derived, so the detail
    /// opens from the already-loaded in-memory snapshot (Assumption A3); commits re-derive it from
    /// a fresh list refresh (#1). Chains a per-task `ListTaskSubtasks` so the detail's read-only
    /// "Sub-tasks" section reflects a server response (A6). A no-op with nothing selected.
    fn open_detail(&mut self, session: Option<&Session>, today_day: i64) -> Option<ClientRequest> {
        let task = self.selected_task(today_day)?.clone();
        self.message = None;
        self.detail = Some(TaskDetail::new(task.clone()));
        let session = session?;
        Some(ClientRequest::ListTaskSubtasks {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: task.id,
        })
    }

    /// Begin the add-sub-task sub-flow for the parent task of the current selection (the selected
    /// task, or the selected sub-task's parent). A no-op with nothing selected.
    fn begin_add_subtask(&mut self, today_day: i64) {
        if let Some(task_id) = self.parent_task_id_of_selection(today_day) {
            self.message = None;
            self.adding_subtask = Some(AddSubtaskState::new(task_id));
        }
    }

    /// Begin editing the selected sub-task's title, pre-filled from its current value.
    fn begin_edit_subtask(&mut self, today_day: i64) {
        let Some(state) = self.selected_subtask(today_day).map(EditSubtaskState::new) else {
            return;
        };
        self.message = None;
        self.editing_subtask = Some(state);
    }

    /// Toggle collapse/expand for the parent task of the current selection: records an in-session
    /// override that is the inverse of the current resolved collapse state (ADR-0012 §5, A4). A
    /// no-op with nothing selected.
    fn toggle_collapse(&mut self, today_day: i64) {
        let Some(task_id) = self.parent_task_id_of_selection(today_day) else {
            return;
        };
        let Some(task) = self.tasks.iter().find(|t| t.id == task_id).cloned() else {
            return;
        };
        // Flip against the group-aware resolved state so `x` on an older task toggles against its
        // `true` (collapsed) default just like an open today task toggles against `false`.
        let next = !self.resolve_collapsed(&task, today_day);
        let _ = self.collapse_overrides.insert(task_id, next);
    }

    /// Arm the delete confirmation for the current selection by its **row kind** (the first `d`),
    /// opening the confirm dialog (ADR-0010 §4, Assumption A5): a task row arms a
    /// [`DeleteTarget::Task`], a sub-task row a [`DeleteTarget::Subtask`] (carrying its parent). The
    /// second key (`Enter`) confirms via [`Self::handle_delete_confirm_event`]. A no-op on the
    /// separator or with nothing selected.
    fn arm_delete(&mut self, today_day: i64) {
        let target = match self.selected_row(today_day) {
            Some(VisibleRow::Task { task_idx }) => {
                self.tasks.get(task_idx).map(|t| DeleteTarget::Task {
                    task_id: t.id.clone(),
                })
            }
            Some(VisibleRow::Subtask { subtask_idx }) => {
                self.subtasks
                    .get(subtask_idx)
                    .map(|s| DeleteTarget::Subtask {
                        task_id: s.task_id.clone(),
                        subtask_id: s.id.clone(),
                    })
            }
            Some(VisibleRow::OlderSeparator) | None => None,
        };
        let Some(target) = target else {
            return;
        };
        self.message = None;
        self.confirming_delete = Some(target);
    }

    /// Handle a key while the delete-confirm dialog is armed: `Submit` (Enter) confirms the delete;
    /// `Cancel` (Esc, routed by the caller) disarms; everything else is inert.
    fn handle_delete_confirm_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        match event {
            Event::Submit => self.confirm_delete(session),
            Event::Cancel => {
                self.confirming_delete = None;
                None
            }
            _ => None,
        }
    }

    /// Handle a key while the per-field detail view is open (ADR-0010 §4). Two-tiered `Esc`: while
    /// editing a field, `Cancel` reverts the edit; with no edit, `Cancel` exits to the list. `e`
    /// opens the edit buffer on the focused editable pane; `Next`/`Prev` cycle panes when not
    /// editing; `Char`/`Backspace` mutate the buffer; `Submit` commits the focused field.
    fn handle_detail_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(detail) = &mut self.detail else {
            return None;
        };
        if detail.is_editing() {
            match event {
                Event::Char(c) => detail.push_char(c),
                Event::Backspace => detail.backspace(),
                Event::Cancel => detail.cancel_edit(),
                Event::Submit => return self.submit_field(session),
                other => {
                    let _ = detail.edit_motion(&other);
                }
            }
            return None;
        }
        match event {
            Event::Next => detail.cycle(true),
            Event::Prev => detail.cycle(false),
            Event::BeginEditTask => detail.begin_edit(),
            Event::Cancel => self.detail = None,
            _ => {}
        }
        None
    }

    /// Commit the focused detail field via [`UpdateTaskRequest`] with **only** the edited field set
    /// (the request's other fields stay `None`, ADR-0010 §4). A no-op if not editing a field. A
    /// blank title is rejected locally without a round-trip (mirrors the edit dialog).
    fn submit_field(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let detail = self.detail.as_mut()?;
        let buffer = detail.edit.as_ref()?.as_str().to_owned();
        let req = match detail.focused_pane()? {
            TaskPane::Title => {
                if buffer.trim().is_empty() {
                    return None;
                }
                UpdateTaskRequest {
                    title: Some(buffer.trim().to_owned()),
                    description: None,
                    status: None,
                }
            }
            TaskPane::Description => UpdateTaskRequest {
                title: None,
                description: Some(buffer),
                status: None,
            },
            TaskPane::Status | TaskPane::Created | TaskPane::Closed => return None,
        };
        Some(ClientRequest::UpdateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: detail.task.id.clone(),
            req,
        })
    }

    fn handle_add_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(add) = &mut self.adding else {
            return None;
        };
        match event {
            Event::Char(c) => add.push_char(c),
            Event::Backspace => add.backspace(),
            Event::Next | Event::Prev => add.toggle_field(),
            Event::Cancel => self.adding = None,
            Event::Submit => return self.submit_add(session),
            other => {
                let _ = add.motion(&other);
            }
        }
        None
    }

    fn submit_add(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let add = self.adding.as_mut()?;
        add.error = None;
        let req = CreateTaskRequest {
            title: add.title.as_str().trim().to_owned(),
            description: add.description.as_str().to_owned(),
        };
        Some(ClientRequest::CreateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            req,
        })
    }

    /// Handle a key while the add-sub-task form is open: a single title field. `Submit` issues the
    /// create; `Cancel` abandons it; `Char`/`Backspace` edit the title.
    fn handle_add_subtask_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(add) = &mut self.adding_subtask else {
            return None;
        };
        match event {
            Event::Char(c) => add.push_char(c),
            Event::Backspace => add.backspace(),
            Event::Cancel => self.adding_subtask = None,
            Event::Submit => return self.submit_add_subtask(session),
            other => {
                let _ = add.motion(&other);
            }
        }
        None
    }

    /// Submit the add-sub-task form as a [`CreateSubtaskRequest`]. A blank title (after trimming)
    /// is rejected locally without a round-trip (mirrors add-task).
    fn submit_add_subtask(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let add = self.adding_subtask.as_mut()?;
        if add.title.as_str().trim().is_empty() {
            add.error = Some("title must not be empty".to_owned());
            return None;
        }
        add.error = None;
        let req = CreateSubtaskRequest {
            title: add.title.as_str().trim().to_owned(),
        };
        Some(ClientRequest::CreateSubtask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: add.task_id.clone(),
            req,
        })
    }

    /// Handle a key while the edit-sub-task-title form is open: a single title field. `Submit`
    /// issues the patch; `Cancel` abandons it; `Char`/`Backspace` edit the title.
    fn handle_edit_subtask_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(edit) = &mut self.editing_subtask else {
            return None;
        };
        match event {
            Event::Char(c) => edit.push_char(c),
            Event::Backspace => edit.backspace(),
            Event::Cancel => self.editing_subtask = None,
            Event::Submit => return self.submit_edit_subtask(session),
            other => {
                let _ = edit.motion(&other);
            }
        }
        None
    }

    /// Submit the edit-sub-task form as a title-only [`UpdateSubtaskRequest`]. A blank title (after
    /// trimming) is rejected locally without a round-trip.
    fn submit_edit_subtask(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let edit = self.editing_subtask.as_mut()?;
        if edit.title.as_str().trim().is_empty() {
            edit.error = Some("title must not be empty".to_owned());
            return None;
        }
        edit.error = None;
        let req = UpdateSubtaskRequest {
            title: Some(edit.title.as_str().trim().to_owned()),
            status: None,
        };
        Some(ClientRequest::UpdateSubtask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: edit.task_id.clone(),
            subtask_id: edit.subtask_id.clone(),
            req,
        })
    }

    fn handle_edit_event(
        &mut self,
        event: Event,
        session: Option<&Session>,
    ) -> Option<ClientRequest> {
        let Some(edit) = &mut self.editing else {
            return None;
        };
        match event {
            Event::Char(c) => edit.push_char(c),
            Event::Backspace => edit.backspace(),
            Event::Next | Event::Prev => edit.toggle_field(),
            Event::Cancel => self.editing = None,
            Event::Submit => return self.submit_edit(session),
            other => {
                let _ = edit.motion(&other);
            }
        }
        None
    }

    /// Open the edit sub-flow for the selected task, pre-filled from its current values.
    fn begin_edit(&mut self, today_day: i64) {
        let Some(state) = self.selected_task(today_day).map(EditTaskState::new) else {
            return;
        };
        self.message = None;
        self.editing = Some(state);
    }

    /// Submit the edit sub-flow as a title+description [`UpdateTaskRequest`]. Mirrors add-task's
    /// inline validation: a blank title (after trimming) is rejected locally without a round-trip.
    fn submit_edit(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        let edit = self.editing.as_mut()?;
        if edit.title.as_str().trim().is_empty() {
            edit.error = Some("title must not be empty".to_owned());
            return None;
        }
        edit.error = None;
        let req = UpdateTaskRequest {
            title: Some(edit.title.as_str().trim().to_owned()),
            description: Some(edit.description.as_str().to_owned()),
            status: None,
        };
        Some(ClientRequest::UpdateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: edit.task_id.clone(),
            req,
        })
    }

    /// Toggle the selected task's status: a done task is reopened (`status: open`, clears
    /// `closed_at` server-side), an open task is marked done (`status: done`).
    fn toggle_done(&mut self, session: Option<&Session>, today_day: i64) -> Option<ClientRequest> {
        let session = session?;
        let task = self.selected_task(today_day)?;
        let next = match task.status {
            TaskStatus::Open => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Open,
        };
        let req = UpdateTaskRequest {
            title: None,
            description: None,
            status: Some(next),
        };
        Some(ClientRequest::UpdateTask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: task.id.clone(),
            req,
        })
    }

    /// Toggle the selected sub-task's status: a done sub-task is reopened, an open one is marked
    /// done (a plain status flip — a sub-task has no `closed_at`).
    fn toggle_subtask_done(
        &mut self,
        session: Option<&Session>,
        today_day: i64,
    ) -> Option<ClientRequest> {
        let session = session?;
        let subtask = self.selected_subtask(today_day)?;
        let next = match subtask.status {
            TaskStatus::Open => TaskStatus::Done,
            TaskStatus::Done => TaskStatus::Open,
        };
        let req = UpdateSubtaskRequest {
            title: None,
            status: Some(next),
        };
        Some(ClientRequest::UpdateSubtask {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            task_id: subtask.task_id.clone(),
            subtask_id: subtask.id.clone(),
            req,
        })
    }

    /// Issue the delete for the armed target (the confirm dialog's `Enter`): a
    /// [`ClientRequest::DeleteTask`] for a task, a [`ClientRequest::DeleteSubtask`] for a sub-task.
    /// A no-op if nothing is armed or the session is gone.
    fn confirm_delete(&mut self, session: Option<&Session>) -> Option<ClientRequest> {
        let session = session?;
        match self.confirming_delete.clone()? {
            DeleteTarget::Task { task_id } => Some(ClientRequest::DeleteTask {
                token: session.token.clone(),
                profile_id: session.profile_id.clone(),
                task_id,
            }),
            DeleteTarget::Subtask {
                task_id,
                subtask_id,
            } => Some(ClientRequest::DeleteSubtask {
                token: session.token.clone(),
                profile_id: session.profile_id.clone(),
                task_id,
                subtask_id,
            }),
        }
    }

    fn refresh(&mut self, session: Option<&Session>, today_day: i64) -> Option<ClientRequest> {
        let session = session?;
        Some(ClientRequest::ListTasks {
            token: session.token.clone(),
            profile_id: session.profile_id.clone(),
            query: super::windowed_task_list_query(
                self.anchor_day(today_day),
                self.hide_window_days,
            ),
        })
    }
}
