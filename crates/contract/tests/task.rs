//! Wire-format and round-trip tests for the task DTOs (`Task`, `TaskStatus`,
//! `CreateTaskRequest`), locking the ADR-0005 conventions: snake_case fields, a UUID-string
//! id, RFC 3339 UTC timestamps, lowercase status enum, and nullable `closed_at`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use chrono::{DateTime, Utc};
use contract::{
    CreateTaskRequest, MAX_TASK_LIST_LIMIT, Task, TaskListQuery, TaskStatus, UpdateTaskRequest,
};
use serde_json::{Value, json};

const TASK_ID: &str = "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b";
const CREATED_AT: &str = "2026-06-11T12:00:00Z";
const CLOSED_AT: &str = "2026-06-11T13:30:00Z";

/// Parse the canonical `created_at` const into a typed timestamp for struct construction.
/// (`DateTime` has no `const` parse, so the typed values live in `let` bindings.)
fn created_at() -> DateTime<Utc> {
    CREATED_AT.parse().unwrap()
}

/// Parse the canonical `closed_at` const into a typed timestamp for struct construction.
fn closed_at() -> DateTime<Utc> {
    CLOSED_AT.parse().unwrap()
}

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
        created_at: created_at(),
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
        created_at: created_at(),
        closed_at: Some(closed_at()),
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
        created_at: created_at(),
        closed_at: Some(closed_at()),
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
        created_at: created_at(),
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
    assert_eq!(older.closed_at, Some(closed_at()));
}

// --- Typed-timestamp parsing: the `DateTime<Utc>` field validates on the wire. ---

#[test]
fn task_rejects_a_malformed_created_at() {
    // The typed `created_at` rejects a non-RFC-3339 string at deserialize time — behaviour the
    // old `String` field could not give us.
    let wire = json!({
        "id": TASK_ID,
        "title": "t",
        "description": "d",
        "status": "open",
        "created_at": "not-a-date",
        "closed_at": null,
    });
    assert!(serde_json::from_value::<Task>(wire).is_err());
}

#[test]
fn task_rejects_a_malformed_closed_at() {
    // A present-but-malformed `closed_at` is also rejected (it is `Option<DateTime<Utc>>`, so a
    // non-null value must still parse).
    let wire = json!({
        "id": TASK_ID,
        "title": "t",
        "description": "d",
        "status": "done",
        "created_at": CREATED_AT,
        "closed_at": "13:30 yesterday",
    });
    assert!(serde_json::from_value::<Task>(wire).is_err());
}

#[test]
fn task_normalizes_an_offset_bearing_created_at_to_utc() {
    // An RFC 3339 input carrying a non-Z offset is accepted and normalized to UTC, so it
    // re-serializes with the canonical `Z` suffix. `11:00:00+01:00` is `10:00:00Z`.
    let wire = json!({
        "id": TASK_ID,
        "title": "t",
        "description": "d",
        "status": "open",
        "created_at": "2026-06-11T11:00:00+01:00",
        "closed_at": null,
    });
    let task: Task = serde_json::from_value(wire).unwrap();
    assert_eq!(
        task.created_at,
        "2026-06-11T10:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    let reserialized = serde_json::to_value(&task).unwrap();
    assert_eq!(
        reserialized.get("created_at").unwrap(),
        "2026-06-11T10:00:00Z"
    );
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

// --- UpdateTaskRequest: all-optional partial patch (ADR-0007, slice 1 / A1). ---

#[test]
fn update_task_request_full_patch_round_trips() {
    // All three fields set: serialize → deserialize is lossless.
    let req = UpdateTaskRequest {
        title: Some("Refined title".to_owned()),
        description: Some("Refined description".to_owned()),
        status: Some(TaskStatus::Done),
    };
    let wire = serde_json::to_string(&req).unwrap();
    let back: UpdateTaskRequest = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, req);
}

#[test]
fn update_task_request_title_only_omits_absent_fields() {
    // A single-field patch serializes with ONLY that key present; absent `Option`s are skipped.
    let req = UpdateTaskRequest {
        title: Some("Refined title".to_owned()),
        description: None,
        status: None,
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "title": "Refined title",
        })
    );
    let object = json.as_object().unwrap();
    assert!(object.contains_key("title"));
    assert!(!object.contains_key("description"));
    assert!(!object.contains_key("status"));
}

#[test]
fn update_task_request_empty_patch_serializes_to_empty_object() {
    // The default (no fields set) serializes to exactly `{}` — a no-op patch on the wire.
    let req = UpdateTaskRequest::default();
    assert_eq!(serde_json::to_string(&req).unwrap(), "{}");
}

#[test]
fn update_task_request_status_only_reopen_round_trips() {
    // A reopen-style patch carries only `status: open`; absent fields stay omitted, and it
    // round-trips losslessly.
    let req = UpdateTaskRequest {
        title: None,
        description: None,
        status: Some(TaskStatus::Open),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "status": "open",
        })
    );
    let wire = serde_json::to_string(&req).unwrap();
    let back: UpdateTaskRequest = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, req);
}

#[test]
fn update_task_request_deserializes_partial_object_with_absent_fields_none() {
    // Deserializing a partial JSON object leaves absent fields `None` — a description-only patch.
    let wire = json!({
        "description": "just the description",
    });
    let req: UpdateTaskRequest = serde_json::from_value(wire).unwrap();
    assert_eq!(req.description, Some("just the description".to_owned()));
    assert!(req.title.is_none());
    assert!(req.status.is_none());

    // An empty object deserializes to the all-`None` default.
    let empty: UpdateTaskRequest = serde_json::from_value(json!({})).unwrap();
    assert_eq!(empty, UpdateTaskRequest::default());
}

// --- TaskListQuery: pagination-ready limit + offset query params (ADR-0014 §1/§2). ---

#[test]
fn max_task_list_limit_is_the_ceiling_constant() {
    // The ADR-0014 ceiling the server enforces; the value is part of the wire contract.
    assert_eq!(MAX_TASK_LIST_LIMIT, 500);
}

#[test]
fn task_list_query_all_none_serializes_to_empty_query_string() {
    // Both params absent: the default carries nothing, so it serializes to an EMPTY query string —
    // an old no-param caller's request is byte-identical to before this feature (additive shape).
    let query = TaskListQuery::default();
    assert_eq!(serde_urlencoded::to_string(&query).unwrap(), "");
    assert!(query.limit.is_none());
    assert!(query.offset.is_none());
}

#[test]
fn task_list_query_limit_only_omits_offset() {
    // A `limit`-only query omits `offset` entirely (skip_serializing_if).
    let query = TaskListQuery {
        limit: Some(200),
        offset: None,
    };
    assert_eq!(serde_urlencoded::to_string(&query).unwrap(), "limit=200");
}

#[test]
fn task_list_query_offset_only_omits_limit() {
    // Symmetric: an `offset`-only query omits `limit`.
    let query = TaskListQuery {
        limit: None,
        offset: Some(40),
    };
    assert_eq!(serde_urlencoded::to_string(&query).unwrap(), "offset=40");
}

#[test]
fn task_list_query_both_present_serialize_together() {
    let query = TaskListQuery {
        limit: Some(200),
        offset: Some(400),
    };
    assert_eq!(
        serde_urlencoded::to_string(&query).unwrap(),
        "limit=200&offset=400"
    );
}

#[test]
fn task_list_query_round_trips_through_query_string() {
    // Serialize → parse is lossless over the query-param encoding (the reqwest `.query()` path).
    let query = TaskListQuery {
        limit: Some(123),
        offset: Some(7),
    };
    let encoded = serde_urlencoded::to_string(&query).unwrap();
    let back: TaskListQuery = serde_urlencoded::from_str(&encoded).unwrap();
    assert_eq!(back, query);
}

#[test]
fn task_list_query_deserializes_from_partial_and_empty_query_strings() {
    // An empty query string yields the all-`None` default (the server then applies its defaults).
    let empty: TaskListQuery = serde_urlencoded::from_str("").unwrap();
    assert_eq!(empty, TaskListQuery::default());

    // A `limit`-only string leaves `offset` as `None`.
    let limit_only: TaskListQuery = serde_urlencoded::from_str("limit=200").unwrap();
    assert_eq!(limit_only.limit, Some(200));
    assert!(limit_only.offset.is_none());
}
