//! Summary draft data types — the `docs/CONTRACTS.md` §4 shapes, implemented
//! verbatim (BACKLOG C1). PURE data: no DB, no I/O, no provider coupling.
//!
//! Like [`crate::sync::protocol`] and [`crate::agents::draft`], this file defines
//! **only** serde-serializable wire types, pinned by the round-trip tests below.
//! The summary pipeline produces these, the UI renders them for review, and the
//! approval flow mutates their status — every AI-generated block and action item
//! starts life as a draft and carries a mandatory `source_chunk_id` back to the
//! transcript evidence it is grounded in (HITL, `CLAUDE.md` §0.5).
//!
//! The documented shapes (`docs/CONTRACTS.md` §4):
//!
//! ```text
//! MeetingNotesDraft { meeting_id, status: Draft, sections: [Section] }
//! Section           { title, blocks: [Block] }
//! Block             { id, type: text|bullet|heading1|heading2, content, source_chunk_id }
//! ActionItemDraft   { id, text, assignee?, due?, status, source_chunk_id }
//! ```
//!
//! `source_chunk_id` is REQUIRED on both `Block` and `ActionItemDraft` — the
//! fields are deliberately not `Option` and have no serde default, so an
//! evidence-free item cannot even be deserialized. No block/action item may be
//! persisted as approved without a human Approve action and a resolvable
//! `source_chunk_id` (§4).

use serde::{Deserialize, Serialize};

/// The visual kind of a summary block.
///
/// The serialized token matches `docs/CONTRACTS.md` §4 verbatim:
/// `text | bullet | heading1 | heading2` (serde `snake_case` keeps the trailing
/// digit attached: `Heading1` → `heading1`). No `Default`: unlike the status
/// enums there is no contract-blessed fallback kind, and the `type` field is
/// required on the wire anyway.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    /// A plain paragraph.
    Text,
    /// A bulleted list item.
    Bullet,
    /// A top-level heading.
    Heading1,
    /// A second-level heading.
    Heading2,
}

/// Lifecycle of a generated summary block or extracted action item. Defaults to
/// [`BlockStatus::Draft`] — nothing is ever auto-approved (HITL). Approval is an
/// explicit HUMAN action, never set by generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlockStatus {
    /// Just generated; awaiting human review. The only status generation may
    /// ever produce.
    #[default]
    Draft,
    /// A human approved this item. Set only by an explicit user action, never by
    /// generation or any automated path.
    Approved,
    /// A human edited the generated content; the pre-edit text is preserved in
    /// [`DraftBlock::original_content`].
    Edited,
    /// A human rejected this item.
    Rejected,
}

/// Lifecycle of a whole summary ([`MeetingNotesDraft`]). Defaults to
/// [`SummaryStatus::Draft`]; like [`BlockStatus`], the approved state is set only
/// by an explicit human action, never by generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SummaryStatus {
    /// Generated; awaiting human review.
    #[default]
    Draft,
    /// A human approved the summary.
    Approved,
}

/// One summary block, always anchored to transcript evidence
/// (`docs/CONTRACTS.md` §4 `Block`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftBlock {
    /// Block id (uuid).
    pub id: String,
    /// Visual kind; serialized under the §4 wire name `type`.
    #[serde(rename = "type")]
    pub block_type: BlockType,
    /// The block text as currently displayed (generated, or human-edited).
    pub content: String,
    /// REQUIRED and IMMUTABLE after creation: the transcript chunk this block is
    /// grounded in. References a `transcripts`-table row id — the same identifier
    /// space as [`crate::agents::draft::SourceRef::source_chunk_id`]. Not an
    /// `Option` and no serde default, so a block without evidence fails to
    /// deserialize.
    pub source_chunk_id: String,
    /// Review status; omitted on the wire deserializes to [`BlockStatus::Draft`].
    #[serde(default)]
    pub status: BlockStatus,
    /// Set on the FIRST human edit and preserves the originally generated text,
    /// so a [`BlockStatus::Edited`] block stays auditable against what the model
    /// actually produced. Absent from the JSON while `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
}

/// A titled group of blocks (`docs/CONTRACTS.md` §4 `Section`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftSection {
    /// Section heading shown in the editor.
    pub title: String,
    /// The section's blocks, in display order.
    pub blocks: Vec<DraftBlock>,
}

/// A whole generated summary for one meeting (`docs/CONTRACTS.md` §4
/// `MeetingNotesDraft`) — a DRAFT until a human approves it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeetingNotesDraft {
    /// The meeting these notes belong to.
    pub meeting_id: String,
    /// Summary-level review status; omitted on the wire deserializes to
    /// [`SummaryStatus::Draft`].
    #[serde(default)]
    pub status: SummaryStatus,
    /// The summary body, in display order.
    pub sections: Vec<DraftSection>,
}

/// An extracted action item (`docs/CONTRACTS.md` §4 `ActionItemDraft`), likewise
/// evidence-anchored and human-reviewed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionItemDraft {
    /// Action item id (uuid).
    pub id: String,
    /// The action text.
    pub text: String,
    /// Suggested owner, if inferred (§4 `assignee?`). Absent from the JSON while
    /// `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// Suggested due date (free text / ISO-8601), if inferred (§4 `due?`).
    /// Absent from the JSON while `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// Review status; omitted on the wire deserializes to [`BlockStatus::Draft`].
    #[serde(default)]
    pub status: BlockStatus,
    /// REQUIRED and IMMUTABLE after creation — the same evidence anchor as
    /// [`DraftBlock::source_chunk_id`].
    pub source_chunk_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full `MeetingNotesDraft` round-trips, and the serialized JSON pins the
    /// EXACT `docs/CONTRACTS.md` §4 wire tokens: the `type` key (not
    /// `block_type`) with all four block-kind tokens, `"status":"draft"`, and
    /// the mandatory `source_chunk_id` key.
    #[test]
    fn meeting_notes_draft_round_trips_with_contracts_tokens() {
        let block = |id: &str, block_type: BlockType, content: &str, chunk: &str| DraftBlock {
            id: id.to_string(),
            block_type,
            content: content.to_string(),
            source_chunk_id: chunk.to_string(),
            status: BlockStatus::Draft,
            original_content: None,
        };
        let draft = MeetingNotesDraft {
            meeting_id: "m1".to_string(),
            status: SummaryStatus::Draft,
            sections: vec![DraftSection {
                title: "Decisions".to_string(),
                blocks: vec![
                    block("b1", BlockType::Heading1, "Key decisions", "chunk-1"),
                    block("b2", BlockType::Heading2, "Budget", "chunk-2"),
                    block(
                        "b3",
                        BlockType::Text,
                        "We agreed on the Q3 budget.",
                        "chunk-2",
                    ),
                    block(
                        "b4",
                        BlockType::Bullet,
                        "Ship the deck by Friday",
                        "chunk-3",
                    ),
                ],
            }],
        };

        let text = serde_json::to_string(&draft).expect("MeetingNotesDraft must serialize");
        // Exact §4 wire tokens (serde_json compact form).
        assert!(text.contains(r#""type":"heading1""#), "got: {text}");
        assert!(text.contains(r#""type":"heading2""#), "got: {text}");
        assert!(text.contains(r#""type":"text""#), "got: {text}");
        assert!(text.contains(r#""type":"bullet""#), "got: {text}");
        assert!(text.contains(r#""status":"draft""#), "got: {text}");
        assert!(text.contains(r#""source_chunk_id""#), "got: {text}");
        // The Rust field name must NOT leak onto the wire.
        assert!(!text.contains("block_type"), "got: {text}");

        let back: MeetingNotesDraft =
            serde_json::from_str(&text).expect("MeetingNotesDraft must deserialize");
        assert_eq!(back, draft);
    }

    /// `source_chunk_id` is REQUIRED: a `DraftBlock` or `ActionItemDraft`
    /// without it must FAIL to deserialize (the field is not `Option` and has no
    /// default) — an evidence-free item is unrepresentable.
    #[test]
    fn missing_source_chunk_id_fails_deserialization() {
        let block = serde_json::from_str::<DraftBlock>(
            r#"{ "id": "b1", "type": "text", "content": "hi", "status": "draft" }"#,
        );
        let err = block
            .expect_err("DraftBlock without source_chunk_id must not parse")
            .to_string();
        assert!(
            err.contains("source_chunk_id"),
            "error must name the field: {err}"
        );

        let action = serde_json::from_str::<ActionItemDraft>(
            r#"{ "id": "a1", "text": "Follow up", "status": "draft" }"#,
        );
        let err = action
            .expect_err("ActionItemDraft without source_chunk_id must not parse")
            .to_string();
        assert!(
            err.contains("source_chunk_id"),
            "error must name the field: {err}"
        );
    }

    /// An omitted `status` deserializes to `Draft` at the block, action-item,
    /// and summary level — the wire default matches the derived `Default` (the
    /// load-bearing HITL baseline).
    #[test]
    fn omitted_status_defaults_to_draft() {
        assert_eq!(BlockStatus::default(), BlockStatus::Draft);
        assert_eq!(SummaryStatus::default(), SummaryStatus::Draft);

        let block: DraftBlock = serde_json::from_str(
            r#"{ "id": "b1", "type": "bullet", "content": "hi", "source_chunk_id": "c1" }"#,
        )
        .expect("DraftBlock without status must parse");
        assert_eq!(block.status, BlockStatus::Draft);

        let action: ActionItemDraft =
            serde_json::from_str(r#"{ "id": "a1", "text": "t", "source_chunk_id": "c1" }"#)
                .expect("ActionItemDraft without status must parse");
        assert_eq!(action.status, BlockStatus::Draft);

        let notes: MeetingNotesDraft =
            serde_json::from_str(r#"{ "meeting_id": "m1", "sections": [] }"#)
                .expect("MeetingNotesDraft without status must parse");
        assert_eq!(notes.status, SummaryStatus::Draft);
    }

    /// `BlockStatus` serializes to the exact wire tokens
    /// `draft | approved | edited | rejected` and round-trips.
    #[test]
    fn block_status_wire_tokens() {
        let cases = [
            (BlockStatus::Draft, "\"draft\""),
            (BlockStatus::Approved, "\"approved\""),
            (BlockStatus::Edited, "\"edited\""),
            (BlockStatus::Rejected, "\"rejected\""),
        ];
        for (variant, expected) in cases {
            let got = serde_json::to_string(&variant).expect("BlockStatus must serialize");
            assert_eq!(got, expected, "wire token for {variant:?}");
            let back: BlockStatus =
                serde_json::from_str(expected).expect("BlockStatus must deserialize");
            assert_eq!(back, variant);
        }
    }

    /// `original_content` is absent from the JSON while `None` and present after
    /// a human edit sets it to `Some` (and a payload without the key parses back
    /// to `None`).
    #[test]
    fn original_content_absent_when_none_present_when_some() {
        let mut block = DraftBlock {
            id: "b1".to_string(),
            block_type: BlockType::Text,
            content: "the generated text".to_string(),
            source_chunk_id: "c1".to_string(),
            status: BlockStatus::Draft,
            original_content: None,
        };
        let text = serde_json::to_string(&block).expect("serialize");
        assert!(
            !text.contains("original_content"),
            "None must be skipped: {text}"
        );

        // First human edit: content replaced, original preserved, status Edited.
        block.original_content = Some(block.content.clone());
        block.content = "the human-edited text".to_string();
        block.status = BlockStatus::Edited;
        let text = serde_json::to_string(&block).expect("serialize");
        assert!(
            text.contains(r#""original_content":"the generated text""#),
            "Some must be present: {text}"
        );
        assert!(text.contains(r#""status":"edited""#), "got: {text}");
        let back: DraftBlock = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back, block);

        // A payload without the key still parses (back to None).
        let no_orig: DraftBlock = serde_json::from_str(
            r#"{ "id": "b1", "type": "text", "content": "hi", "source_chunk_id": "c1" }"#,
        )
        .expect("parse without original_content");
        assert_eq!(no_orig.original_content, None);
    }
}
