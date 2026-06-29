//! Wire-format and round-trip tests for the sub-task DTOs (`Subtask`, `CreateSubtaskRequest`,
//! `UpdateSubtaskRequest`), locking the ADR-0012/0013 conventions: snake_case fields, UUID-string
//! `id`/`task_id`, the reused lowercase `TaskStatus` enum, **no** description/timestamps, and the
//! all-optional `UpdateSubtaskRequest` partial (absent fields omitted via `skip_serializing_if`,
//! empty patch `{}` ⇒ all-`None`).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use contract::{CreateSubtaskRequest, Subtask, TaskStatus, UpdateSubtaskRequest};
use serde_json::{Value, json};

const SUBTASK_ID: &str = "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b";
const TASK_ID: &str = "1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d";

// --- Subtask: full wire shape (title + status only, no timestamps/description). ---

#[test]
fn subtask_serializes_snake_case_with_only_the_four_fields() {
    let subtask = Subtask {
        id: SUBTASK_ID.to_owned(),
        task_id: TASK_ID.to_owned(),
        title: "Draft the migration".to_owned(),
        status: TaskStatus::Open,
    };
    let json = serde_json::to_value(&subtask).unwrap();
    assert_eq!(
        json,
        json!({
            "id": SUBTASK_ID,
            "task_id": TASK_ID,
            "title": "Draft the migration",
            "status": "open",
        })
    );
    // ADR-0012 §1: a sub-task carries NO description and NO timestamps. Lock that the wire shape
    // has exactly these four keys — nothing leaks in.
    let object = json.as_object().unwrap();
    assert_eq!(
        object.len(),
        4,
        "exactly id/task_id/title/status: {object:?}"
    );
    assert!(!object.contains_key("description"));
    assert!(!object.contains_key("created_at"));
    assert!(!object.contains_key("closed_at"));
    assert!(!object.contains_key("updated_at"));
    // Scoping linkage is task_id (the parent), never a profile id on the sub-task itself (#4).
    assert!(!object.contains_key("profile_id"));
}

#[test]
fn open_subtask_deserializes_from_canonical_json() {
    let wire = json!({
        "id": SUBTASK_ID,
        "task_id": TASK_ID,
        "title": "Draft the migration",
        "status": "open",
    });
    let subtask: Subtask = serde_json::from_value(wire).unwrap();
    assert_eq!(subtask.id, SUBTASK_ID);
    assert_eq!(subtask.task_id, TASK_ID);
    assert_eq!(subtask.title, "Draft the migration");
    assert_eq!(subtask.status, TaskStatus::Open);
}

#[test]
fn done_subtask_round_trips_losslessly() {
    let subtask = Subtask {
        id: SUBTASK_ID.to_owned(),
        task_id: TASK_ID.to_owned(),
        title: "Apply it".to_owned(),
        status: TaskStatus::Done,
    };
    let wire = serde_json::to_string(&subtask).unwrap();
    let back: Subtask = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, subtask);
}

#[test]
fn open_subtask_round_trips_losslessly() {
    let subtask = Subtask {
        id: SUBTASK_ID.to_owned(),
        task_id: TASK_ID.to_owned(),
        title: "Draft it".to_owned(),
        status: TaskStatus::Open,
    };
    let wire = serde_json::to_string(&subtask).unwrap();
    let back: Subtask = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, subtask);
}

#[test]
fn subtask_status_rejects_non_lowercase_or_unknown() {
    // The status reuses the closed `TaskStatus` enum: only lowercase open/done parse.
    let make = |status: &str| {
        json!({
            "id": SUBTASK_ID,
            "task_id": TASK_ID,
            "title": "t",
            "status": status,
        })
    };
    assert!(serde_json::from_value::<Subtask>(make("Open")).is_err());
    assert!(serde_json::from_value::<Subtask>(make("DONE")).is_err());
    assert!(serde_json::from_value::<Subtask>(make("pending")).is_err());
}

#[test]
fn subtask_list_deserializes_as_a_bare_array() {
    // The list endpoints return a bare JSON array (no envelope), creation order.
    let wire = json!([
        {
            "id": SUBTASK_ID,
            "task_id": TASK_ID,
            "title": "first",
            "status": "open",
        },
        {
            "id": "11111111-2222-3333-4444-555555555555",
            "task_id": TASK_ID,
            "title": "second",
            "status": "done",
        }
    ]);
    let subtasks: Vec<Subtask> = serde_json::from_value(wire).unwrap();
    assert_eq!(subtasks.len(), 2);
    assert_eq!(subtasks.first().unwrap().title, "first");
    assert_eq!(subtasks.first().unwrap().status, TaskStatus::Open);
    assert_eq!(subtasks.get(1).unwrap().status, TaskStatus::Done);
}

// --- CreateSubtaskRequest: title-only create body. ---

#[test]
fn create_subtask_request_serializes_title_only() {
    let req = CreateSubtaskRequest {
        title: "Draft the migration".to_owned(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json, json!({ "title": "Draft the migration" }));
    // Create carries no status (a new sub-task always starts `open`, server default).
    assert!(!json.as_object().unwrap().contains_key("status"));
}

#[test]
fn create_subtask_request_round_trips() {
    let wire = r#"{"title":"t"}"#;
    let req: CreateSubtaskRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.title, "t");
    let reserialized: Value = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    let original: Value = serde_json::from_str(wire).unwrap();
    assert_eq!(reserialized, original);
}

// --- UpdateSubtaskRequest: all-optional partial patch (skip_serializing_if + empty `{}`). ---

#[test]
fn update_subtask_request_full_patch_round_trips() {
    // Both fields set: serialize → deserialize is lossless.
    let req = UpdateSubtaskRequest {
        title: Some("Refined".to_owned()),
        status: Some(TaskStatus::Done),
    };
    let wire = serde_json::to_string(&req).unwrap();
    let back: UpdateSubtaskRequest = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, req);
}

#[test]
fn update_subtask_request_title_only_omits_status() {
    // The TUI's edit-title patch: ONLY the title key present; the absent `status` is skipped.
    let req = UpdateSubtaskRequest {
        title: Some("Refined".to_owned()),
        status: None,
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json, json!({ "title": "Refined" }));
    let object = json.as_object().unwrap();
    assert!(object.contains_key("title"));
    assert!(!object.contains_key("status"), "absent status is omitted");
}

#[test]
fn update_subtask_request_status_only_omits_title() {
    // The TUI's toggle patch: ONLY the status key present; the absent `title` is skipped.
    let req = UpdateSubtaskRequest {
        title: None,
        status: Some(TaskStatus::Done),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json, json!({ "status": "done" }));
    let object = json.as_object().unwrap();
    assert!(object.contains_key("status"));
    assert!(!object.contains_key("title"), "absent title is omitted");
}

#[test]
fn update_subtask_request_empty_patch_serializes_to_empty_object() {
    // The default (no fields set) serializes to exactly `{}` — a no-op patch on the wire.
    let req = UpdateSubtaskRequest::default();
    assert_eq!(serde_json::to_string(&req).unwrap(), "{}");
}

#[test]
fn update_subtask_request_empty_object_deserializes_to_all_none() {
    // An empty patch `{}` deserializes to the all-`None` default — every field absent.
    let req: UpdateSubtaskRequest = serde_json::from_value(json!({})).unwrap();
    assert!(req.title.is_none());
    assert!(req.status.is_none());
    assert_eq!(req, UpdateSubtaskRequest::default());
}

#[test]
fn update_subtask_request_deserializes_partial_object_with_absent_fields_none() {
    // A status-only patch leaves the absent title `None`.
    let req: UpdateSubtaskRequest = serde_json::from_value(json!({ "status": "open" })).unwrap();
    assert_eq!(req.status, Some(TaskStatus::Open));
    assert!(req.title.is_none());
}
