//! Sync wire protocol — the `docs/CONTRACTS.md` §5 shapes, implemented verbatim.
//!
//! DORMANT until Phase 2. This module defines **only** the serde-serializable wire
//! types (`PushItem`, `ServerAck`, `SyncEntity`) so that when the optional
//! sync/collaboration server (`server/`, ADR-0003 Rust/Axum) ships, client and
//! server exchange byte-identical JSON that is pinned by the round-trip tests in
//! this file. Nothing here performs I/O or depends on the network — it is pure
//! data. See [`crate::sync::client`] for the (also dormant) transport skeleton.
//!
//! The two documented shapes (`docs/CONTRACTS.md` §5):
//!
//! ```jsonc
//! // client push item
//! { "tenant_id": "...", "entity": "meeting|transcript|summary|action_item",
//!   "id": "uuid", "rev": 42, "updated_by": "user_id", "updated_at": "ISO-8601",
//!   "deleted": false, "payload": { /* entity fields */ } }
//! // server ack
//! { "id": "uuid", "server_rev": 43, "conflict": false }
//! ```
//!
//! ## Field-order note
//!
//! serde_json emits object keys in struct-declaration order, so the field order in
//! [`PushItem`] / [`ServerAck`] below is kept identical to §5. JSON object key
//! order is not semantically significant, but keeping it identical makes the
//! documented sample and the produced bytes match line-for-line, which the tests
//! assert.
//!
//! ## Classification (why `settings` / `provider_credential` are absent)
//!
//! [`SyncEntity`] deliberately enumerates **only** the synced domain entities
//! (`docs/DATA_MODEL.md` "Entities": meeting/transcript/summary/action_item are
//! `Synced? = yes`). Per `docs/MULTITENANCY.md` "Data classification & sync scope"
//! and `docs/CONTRACTS.md` §5, the `settings` / `transcript_settings` tables and
//! `provider_credential` secrets are **local-only** and MUST NOT appear as a
//! `SyncEntity` variant — there is intentionally no `Settings` / `Credential`
//! arm, so a `PushItem` for a secret cannot even be constructed. Only a
//! non-secret label may ever sync, and that is a future, separate decision.

use serde::{Deserialize, Serialize};

/// The synced domain entity a [`PushItem`]/[`ServerAck`] refers to.
///
/// The serialized token matches `docs/CONTRACTS.md` §5 verbatim:
/// `meeting | transcript | summary | action_item`. Only these four logical
/// entities are `Synced? = yes` in `docs/DATA_MODEL.md`; there is deliberately no
/// variant for `settings` or `provider_credential` (local-only — see the module
/// docs and `docs/MULTITENANCY.md`).
///
/// `snake_case` renaming makes the multi-word `ActionItem` serialize to
/// `action_item` while the single-word variants serialize lowercase, matching §5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncEntity {
    /// The `meetings` table → logical `meeting`.
    Meeting,
    /// The transcript tables (`transcripts` chunks / `transcript_chunks` full
    /// text) → logical `transcript`.
    Transcript,
    /// The `summary_processes` table → logical `summary`.
    Summary,
    /// Extracted actions (today embedded in the summary JSON; a first-class table
    /// is a future migration) → logical `action_item`.
    ActionItem,
}

impl SyncEntity {
    /// The stable wire token (`meeting` / `transcript` / `summary` /
    /// `action_item`), identical to the serialized form. Handy for logging /
    /// building routes without a serde round-trip.
    pub fn wire_name(self) -> &'static str {
        match self {
            SyncEntity::Meeting => "meeting",
            SyncEntity::Transcript => "transcript",
            SyncEntity::Summary => "summary",
            SyncEntity::ActionItem => "action_item",
        }
    }
}

/// A single change the client pushes to the server (`docs/CONTRACTS.md` §5
/// "client push item").
///
/// Field declaration order matches §5 so the serialized JSON key order matches the
/// documented sample. `rev` is a monotonic `u64` (`docs/DATA_MODEL.md`: `rev`
/// monotonic; SQLite stores it as `INTEGER`, always non-negative in practice).
/// `payload` carries the entity's own fields as an opaque [`serde_json::Value`]
/// so this protocol type stays decoupled from every concrete entity struct —
/// the repository/sync mapping layer (Phase 2) builds and interprets it.
///
/// ## `updated_by` and the never-synced baseline
///
/// `docs/DATA_MODEL.md` (migration `20260702000000` note) defines a row with
/// `rev = 1` and `updated_by IS NULL` as "never synced / never edited remotely".
/// `updated_by` is therefore `Option<String>` here (a `None` serializes to JSON
/// `null`), so that baseline is representable on the wire without inventing a
/// sentinel user id. **Applying** an inbound remote change must preserve these
/// fields verbatim — see the [`crate::sync::client::RemoteApply`] seam and its
/// warning about why it must NOT go through the Phase-1 repositories.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushItem {
    /// Tenant/workspace scope (local mode: the constant `local`). Present on every
    /// item so the server can enforce tenant isolation independently.
    pub tenant_id: String,
    /// Which entity this change is for.
    pub entity: SyncEntity,
    /// The entity's uuid primary key.
    pub id: String,
    /// Monotonic revision of the row being pushed.
    pub rev: u64,
    /// User id of the last writer, or `None`/`null` for the never-synced baseline.
    pub updated_by: Option<String>,
    /// Last-modified timestamp, ISO-8601 / RFC 3339 text (kept as `String` to
    /// forward whatever the row holds without reformatting).
    pub updated_at: String,
    /// Soft-delete tombstone flag; deletes propagate (`docs/DATA_MODEL.md`
    /// "Deletes are soft ... and propagate").
    pub deleted: bool,
    /// The entity's own fields, opaque to this protocol layer.
    pub payload: serde_json::Value,
}

/// The server's per-item response (`docs/CONTRACTS.md` §5 "server ack").
///
/// Field declaration order matches §5. `conflict = true` means the server applied
/// last-write-wins and wrote an audit entry (§5); the client then adopts
/// `server_rev` as the new baseline for that row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerAck {
    /// The uuid of the item this acknowledges (echoes [`PushItem::id`]).
    pub id: String,
    /// The authoritative revision the server assigned after merge.
    pub server_rev: u64,
    /// `true` iff a conflict was detected and resolved via last-write-wins + audit.
    pub conflict: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `SyncEntity` serializes to the exact §5 tokens
    /// `meeting | transcript | summary | action_item` (note `action_item`, not
    /// `actionItem` / `action-item`).
    #[test]
    fn sync_entity_serializes_to_contracts_tokens() {
        let cases = [
            (SyncEntity::Meeting, "\"meeting\""),
            (SyncEntity::Transcript, "\"transcript\""),
            (SyncEntity::Summary, "\"summary\""),
            (SyncEntity::ActionItem, "\"action_item\""),
        ];
        for (variant, expected) in cases {
            let got = serde_json::to_string(&variant).expect("SyncEntity must serialize");
            assert_eq!(got, expected, "wire token for {variant:?}");
            // wire_name() and the serde token must not drift apart.
            assert_eq!(got, format!("\"{}\"", variant.wire_name()));
            // And it must round-trip back.
            let back: SyncEntity =
                serde_json::from_str(expected).expect("SyncEntity must deserialize");
            assert_eq!(back, variant);
        }
    }

    /// A `PushItem` serializes to EXACTLY the §5 "client push item" shape: the
    /// same eight keys, in the documented order, with the documented value types
    /// (compared against a hand-written JSON literal transcribed from §5).
    #[test]
    fn push_item_matches_contracts_v5_shape() {
        let item = PushItem {
            tenant_id: "local".to_string(),
            entity: SyncEntity::Meeting,
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            rev: 42,
            updated_by: Some("local-user".to_string()),
            updated_at: "2026-07-02T10:00:00.123+00:00".to_string(),
            deleted: false,
            payload: json!({ "title": "Kickoff", "started_at": "2026-07-02T09:00:00Z" }),
        };

        // Compare as structured JSON so the assertion is on keys + values + types,
        // exactly matching the documented §5 sample.
        let got = serde_json::to_value(&item).expect("PushItem must serialize");
        let expected = json!({
            "tenant_id": "local",
            "entity": "meeting",
            "id": "11111111-1111-4111-8111-111111111111",
            "rev": 42,
            "updated_by": "local-user",
            "updated_at": "2026-07-02T10:00:00.123+00:00",
            "deleted": false,
            "payload": { "title": "Kickoff", "started_at": "2026-07-02T09:00:00Z" }
        });
        assert_eq!(
            got, expected,
            "PushItem JSON must match CONTRACTS §5 verbatim"
        );

        // Assert the exact key SET is the §5 keys and nothing else (no extra
        // fields leaked in).
        let obj = got
            .as_object()
            .expect("PushItem serializes to a JSON object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "deleted",
                "entity",
                "id",
                "payload",
                "rev",
                "tenant_id",
                "updated_at",
                "updated_by",
            ],
            "PushItem must carry exactly the CONTRACTS §5 keys"
        );

        // Pin the on-the-wire key ORDER to the §5 declaration order too (serde_json
        // preserves struct field order): tenant_id, entity, id, rev, updated_by,
        // updated_at, deleted, payload.
        let ordered = serde_json::to_string(&item).expect("PushItem must serialize");
        let positions: Vec<usize> = [
            "\"tenant_id\"",
            "\"entity\"",
            "\"id\"",
            "\"rev\"",
            "\"updated_by\"",
            "\"updated_at\"",
            "\"deleted\"",
            "\"payload\"",
        ]
        .iter()
        .map(|k| ordered.find(k).unwrap_or_else(|| panic!("missing key {k}")))
        .collect();
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "PushItem keys must serialize in CONTRACTS §5 order, got: {ordered}"
        );
    }

    /// The never-synced baseline (`rev = 1`, `updated_by = None`) must serialize
    /// with `updated_by: null` — the wire representation of that state
    /// (`docs/DATA_MODEL.md`).
    #[test]
    fn push_item_null_updated_by_baseline() {
        let item = PushItem {
            tenant_id: "local".to_string(),
            entity: SyncEntity::ActionItem,
            id: "22222222-2222-4222-8222-222222222222".to_string(),
            rev: 1,
            updated_by: None,
            updated_at: "2026-07-02T10:00:00.000+00:00".to_string(),
            deleted: false,
            payload: json!({ "text": "Follow up", "source_chunk_id": "chunk-1" }),
        };
        let got = serde_json::to_value(&item).expect("PushItem must serialize");
        assert_eq!(got["updated_by"], serde_json::Value::Null);
        assert_eq!(got["entity"], "action_item");
        assert_eq!(got["rev"], 1);

        // Round-trips back to an equal value (None stays None).
        let back: PushItem =
            serde_json::from_value(got).expect("PushItem must deserialize from its own output");
        assert_eq!(back, item);
    }

    /// A `ServerAck` serializes to EXACTLY the §5 "server ack" shape: three keys
    /// `id | server_rev | conflict`, in order, with the documented types.
    #[test]
    fn server_ack_matches_contracts_v5_shape() {
        let ack = ServerAck {
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            server_rev: 43,
            conflict: false,
        };
        let got = serde_json::to_value(&ack).expect("ServerAck must serialize");
        let expected = json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "server_rev": 43,
            "conflict": false
        });
        assert_eq!(
            got, expected,
            "ServerAck JSON must match CONTRACTS §5 verbatim"
        );

        let obj = got
            .as_object()
            .expect("ServerAck serializes to a JSON object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["conflict", "id", "server_rev"],
            "ServerAck must carry exactly the CONTRACTS §5 keys"
        );

        // Key order: id, server_rev, conflict.
        let ordered = serde_json::to_string(&ack).expect("ServerAck must serialize");
        let id_pos = ordered.find("\"id\"").expect("id key");
        let rev_pos = ordered.find("\"server_rev\"").expect("server_rev key");
        let conflict_pos = ordered.find("\"conflict\"").expect("conflict key");
        assert!(
            id_pos < rev_pos && rev_pos < conflict_pos,
            "ServerAck keys must serialize in CONTRACTS §5 order, got: {ordered}"
        );
    }

    /// The server signalling a conflict-resolved (LWW + audit) ack round-trips.
    #[test]
    fn server_ack_conflict_true_round_trips() {
        let ack = ServerAck {
            id: "33333333-3333-4333-8333-333333333333".to_string(),
            server_rev: 100,
            conflict: true,
        };
        let text = serde_json::to_string(&ack).expect("ServerAck must serialize");
        let back: ServerAck = serde_json::from_str(&text).expect("ServerAck must deserialize");
        assert_eq!(back, ack);
        assert!(back.conflict);
    }

    /// Parsing the literal §5 documentation samples must yield the expected typed
    /// values — proves the doc examples are valid inputs to our types (the wire is
    /// bidirectional).
    #[test]
    fn parses_contracts_v5_documented_samples() {
        // Verbatim from docs/CONTRACTS.md §5 (deleted:false form).
        let push_json = r#"{ "tenant_id": "t1", "entity": "transcript",
            "id": "abc", "rev": 42, "updated_by": "user_id", "updated_at": "2026-01-01T00:00:00Z",
            "deleted": false, "payload": { "language": "en" } }"#;
        let push: PushItem = serde_json::from_str(push_json).expect("§5 push sample must parse");
        assert_eq!(push.entity, SyncEntity::Transcript);
        assert_eq!(push.rev, 42);
        assert_eq!(push.updated_by.as_deref(), Some("user_id"));
        assert!(!push.deleted);

        let ack_json = r#"{ "id": "abc", "server_rev": 43, "conflict": false }"#;
        let ack: ServerAck = serde_json::from_str(ack_json).expect("§5 ack sample must parse");
        assert_eq!(ack.server_rev, 43);
        assert!(!ack.conflict);
    }
}
