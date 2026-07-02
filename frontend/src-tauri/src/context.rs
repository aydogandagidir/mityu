//! Identity seam: the single source of "who is acting, in which tenant/workspace".
//!
//! Implements `AuthContext` per `docs/CONTRACTS.md` §1 and `docs/MULTITENANCY.md`
//! rule 2: **all identity comes from `AuthContext`** — no other code path (a global,
//! a hardcoded value, an implicit singleton, or a frontend-supplied argument) may
//! determine the current user or tenant.
//!
//! - **Phase 1 (local-first, today):** [`current`] is a constant resolver returning
//!   the single implicit local workspace/user ([`LOCAL_WORKSPACE_ID`] /
//!   [`LOCAL_USER_ID`]) holding the [`Role::Owner`] role. No login, no network.
//! - **Phase 2 (optional server):** the body of [`current`] is replaced by a resolver
//!   that derives the context from a validated OIDC token. The seam — and therefore
//!   every caller — stays unchanged.
//!
//! Repositories (EPIC B2) take `&AuthContext` on every method and scope every query
//! by `ctx.tenant_id`. Tauri commands obtain the context via [`current`]; the UI
//! never supplies identity (deliberately, [`AuthContext`] does **not** derive
//! `Deserialize`).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

/// Fixed workspace id for local-first mode (`docs/MULTITENANCY.md`: exactly one
/// implicit workspace on the client until the optional server exists).
pub const LOCAL_WORKSPACE_ID: &str = "local";

/// Fixed user id for the single implicit local user in local-first mode.
pub const LOCAL_USER_ID: &str = "local-user";

/// Tenant (server) / workspace (client) identifier. String-backed newtype so ids
/// cannot be swapped with user or request ids at compile time.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    /// Wrap an existing id (e.g. read from a persisted row or, in Phase 2, an
    /// OIDC claim). Feature code should not mint tenant ids — use [`current`].
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the raw id, e.g. to bind it to a `WHERE workspace_id = ?` query.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// User identifier. String-backed newtype; see [`TenantId`] for rationale.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(String);

impl UserId {
    /// Wrap an existing id. Feature code should not mint user ids — use [`current`].
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the raw id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Per-operation correlation id for audit trails and log correlation
/// (`docs/CONTRACTS.md` §6). String-backed (UUID v4 text form).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    /// Wrap an existing id (e.g. one received from the Phase-2 server).
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Mint a fresh random correlation id (UUID v4).
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Borrow the raw id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// RBAC role, per tenant (`docs/MULTITENANCY.md` "AuthN / AuthZ").
///
/// Privilege ordering (highest → lowest): **Owner > Admin > Member > Viewer**.
///
/// The declaration order below follows `docs/CONTRACTS.md` §1 verbatim, so `Ord`
/// is implemented manually on top of [`Role::privilege_level`] instead of being
/// derived (a derived `Ord` would use declaration order and invert the ranking).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Owner,
    Admin,
    Member,
    Viewer,
}

impl Role {
    /// Numeric privilege level; strictly monotonic in the ordering documented on
    /// [`Role`]: `Owner = 3`, `Admin = 2`, `Member = 1`, `Viewer = 0`.
    /// A role satisfies a requirement iff its level is `>=` the required level.
    pub fn privilege_level(self) -> u8 {
        match self {
            Role::Owner => 3,
            Role::Admin => 2,
            Role::Member => 1,
            Role::Viewer => 0,
        }
    }

    /// Stable lowercase name, matching the serialized form and the server-side
    /// role vocabulary (`owner` / `admin` / `member` / `viewer`).
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Admin => "admin",
            Role::Member => "member",
            Role::Viewer => "viewer",
        }
    }
}

impl Ord for Role {
    /// Orders by privilege: `Viewer < Member < Admin < Owner`.
    fn cmp(&self, other: &Self) -> Ordering {
        self.privilege_level().cmp(&other.privilege_level())
    }
}

impl PartialOrd for Role {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Typed authorization error (`docs/CONVENTIONS.md`: typed domain/authz errors via
/// `thiserror`). Serializable so future Tauri commands can return it directly as
/// their error type.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthzError {
    /// No role held by the context reaches the required privilege level.
    /// Also returned for a context holding no roles at all (default-deny).
    #[error("insufficient privileges: this action requires role '{required}' or higher")]
    InsufficientRole {
        /// The minimum role the action requires.
        required: Role,
    },
}

/// Who is acting, in which tenant/workspace, with which roles — the single
/// identity carrier for all domain operations (`docs/CONTRACTS.md` §1).
///
/// Deliberately **not** `Deserialize`: identity is *resolved* (via [`current`]),
/// never *supplied* by the frontend or the network.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuthContext {
    /// Local mode: always [`LOCAL_WORKSPACE_ID`]. Phase 2: the real tenant id.
    pub tenant_id: TenantId,
    /// Local mode: always [`LOCAL_USER_ID`]. Phase 2: the OIDC subject.
    pub user_id: UserId,
    /// Roles held in `tenant_id`. Local mode: `[Role::Owner]`.
    pub roles: Vec<Role>,
    /// Fresh per resolved context; used for audit/log correlation.
    pub request_id: RequestId,
}

/// Client-side vocabulary alias: the docs call this seam
/// "WorkspaceContext / AuthContext" (BACKLOG B1); they are the same type.
pub type WorkspaceContext = AuthContext;

impl AuthContext {
    /// The Phase-1 constant resolution: the single implicit local workspace and
    /// user, with owner-equivalent rights and a fresh [`RequestId`] per call.
    ///
    /// Prefer [`current`] in feature code — it is the seam Phase 2 swaps out.
    pub fn local() -> Self {
        Self {
            tenant_id: TenantId::new(LOCAL_WORKSPACE_ID),
            user_id: UserId::new(LOCAL_USER_ID),
            roles: vec![Role::Owner],
            request_id: RequestId::generate(),
        }
    }

    /// RBAC check: `Ok(())` iff some held role has a privilege level `>=` `min`
    /// (see the ordering documented on [`Role`]). A context holding no roles is
    /// denied everything (default-deny).
    pub fn require(&self, min: Role) -> Result<(), AuthzError> {
        let highest = self.roles.iter().map(|r| r.privilege_level()).max();
        match highest {
            Some(level) if level >= min.privilege_level() => Ok(()),
            _ => Err(AuthzError::InsufficientRole { required: min }),
        }
    }
}

/// Resolve the [`AuthContext`] for the current operation — **the single entry
/// point** repositories (B2) and commands use to learn "who + which tenant".
///
/// Phase 1 (today): a constant resolver — always the local workspace/user
/// ([`AuthContext::local`]). Phase 2: this body is replaced by an OIDC-derived
/// resolver; callers do not change. Per `docs/MULTITENANCY.md` rule 2, no other
/// code path may determine identity.
pub fn current() -> AuthContext {
    let ctx = AuthContext::local();
    tracing::trace!(
        tenant_id = %ctx.tenant_id,
        request_id = %ctx.request_id,
        "resolved local AuthContext"
    );
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_ROLES: [Role; 4] = [Role::Owner, Role::Admin, Role::Member, Role::Viewer];

    fn ctx_with_roles(roles: Vec<Role>) -> AuthContext {
        AuthContext {
            roles,
            ..AuthContext::local()
        }
    }

    /// Full truth table for `require()` with a single held role, written out
    /// explicitly (not derived from `privilege_level`) so the ordering semantics
    /// themselves are asserted: Owner > Admin > Member > Viewer.
    #[test]
    fn require_matrix_single_role() {
        use Role::{Admin, Member, Owner, Viewer};
        let cases: [(Role, Role, bool); 16] = [
            // Owner passes everything.
            (Owner, Owner, true),
            (Owner, Admin, true),
            (Owner, Member, true),
            (Owner, Viewer, true),
            // Admin passes all but Owner.
            (Admin, Owner, false),
            (Admin, Admin, true),
            (Admin, Member, true),
            (Admin, Viewer, true),
            // Member passes Member/Viewer only.
            (Member, Owner, false),
            (Member, Admin, false),
            (Member, Member, true),
            (Member, Viewer, true),
            // Viewer passes only Viewer.
            (Viewer, Owner, false),
            (Viewer, Admin, false),
            (Viewer, Member, false),
            (Viewer, Viewer, true),
        ];
        for (held, min, expect_ok) in cases {
            let result = ctx_with_roles(vec![held]).require(min);
            assert_eq!(
                result.is_ok(),
                expect_ok,
                "held={held:?} require({min:?}) expected ok={expect_ok}, got {result:?}"
            );
            if !expect_ok {
                assert_eq!(result, Err(AuthzError::InsufficientRole { required: min }));
            }
        }
    }

    #[test]
    fn require_denies_everything_with_no_roles() {
        let ctx = ctx_with_roles(Vec::new());
        for min in ALL_ROLES {
            assert!(
                ctx.require(min).is_err(),
                "empty-roles context must be denied require({min:?})"
            );
        }
    }

    #[test]
    fn require_uses_highest_held_role() {
        let ctx = ctx_with_roles(vec![Role::Viewer, Role::Admin]);
        assert!(ctx.require(Role::Admin).is_ok());
        assert!(ctx.require(Role::Member).is_ok());
        assert!(ctx.require(Role::Viewer).is_ok());
        assert!(ctx.require(Role::Owner).is_err());
    }

    #[test]
    fn role_ordering_owner_is_highest() {
        assert!(Role::Owner > Role::Admin);
        assert!(Role::Admin > Role::Member);
        assert!(Role::Member > Role::Viewer);

        let mut roles = [Role::Member, Role::Owner, Role::Viewer, Role::Admin];
        roles.sort();
        assert_eq!(
            roles,
            [Role::Viewer, Role::Member, Role::Admin, Role::Owner],
            "sort ascending must end with Owner (highest privilege)"
        );
    }

    #[test]
    fn local_context_invariants() {
        let ctx = AuthContext::local();
        assert_eq!(ctx.tenant_id.as_str(), LOCAL_WORKSPACE_ID);
        assert_eq!(ctx.tenant_id.as_str(), "local");
        assert_eq!(ctx.user_id.as_str(), LOCAL_USER_ID);
        assert_eq!(ctx.roles, [Role::Owner]);
        assert!(
            Uuid::parse_str(ctx.request_id.as_str()).is_ok(),
            "request_id must be a well-formed UUID, got {:?}",
            ctx.request_id
        );
    }

    #[test]
    fn local_request_ids_differ_across_calls() {
        let first = AuthContext::local();
        let second = AuthContext::local();
        assert_ne!(first.request_id, second.request_id);
        assert_ne!(RequestId::generate(), RequestId::generate());
    }

    #[test]
    fn current_resolves_to_local_workspace() {
        let ctx = current();
        assert_eq!(ctx.tenant_id.as_str(), LOCAL_WORKSPACE_ID);
        assert_eq!(ctx.user_id.as_str(), LOCAL_USER_ID);
        assert_eq!(ctx.roles, [Role::Owner]);
    }

    #[test]
    fn id_newtypes_serialize_transparently() {
        let tenant = TenantId::new(LOCAL_WORKSPACE_ID);
        let json = serde_json::to_string(&tenant).expect("TenantId must serialize");
        assert_eq!(json, "\"local\"");
        let back: TenantId = serde_json::from_str(&json).expect("TenantId must deserialize");
        assert_eq!(back, tenant);
    }

    #[test]
    fn authz_error_serializes_for_command_mapping() {
        let err = AuthzError::InsufficientRole {
            required: Role::Admin,
        };
        let json = serde_json::to_value(&err).expect("AuthzError must serialize");
        assert_eq!(json["kind"], "insufficient_role");
        assert_eq!(json["required"], "admin");
        assert_eq!(
            err.to_string(),
            "insufficient privileges: this action requires role 'admin' or higher"
        );
    }
}
