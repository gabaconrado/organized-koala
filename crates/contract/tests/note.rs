//! Wire-format and round-trip tests for the note DTOs (`Note`, `CreateNoteRequest`,
//! `UpdateNoteRequest`), locking the ADR-0007 conventions: snake_case fields, a UUID-string
//! id, an RFC 3339 UTC `created_at`, and the deliberately-flat shape with NO `updated_at`,
//! status, or `closed_at` (hard-constraint #3).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use chrono::{DateTime, Utc};
use contract::{CreateNoteRequest, Note, UpdateNoteRequest};
use serde_json::{Value, json};

const NOTE_ID: &str = "5f9a2c1e-0b3d-4e6f-8a1b-2c3d4e5f6a7b";
const CREATED_AT: &str = "2026-06-11T12:00:00Z";

/// Parse the canonical `created_at` const into a typed timestamp for struct construction.
/// (`DateTime` has no `const` parse, so the typed value lives in a `let` binding.)
fn created_at() -> DateTime<Utc> {
    CREATED_AT.parse().unwrap()
}

// --- Note: full wire shape, flat. ---

#[test]
fn note_serializes_with_exactly_the_flat_fields() {
    let note = Note {
        id: NOTE_ID.to_owned(),
        title: "Groceries".to_owned(),
        content: "milk, eggs, bread".to_owned(),
        created_at: created_at(),
    };
    let json = serde_json::to_value(&note).unwrap();
    assert_eq!(
        json,
        json!({
            "id": NOTE_ID,
            "title": "Groceries",
            "content": "milk, eggs, bread",
            "created_at": CREATED_AT,
        })
    );
    // Lock the flat contract (#3): exactly four keys, and none of the forbidden ones.
    let object = json.as_object().unwrap();
    assert_eq!(object.len(), 4);
    assert!(object.contains_key("id"));
    assert!(object.contains_key("title"));
    assert!(object.contains_key("content"));
    assert!(object.contains_key("created_at"));
    // No second timestamp and no task-style lifecycle leaked onto notes.
    assert!(!object.contains_key("updated_at"));
    assert!(!object.contains_key("status"));
    assert!(!object.contains_key("closed_at"));
}

#[test]
fn note_serializes_with_empty_content_preserved() {
    let note = Note {
        id: NOTE_ID.to_owned(),
        title: "Empty".to_owned(),
        content: String::new(),
        created_at: created_at(),
    };
    let json = serde_json::to_value(&note).unwrap();
    let object = json.as_object().unwrap();
    // An empty content is preserved (content may be empty per ADR-0007).
    assert_eq!(object.get("content").unwrap(), "");
}

#[test]
fn note_deserializes_from_the_flat_shape() {
    let wire = json!({
        "id": NOTE_ID,
        "title": "Groceries",
        "content": "milk, eggs, bread",
        "created_at": CREATED_AT,
    });
    let note: Note = serde_json::from_value(wire).unwrap();
    assert_eq!(note.id, NOTE_ID);
    assert_eq!(note.title, "Groceries");
    assert_eq!(note.content, "milk, eggs, bread");
    assert_eq!(note.created_at, created_at());
}

#[test]
fn note_round_trips_losslessly() {
    let note = Note {
        id: NOTE_ID.to_owned(),
        title: "Groceries".to_owned(),
        content: "milk, eggs, bread".to_owned(),
        created_at: created_at(),
    };
    let wire = serde_json::to_string(&note).unwrap();
    let back: Note = serde_json::from_str(&wire).unwrap();
    assert_eq!(back, note);
}

#[test]
fn note_list_deserializes_as_a_bare_array() {
    // The list endpoint returns a bare JSON array (no envelope), newest-first.
    let wire = json!([
        {
            "id": NOTE_ID,
            "title": "newer",
            "content": "",
            "created_at": "2026-06-11T14:00:00Z",
        },
        {
            "id": "11111111-2222-3333-4444-555555555555",
            "title": "older",
            "content": "earlier note",
            "created_at": CREATED_AT,
        }
    ]);
    let notes: Vec<Note> = serde_json::from_value(wire).unwrap();
    assert_eq!(notes.len(), 2);
    let newer = notes.first().unwrap();
    let older = notes.get(1).unwrap();
    assert_eq!(newer.title, "newer");
    assert_eq!(newer.content, "");
    assert_eq!(older.title, "older");
    assert_eq!(older.created_at, created_at());
}

// --- Typed-timestamp parsing: the `DateTime<Utc>` field validates on the wire. ---

#[test]
fn note_rejects_a_malformed_created_at() {
    // The typed `created_at` rejects a non-RFC-3339 string at deserialize time.
    let wire = json!({
        "id": NOTE_ID,
        "title": "t",
        "content": "c",
        "created_at": "not-a-date",
    });
    assert!(serde_json::from_value::<Note>(wire).is_err());
}

#[test]
fn note_normalizes_an_offset_bearing_created_at_to_utc() {
    // An RFC 3339 input carrying a non-Z offset is accepted and normalized to UTC, so it
    // re-serializes with the canonical `Z` suffix. `11:00:00+01:00` is `10:00:00Z`.
    let wire = json!({
        "id": NOTE_ID,
        "title": "t",
        "content": "c",
        "created_at": "2026-06-11T11:00:00+01:00",
    });
    let note: Note = serde_json::from_value(wire).unwrap();
    assert_eq!(
        note.created_at,
        "2026-06-11T10:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    let reserialized = serde_json::to_value(&note).unwrap();
    assert_eq!(
        reserialized.get("created_at").unwrap(),
        "2026-06-11T10:00:00Z"
    );
}

// --- CreateNoteRequest: minimal create body. ---

#[test]
fn create_note_request_serializes_the_two_string_fields() {
    let req = CreateNoteRequest {
        title: "Groceries".to_owned(),
        content: "milk, eggs, bread".to_owned(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "title": "Groceries",
            "content": "milk, eggs, bread",
        })
    );
}

#[test]
fn create_note_request_round_trips() {
    let wire = r#"{"title":"t","content":""}"#;
    let req: CreateNoteRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.title, "t");
    assert_eq!(req.content, "");
    let reserialized: Value = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    let original: Value = serde_json::from_str(wire).unwrap();
    assert_eq!(reserialized, original);
}

// --- UpdateNoteRequest: full-replace edit body. ---

#[test]
fn update_note_request_serializes_the_two_string_fields() {
    let req = UpdateNoteRequest {
        title: "Groceries (updated)".to_owned(),
        content: "milk, eggs, bread, butter".to_owned(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(
        json,
        json!({
            "title": "Groceries (updated)",
            "content": "milk, eggs, bread, butter",
        })
    );
}

#[test]
fn update_note_request_round_trips() {
    let wire = r#"{"title":"t","content":""}"#;
    let req: UpdateNoteRequest = serde_json::from_str(wire).unwrap();
    assert_eq!(req.title, "t");
    assert_eq!(req.content, "");
    let reserialized: Value = serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
    let original: Value = serde_json::from_str(wire).unwrap();
    assert_eq!(reserialized, original);
}
