# CONTRACTS — Interfaces the agent implements verbatim (adapt names to the real code)

These are the load-bearing seams. Implement them with these shapes so tenant-safety and local-first are structurally guaranteed, not left to per-feature discipline. Code below is a **starter sketch** — wire it to the actual repo modules; keep the semantics.

## 1. Identity: `AuthContext` / `WorkspaceContext`
Single source of "who + which tenant" for all domain operations. In local mode it resolves to one implicit user/workspace; on the server it comes from the validated OIDC token. **No other code path may determine the current user/tenant.**

```rust
// core: src-tauri/src/context.rs  (and mirrored server-side)
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub tenant_id: TenantId,     // local mode: the constant LOCAL_WORKSPACE_ID
    pub user_id: UserId,         // local mode: the single local user
    pub roles: Vec<Role>,        // local mode: [Role::Owner]
    pub request_id: RequestId,   // for audit correlation
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role { Owner, Admin, Member, Viewer }

pub const LOCAL_WORKSPACE_ID: &str = "local";

impl AuthContext {
    pub fn local() -> Self { /* single local user, Owner role */ }
    pub fn require(&self, min: Role) -> Result<(), AuthzError> { /* RBAC check */ }
}
```

## 2. Storage: tenant-scoped `Repository`
The **only** place that issues queries. Every method takes `&AuthContext` and filters by `ctx.tenant_id`. On the server, Postgres RLS is the second barrier (defense in depth). No ad-hoc SQL in feature/command code.

```rust
// core: src-tauri/src/repository/mod.rs
#[async_trait::async_trait]
pub trait MeetingRepository: Send + Sync {
    async fn create(&self, ctx: &AuthContext, m: NewMeeting) -> Result<Meeting>;
    async fn get(&self, ctx: &AuthContext, id: MeetingId) -> Result<Option<Meeting>>;
    async fn list(&self, ctx: &AuthContext, page: Page) -> Result<Vec<Meeting>>;
    async fn update(&self, ctx: &AuthContext, id: MeetingId, patch: MeetingPatch) -> Result<Meeting>;
    async fn soft_delete(&self, ctx: &AuthContext, id: MeetingId) -> Result<()>;
}
// Implementations MUST inject `WHERE tenant_id = ctx.tenant_id` (or set app.tenant_id for RLS).
// A query over tenant data without ctx is a review BLOCKER (tenant-scope-check.sh + /tenant-check).
```

Analogous repos: `TranscriptRepository`, `SummaryRepository`, `ActionItemRepository`, `SettingsRepository`.

## 3. Provider abstraction (LLM) — provider-agnostic + BYOK
Summarization/extraction depend on a trait, not a vendor. Keys come from the secure store, never SQLite/source.

```rust
// core: src-tauri/src/llm/provider.rs
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &'static str;                 // "ollama" | "openai" | "anthropic" | "groq" | "openrouter"
    fn is_local(&self) -> bool;                   // ollama => true (offline-capable)
    async fn structured_summary(&self, transcript: &Transcript, schema: &SummarySchema)
        -> Result<MeetingNotesDraft>;             // returns DRAFT, source_chunk_id per block
}
// Selection is per-workspace policy (allowed_providers). Default offline: Ollama.
```

## 4. Summary schema (source-linked, HITL) — mandatory `source_chunk_id`
```
MeetingNotesDraft { meeting_id, status: Draft, sections: [Section] }
Section           { title, blocks: [Block] }
Block             { id, type: text|bullet|heading1|heading2, content, source_chunk_id }   // source_chunk_id REQUIRED
ActionItemDraft   { id, text, assignee?, due?, status, source_chunk_id }                   // source_chunk_id REQUIRED
```
Generation repositories force every incoming summary block and action item to
`draft`; a producer-supplied status may not mint approval. No block/action item
may be persisted as `approved` without an explicit human Approve action and a
resolvable `source_chunk_id`. Resolvable means an active transcript row for the
same active meeting in the caller's workspace; soft-deleted sources/meetings do
not resolve.

## 5. Sync protocol (dormant until Phase 2) — tenant-scoped, mergeable
```jsonc
// client push item
{ "tenant_id": "...", "entity": "meeting|transcript|summary|action_item",
  "id": "uuid", "rev": 42, "updated_by": "user_id", "updated_at": "ISO-8601",
  "deleted": false, "payload": { /* entity fields */ } }
// server ack
{ "id": "uuid", "server_rev": 43, "conflict": false }   // conflict=true => LWW applied + audit entry written
```
Rules: client SQLite is authoritative for a user's own local captures; server is authoritative for shared records; conflicts resolve per-field last-write-wins with an audit note; deletes are soft and propagate. `provider_credential` secrets NEVER sync (only a non-secret label may).

## 6. Audit (server) — append-only
```rust
struct AuditEvent { tenant_id, actor: UserId, action: String, resource: String,
                    resource_id: String, ts: Timestamp, request_id: RequestId }
// Emit on: auth events, shares, exports, deletions, policy/role changes. Never mutable.
```

## 7. Common entity fields (enforced by migrations)
`id: uuid` · `tenant_id`/`workspace_id` · `created_at` · `updated_at`; synced entities also `updated_by`, `rev` (monotonic), `deleted_at`. See docs/DATA_MODEL.md.
