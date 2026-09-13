//! What a meeting *mode* is (BACKLOG I4a, ADR-0038).
//!
//! A mode is the answer to "what kind of conversation is this, and what may the
//! copilot do in it". It generalises `summary/templates`: a [`Template`] says
//! how to write the notes afterwards, a [`Mode`] adds who is in the room, what
//! the assistant is allowed to offer *during* the conversation, and what it is
//! allowed to read while doing it.
//!
//! Nothing here calls a model, reads a file or touches the network. A mode is
//! data plus a few total functions over it, so every rule below is a unit test
//! rather than a comment.
//!
//! ## The reject list is part of the type, not a policy document
//!
//! ADR-0038 rules out a whole category: candidate-side interview coaching, exam
//! answering and coding-problem solving. Mityu ships no such mode, and
//! [`super::validator`] refuses to install a custom one that announces itself as
//! one. That check is **a signpost, not a security control** — someone
//! determined can rename a file — and it is written down as such where it lives.
//! The point is that the product does not hand you the tool and does not
//! pretend the category is unconsidered.

use serde::{Deserialize, Serialize};

/// What the copilot may be asked for during the conversation.
///
/// These are exactly I3's four on-demand actions. A mode narrows the set — a
/// lecture has little use for "suggest what to say next" — and the panel shows
/// only what the active mode allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LiveAction {
    /// "What could I say next?" — the category's signature action.
    Suggest,
    /// One to three questions worth asking, each tied to what was just said.
    FollowUpQuestions,
    /// Where the conversation has got to so far.
    Recap,
    /// Explain a term that was just used.
    Define,
}

impl LiveAction {
    pub const ALL: [LiveAction; 4] = [
        LiveAction::Suggest,
        LiveAction::FollowUpQuestions,
        LiveAction::Recap,
        LiveAction::Define,
    ];

    /// Stable wire token, identical to the serialized form.
    pub fn wire_name(self) -> &'static str {
        match self {
            LiveAction::Suggest => "suggest",
            LiveAction::FollowUpQuestions => "followUpQuestions",
            LiveAction::Recap => "recap",
            LiveAction::Define => "define",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            LiveAction::Suggest => "Suggest",
            LiveAction::FollowUpQuestions => "Follow-up questions",
            LiveAction::Recap => "Recap",
            LiveAction::Define => "Define",
        }
    }
}

/// How strictly an answer must rest on retrieved evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidencePolicy {
    /// Every claim cites a transcript segment or a knowledge chunk, and an
    /// uncited one is dropped rather than repaired (`ask::grounding`). The
    /// default, and the only setting that suits a mode whose output may end up
    /// in a client note.
    SourceFirst,
    /// The model may also draw on what it knows, for a mode where that is the
    /// point — explaining a term in a lecture, say. Claims that *do* cite are
    /// still grounded; the difference is that an uncited claim is allowed to
    /// survive, marked as uncited.
    Open,
}

/// Where an answer's evidence may come from.
///
/// Declared in full so a mode written today still means the same thing when the
/// later sources exist; [`AllowedSource::is_available_today`] is the single
/// place that says which of them can actually be retrieved, the same shape as
/// `copilot::config::KeybindAction::is_registerable` (ADR-0033).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AllowedSource {
    /// The live transcript of the running session — the only one that exists.
    Transcript,
    /// The local knowledge base. Arrives with I5.
    Knowledge,
    /// Text read from one user-initiated screenshot. Arrives with I6.
    Screen,
}

impl AllowedSource {
    pub const ALL: [AllowedSource; 3] = [
        AllowedSource::Transcript,
        AllowedSource::Knowledge,
        AllowedSource::Screen,
    ];

    /// Can anything actually retrieve from this source today?
    ///
    /// A mode may list a source that does not exist yet — that is the point of
    /// writing the modes once — but nothing may quietly behave as though it
    /// returned results. I3 asks this before building a prompt.
    pub fn is_available_today(self) -> bool {
        match self {
            AllowedSource::Transcript => true,
            // I5 — no knowledge base exists yet.
            AllowedSource::Knowledge => false,
            // I6 — no screen capture exists yet.
            AllowedSource::Screen => false,
        }
    }

    /// Why it cannot be retrieved from, when it cannot. `None` when it can.
    pub fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            AllowedSource::Transcript => None,
            AllowedSource::Knowledge => {
                Some("The local knowledge base arrives with a later version.")
            }
            AllowedSource::Screen => Some("Screen context arrives with a later version."),
        }
    }

    pub fn wire_name(self) -> &'static str {
        match self {
            AllowedSource::Transcript => "transcript",
            AllowedSource::Knowledge => "knowledge",
            AllowedSource::Screen => "screen",
        }
    }
}

/// What the copilot may do while the conversation is happening.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LivePolicy {
    /// The actions this mode offers. Never empty — a mode that offers nothing
    /// live is a summary template, and one of those already exists.
    pub allowed_actions: Vec<LiveAction>,
    pub evidence_policy: EvidencePolicy,
    /// Must every claim carry a citation? Independent of `evidence_policy`
    /// because a mode can be `Open` and still require that whatever *is* drawn
    /// from the conversation is cited.
    pub cite_required: bool,
}

/// A meeting mode.
///
/// `serde(default)` is deliberately **not** used: a mode is authored, not
/// migrated, and a field silently defaulting would give a custom mode
/// permissions its author never wrote. A missing field is a validation error
/// the user sees before the mode is installed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mode {
    /// Stable identifier, `[a-z0-9_]`. Used in settings and in the panel chip.
    pub id: String,
    pub name: String,
    /// One sentence: what this kind of conversation is for. Goes into the
    /// prompt, so it is written as guidance to an assistant, not as marketing.
    pub purpose: String,
    /// Who the user is in this conversation ("the consultant", "the
    /// interviewer"). Both roles exist so the assistant knows which side it is
    /// helping — and so the *other* side is named rather than assumed.
    pub user_role: String,
    pub counterpart_role: String,
    /// How an answer should read: terse, formal, plain-language.
    pub voice: String,
    /// The summary template this mode uses after the meeting. Must resolve
    /// through `summary::templates::get_template`.
    pub summary_template_id: String,
    pub live: LivePolicy,
    /// Never empty; `Transcript` is always present in a built-in.
    pub allowed_sources: Vec<AllowedSource>,
}

impl Mode {
    pub fn allows(&self, action: LiveAction) -> bool {
        self.live.allowed_actions.contains(&action)
    }

    /// The sources this mode allows **and** that can be retrieved from today.
    ///
    /// I3 builds its prompt from this, not from `allowed_sources`, so a mode
    /// that lists the knowledge base simply reads the transcript until I5 lands
    /// — rather than producing an answer that silently rests on nothing.
    pub fn usable_sources(&self) -> Vec<AllowedSource> {
        self.allowed_sources
            .iter()
            .copied()
            .filter(|s| s.is_available_today())
            .collect()
    }
}
