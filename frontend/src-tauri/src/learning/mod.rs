//! Local learning: turning the human-in-the-loop correction signal into
//! plain-language rules that shape the next summary (ADR-0030).
//!
//! The whole system in one line: **learning is data, not weights.** Corrections
//! are captured append-only (`database/repositories/correction_event.rs`), mined
//! into rules the user can read, edit and delete, and injected back into the
//! prompt as retrieved context. Nothing is ever trained; nothing leaves the
//! device to make it work.
//!
//! Two consequences that look like restrictions and are actually the point:
//! - **Erasable.** An unwanted rule is one `DELETE`. An unwanted fine-tune is a
//!   retrain — which is why KVKK/GDPR erasure ruled weights out (§6, §10).
//! - **Reproducible.** Rules render into the prompt in a stable order and each
//!   generation snapshots the rules that shaped it (§5), so a six-month-old
//!   summary can still be explained. That is the EU-AI-Act Art.50 requirement,
//!   and it is the precondition that makes auto-activation defensible at all —
//!   the output HITL gate bounds a bad rule, the snapshot explains it.
//!
//! Module map: [`rule`] is the domain (types, tokens, scope filter, lifecycle),
//! [`config`] the per-workspace policy. Persistence lives in
//! `database/repositories/learned_rule.rs`; prompt rendering lives in
//! `summary::structured`, beside the prompts it shapes.

pub mod burden;
pub mod commands;
pub mod config;
pub mod llm_miner;
pub mod miner;
pub mod rule;
