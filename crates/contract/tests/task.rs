//! Wire-format and round-trip tests for the task DTOs (`Task`, `TaskStatus`,
//! `CreateTaskRequest`), locking the ADR-0005 conventions: snake_case fields, a UUID-string
//! id, RFC 3339 UTC timestamps, lowercase status enum, and nullable `closed_at`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use contract::{CreateTaskRequest, Task, TaskStatus};
use serde_json::{Value, json};

const TASK_ID: &str = "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b";
const CREATED_AT: &str = "2026-06-11T12:00:00Z";
const CLOSED_AT: &str = "2026-06-11T13:30:00Z";

// --- TaskStatus: lowercase enum strings. ---

#[test]
fn task_status_serializes_lowercase() {
    assert_eq!(
        serde_json::to_string(&TaskStatus::Open).unwrap(),
        r#""open""#
    );
    assert_eq!(
        serde_json::to_string(&TaskStatus::Done).unwrap(),
        r#""done""#
    );
}

#[test]
fn task_status_deserializes_lowercase() {
    assert_eq!(
        serde_json::from_str::<TaskStatus>(r#""open""#).unwrap(),
        TaskStatus::Open
    );
    assert_eq!(
        serde_json::from_str::<TaskStatus>(r#""done""#).unwrap(),
        TaskStatus::Done
    );
}

#[test]
fn task_status_rejects_non_lowercase_or_unknown() {
    // The enum is closed: only the two lowercase variants are valid on the wire.
    assert!(serde_json::from_str::<TaskStatus>(r#""Open""#).is_err());
    assert!(serde_json::from_str::<TaskStatus>(r#""OPEN""#).is_err());
    assert!(serde_json::from_str::<TaskStatus>(r#""pending""#).is_err());
}

#[test]
fn task_status_round_trips() {
    for status in [TaskStatus::Open, TaskStatus::Done] {
        let wire = serde_json::to_string(&status).unwrap();
        let back: TaskStatus = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, status);
    }
}

// --- Task: full wire shape, open and done. ---

#[test]
fn open_task_serializes_with_closed_at_present_as_null() {
    let task = Task {
        id: TASK_ID.to_owned(),
        title: "Write the contract crate".to_owned(),
        description: "ADR-0005 DTOs".to_owned(),
        status: TaskStatus::Open,
        created_at: CREATED_AT.to_owned(),
        closed_at: None,
    };
    let json = serde_json::to_value(&task).unwrap();
    assert_eq!(
        json,
        json!({
            "id": TASK_ID,
            "title": "Write the contract crate",
            "description": "ADR-0005 DTOs",
            "status": "open",
            "created_at": CREATED_AT,
            "closed_at": null,
        })
    );
    // Lock the contract: `closed_at` is EMITTED as an explicit null (the key is present),
    // not skipped. Server and TUI both rely on the key always being there.
    let object = json.as_object().unwrap();
    assert!(object.contains_key("closed_at"));
    assert!(object.get("closed_at").unwrap().is_null());
}

#[test]
fn done_task_serializes_with_closed_at_string() {
    let task = Task {
        id: TASK_ID.to_owned(),
        title: "Write the contract crate".to_owned(),
        description: String::new(),
        status: TaskStatus::Done,
        created_at: CREATED_AT.to_owned(),
        closed_at: Some(CLOSED_AT.to_owned()),
    };
    let json = serde_json::to_value(&task).unwrap();
    let object = json.as_object().unwrap();
    assert_eq!(object.get("status").unwrap(), "done");
    assert_eq!(object.get("closed_at").unwrap(), CLOSED_AT);
    // The close timestamp travels as an RFC 3339 UTC string.
    assert!(object.get("closed_at").unwrap().is_string());
    // An empty description is preserved (description may be empty per ADR-0005).
    assert_eq!(object.get("description").unwrap(), "");
}

#[test]
fn open_task_deserializes_from_explicit_null_closed_at() {
    let wire = json!({
        "id": TASK_ID,
        "title": "t",
        "description": "d",
        "status": "open",
        "created_at": CREATED_AT,
        "closed_at": null,
    });
    let task: Task = serde_json::from_value(wire).unwrap();
    assert_eq!(task.status, TaskStatus::Open);
    assert!(task.closed_at.is_none());
}

#[test]
fn task_tolerates_an_absent_closed_at() {
    // `Option<String>` defaults to `None` when the key is absent, so a producer that omits
    // the key (rather than sending null) still parses — defensive forward compatibility.
    let wire = json!({
        "id": TASK_ID,
        "title": "t",
        "description": "d",
        "status": "open",
        "created_at": CREATED_AT,
    });
    let task: Task = serde_json::from_value(wire).unwrap();
    assert!(task.closed_at.is_none());
}

#[test]
fn done_task_round_trips_losslessly() {
    let task = Task {
        id: TASK_ID.to_owned(),
        title: "Write the contract crate".to_owned(),
        description: "ADR-0005 DTOs".to_owned(),
        status: TaskStatus::Done,
        created_at: CREATED_AT.to_owned(),
        closed_at: Some(CLOSED_AT.to_owned()),
    };
    let wire = serde_json::to_string(&task).unwrap();
    let back: Task = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, task);
}

#[test]
fn open_task_round_trips_losslessly() {
    let task = Task {
        id: TASK_ID.to_owned(),
        title: "Write the contract crate".to_owned(),
        description: "ADR-0005 DTOs".to_owned(),
        status: TaskStatus::Open,
        created_at: CREATED_AT.to_owned(),
        closed_at: None,
    };
    let wire = serde_json::to_string(&task).unwrap();
    let back: Task = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, task);
}

#[test]
fn task_list_deserializes_as_a_bare_array() {
    // The list endpoint returns a bare JSON array (no envelope), newest-first.
    let wire = json!([
        {
            "id": TASK_ID,
            "title": "newer",
            "description": "",
            "status": "open",
            "created_at": "2026-06-11T14:00:00Z",
            "closed_at": null,
        },
        {
            "id": "11111111-2222-3333-4444-555555555555",
            "title": "older",
            "description": "",
            "status": "done",
            "created_at": CREATED_AT,
            "closed_at": CLOSED_AT,
        }
    ]);
    let tasks: Vec<Task> = serde_json::from_value(wire).unwrap();
    assert_eq!(tasks.len(), 2);
    let newer = tasks.first().unwrap();
    let older = tasks.get(1).unwrap();
    assert_eq!(newer.title, "newer");
    assert_eq!(newer.status, TaskStatus::Open);
    assert_eq!(older.status, TaskStatus::Done);
    assert_eq!(older.closed_at.as_deref(), Some(CLOSED_AT));
}

// --- CreateTaskRequest: minimal create body. ---

#[test]
fn create_task_request_serializes_snake_case_fields() {
    let req = CreateTaskRequest {
        title: "Write the contract crate".to_owned(),
        description: "ADR-0005 DTOs".to_owned(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "title": "Write the contract crate",
            "description": "ADR-0005 DTOs",
        })
    );
}

#[test]
fn create_task_request_round_trips() {
    let wire = r#"{"title":"t","description":""}"#;
    let req: CreateTaskRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.title, "t");
    assert_eq!(req.description, "");
    let reserialized: Value = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    let original: Value = serde_json::from_str(wire).unwrap();
    assert_eq!(reserialized, original);
}
