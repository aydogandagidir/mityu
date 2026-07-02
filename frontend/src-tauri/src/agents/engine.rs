//! Agent engine skeleton — DORMANT until EPIC F.
//!
//! The behavior half of the seam (the analogue of `sync/client.rs`). It is
//! **disabled by default and reachable from nothing**: no Tauri command, no caller
//! on the core path, no UI. While [`AgentsConfig::enabled`] is `false` (the
//! default), [`AgentRunner::run`] short-circuits with [`AgentError::Disabled`]
//! **before touching any agent** — asserted by the tests.
//!
//! ## No network dependency
//!
//! There is intentionally no LLM/HTTP client here. An [`Agent`] is an abstract
//! trait; the only Phase-1 implementor is [`NoopAgent`], which does no I/O. The
//! Phase-F agents reuse the existing summary providers (`summary/` + `anthropic/`
//! `openai/` `groq/` `ollama/` `openrouter/`, BYOK, offline-capable) behind this
//! trait — this file does not change when they land.
//!
//! ## HITL is enforced here, not just documented
//!
//! [`AgentRunner::run`] rewrites the returned draft's status to
//! [`super::draft::DraftStatus::Draft`] unconditionally, so even a misbehaving
//! agent cannot produce an auto-approved result (`CLAUDE.md` §0.5). There is no
//! "execute this action" method anywhere — proposals stay proposals
//! (`CLAUDE.md` §10).

use super::draft::{AgentDraft, DraftStatus, SourceRef};
use super::AgentKind;
use crate::context::AuthContext;
use thiserror::Error;

/// Dormant agents configuration. `enabled` defaults to `false` (`bool::default()`),
/// so a default-constructed [`AgentRunner`] is inert. Phase F grows this (per-agent
/// enablement, provider selection); today it carries only the master switch so the
/// "off" path is typed and testable. Not surfaced in any UI or config file yet.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct AgentsConfig {
    /// Master switch. `false` (the derived default) ⇒ [`AgentRunner::run`] returns
    /// [`AgentError::Disabled`] and does no work.
    pub enabled: bool,
}

/// Typed agent errors (`docs/CONVENTIONS.md`: typed domain errors via `thiserror`).
/// In this dormant phase only [`AgentError::Disabled`] / [`AgentError::Unknown`] can
/// occur; [`AgentError::Provider`] is the vocabulary the Phase-F provider layer will
/// use, defined now so the surface is stable.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Agents are turned off ([`AgentsConfig::enabled`] is `false`). Not a
    /// user-facing error — callers treat it as "feature not active".
    #[error("agents are disabled")]
    Disabled,
    /// No [`Agent`] is registered for the requested [`AgentKind`].
    #[error("no agent registered for {0:?}")]
    Unknown(AgentKind),
    /// Phase-F provider/LLM failure. Local-first: callers degrade gracefully
    /// (surface a message, keep working on local data).
    #[error("agent provider error: {0}")]
    Provider(String),
}

/// Read-only evidence an [`Agent`] reasons over. The agent never fetches anything
/// from the network via this seam — it is handed the meeting's already-local
/// summary + source anchors and returns a draft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentInput {
    /// The meeting this run is about.
    pub meeting_id: String,
    /// The approved summary text (if any) the agent works from.
    pub summary_text: Option<String>,
    /// Source anchors the agent should ground its output in.
    pub sources: Vec<SourceRef>,
}

/// A single on-device agent. Object-safe so [`AgentRunner`] can hold
/// `Box<dyn Agent>`. Implementors MUST be pure "propose a draft" functions: no
/// external side effects, offline-capable.
pub trait Agent: Send + Sync {
    /// Which [`AgentKind`] this implements.
    fn kind(&self) -> AgentKind;

    /// Produce a DRAFT grounded in `input`. `ctx` scopes the run to
    /// `ctx.tenant_id`. Never sends/creates anything in the outside world.
    fn draft(&self, ctx: &AuthContext, input: &AgentInput) -> Result<AgentDraft, AgentError>;
}

/// Placeholder agent (the analogue of `sync::NoopTransport`): returns an empty
/// draft and performs no I/O. Lets the dormant seam be exercised without any real
/// provider.
pub struct NoopAgent {
    kind: AgentKind,
}

impl NoopAgent {
    /// A no-op agent that answers for `kind`.
    pub fn new(kind: AgentKind) -> Self {
        Self { kind }
    }
}

impl Agent for NoopAgent {
    fn kind(&self) -> AgentKind {
        self.kind
    }

    fn draft(&self, _ctx: &AuthContext, _input: &AgentInput) -> Result<AgentDraft, AgentError> {
        Ok(AgentDraft::empty())
    }
}

/// The dormant agent orchestrator. Owns an [`AgentsConfig`] and a set of registered
/// [`Agent`]s; while disabled it is inert.
pub struct AgentRunner {
    config: AgentsConfig,
    agents: Vec<Box<dyn Agent>>,
}

impl AgentRunner {
    /// Construct a runner. With the default [`AgentsConfig`] (`enabled: false`) it
    /// does nothing until Phase F turns it on.
    pub fn new(config: AgentsConfig) -> Self {
        Self {
            config,
            agents: Vec::new(),
        }
    }

    /// Register an [`Agent`] (Phase F wires the real ones here).
    pub fn register(&mut self, agent: Box<dyn Agent>) {
        self.agents.push(agent);
    }

    /// Whether agents are currently enabled (mirrors [`AgentsConfig::enabled`]).
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Run the agent of `kind` over `input`.
    ///
    /// Phase 1: always returns [`AgentError::Disabled`] (agents are off) and does
    /// **no work** — no agent is touched. The returned draft is ALWAYS
    /// [`DraftStatus::Draft`]: approval is a separate human step (HITL,
    /// `CLAUDE.md` §0.5), enforced here defensively so no agent can self-approve.
    pub fn run(
        &self,
        ctx: &AuthContext,
        kind: AgentKind,
        input: &AgentInput,
    ) -> Result<AgentDraft, AgentError> {
        // Guard FIRST: never touch an agent while disabled.
        if !self.config.enabled {
            return Err(AgentError::Disabled);
        }
        let agent = self
            .agents
            .iter()
            .find(|a| a.kind() == kind)
            .ok_or(AgentError::Unknown(kind))?;
        let produced = agent.draft(ctx, input)?;
        // HITL: coerce back to Draft no matter what the agent returned.
        Ok(AgentDraft {
            status: DraftStatus::Draft,
            ..produced
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::draft::{DraftStatus, ProposedAction};
    use super::*;
    use crate::context::AuthContext;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    fn input() -> AgentInput {
        AgentInput {
            meeting_id: "m1".to_string(),
            summary_text: Some("we agreed to follow up".to_string()),
            sources: Vec::new(),
        }
    }

    /// The default config is OFF — the local-first invariant for this module.
    #[test]
    fn config_default_is_disabled() {
        assert!(!AgentsConfig::default().enabled);
        assert!(!AgentRunner::new(AgentsConfig::default()).is_enabled());
    }

    /// An agent that records whether it was ever invoked, so we can prove a
    /// disabled runner does NO work (never reaches an agent).
    struct SpyAgent {
        called: Arc<AtomicBool>,
    }
    impl Agent for SpyAgent {
        fn kind(&self) -> AgentKind {
            AgentKind::FollowUpDrafter
        }
        fn draft(&self, _ctx: &AuthContext, _input: &AgentInput) -> Result<AgentDraft, AgentError> {
            self.called.store(true, Ordering::SeqCst);
            Ok(AgentDraft::empty())
        }
    }

    /// Disabled `run` returns `Disabled` AND never invokes an agent (no work).
    #[test]
    fn disabled_run_returns_disabled_and_does_no_work() {
        let flag = Arc::new(AtomicBool::new(false));
        let mut runner = AgentRunner::new(AgentsConfig::default());
        runner.register(Box::new(SpyAgent {
            called: flag.clone(),
        }));
        let ctx = AuthContext::local();

        let err = runner
            .run(&ctx, AgentKind::FollowUpDrafter, &input())
            .unwrap_err();
        assert!(matches!(err, AgentError::Disabled), "got {err:?}");
        assert!(
            !flag.load(Ordering::SeqCst),
            "disabled runner must not invoke any agent (no work)"
        );
    }

    /// An agent that tries to self-approve; the runner MUST coerce it back to
    /// `Draft` (HITL) — the executable form of `CLAUDE.md` §0.5.
    struct SelfApprovingAgent;
    impl Agent for SelfApprovingAgent {
        fn kind(&self) -> AgentKind {
            AgentKind::ActionTracker
        }
        fn draft(&self, _ctx: &AuthContext, _input: &AgentInput) -> Result<AgentDraft, AgentError> {
            Ok(AgentDraft {
                status: DraftStatus::Approved, // <-- must NOT survive the runner
                actions: vec![ProposedAction::TrackActionItem {
                    text: "do the thing".to_string(),
                    assignee: None,
                    due: None,
                }],
                sources: Vec::new(),
            })
        }
    }

    #[test]
    fn run_forces_hitl_never_returns_approved() {
        let mut runner = AgentRunner::new(AgentsConfig { enabled: true });
        runner.register(Box::new(SelfApprovingAgent));
        let ctx = AuthContext::local();

        let draft = runner
            .run(&ctx, AgentKind::ActionTracker, &input())
            .expect("enabled run with a registered agent");
        assert_eq!(
            draft.status,
            DraftStatus::Draft,
            "runner must force HITL: an agent can never auto-approve"
        );
        assert_eq!(draft.actions.len(), 1, "the proposal itself is preserved");
    }

    /// Enabled but no matching agent ⇒ `Unknown(kind)`.
    #[test]
    fn enabled_unknown_kind_returns_unknown() {
        let runner = AgentRunner::new(AgentsConfig { enabled: true });
        let ctx = AuthContext::local();
        let err = runner
            .run(&ctx, AgentKind::FollowUpDrafter, &input())
            .unwrap_err();
        assert!(
            matches!(err, AgentError::Unknown(AgentKind::FollowUpDrafter)),
            "got {err:?}"
        );
    }

    /// The placeholder agent yields an empty Draft and does no I/O.
    #[test]
    fn noop_agent_yields_empty_draft() {
        let a = NoopAgent::new(AgentKind::FollowUpDrafter);
        let ctx = AuthContext::local();
        let d = a.draft(&ctx, &input()).expect("noop draft");
        assert_eq!(a.kind(), AgentKind::FollowUpDrafter);
        assert!(d.actions.is_empty());
        assert_eq!(d.status, DraftStatus::Draft);
    }

    /// The provider error variant carries its cause in Display.
    #[test]
    fn provider_error_displays_cause() {
        let e = AgentError::Provider("model unavailable".to_string());
        assert_eq!(e.to_string(), "agent provider error: model unavailable");
    }
}
