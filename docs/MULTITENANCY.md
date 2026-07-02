# Multi-Tenancy Playbook

Goal: reach a **managed multi-tenant SaaS** eventually, without ever breaking the **local-first single-user** experience we ship first. This document is the contract every agent follows when touching data or the server.

## The core principle

> Design **tenant-aware** from commit #1; run **single-tenant/local** until the server exists.

A "tenant" = an organization (a customer company). A "workspace" = a tenant's data boundary. In local-first mode there is exactly **one** implicit workspace with a fixed id (e.g. `local`), and one implicit user. On the server, `tenant_id` is real and enforced.

## Five rules (enforced by hooks, reviews, and RLS)

1. **Every persisted domain entity has `tenant_id` (server) / `workspace_id` (client).** No exceptions. Added in the creating migration. Local value is the constant local workspace id.
2. **All identity comes from `AuthContext`.** Never read "the current user/org" from a global, a hardcoded value, or an implicit singleton in server code. `AuthContext { tenant_id, user_id, roles, request_id }`.
3. **Storage access goes through a tenant-scoped repository** — the only layer that issues queries. Plus **PostgreSQL Row-Level Security** on the server as defense in depth. Two independent barriers.
4. **No cross-tenant access and no god-mode.** No endpoint, admin panel, or query may read across tenants. "Admin" is always scoped to one tenant. Support/ops access, if ever needed, is a separate, audited, break-glass mechanism — not a bypass baked into the app.
5. **Every sensitive action is audited** (append-only): tenant_id, actor, action, resource, timestamp, request_id. Auth events, shares, exports, deletions, policy changes.

## Isolation model (server, Phase 2+)

**Chosen model: shared database, shared schema, isolated by `tenant_id` + RLS.** Simplest to operate at SME scale; upgrade path to schema-per-tenant or DB-per-tenant exists for large/regulated tenants if required.

```sql
-- Every server domain table:
CREATE TABLE meetings (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id     uuid NOT NULL REFERENCES tenants(id),
  title         text NOT NULL,
  created_by    uuid NOT NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  updated_at    timestamptz NOT NULL DEFAULT now(),
  updated_by    uuid,
  rev           bigint NOT NULL DEFAULT 1,
  deleted_at    timestamptz
);
ALTER TABLE meetings ENABLE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON meetings
  USING (tenant_id = current_setting('app.tenant_id')::uuid);
-- The API sets app.tenant_id per request from AuthContext, inside the transaction.
```

**Negative test is mandatory:** the server test suite must include a test proving tenant A cannot read/modify tenant B's rows (via API and, ideally, directly against RLS).

## AuthN / AuthZ

- **AuthN:** OIDC only (Keycloak / Authentik self-host; Entra ID / Google / Okta for SaaS). No home-grown password storage. Validate JWT/session centrally at the gateway.
- **AuthZ (RBAC):** minimum roles `owner`, `admin`, `member`, `viewer`. Checked on every mutating and sensitive-read endpoint. Roles are per-tenant.
- **Local-first mode:** AuthContext resolves to a single local user with `owner`-equivalent rights; no login required. The same code paths run — only the resolver differs.

## Data classification & sync scope

Each record is one of:
- **local-only** — never leaves the device (default for drafts / sensitive captures until the user shares).
- **synced** — replicated to the tenant's server space (encrypted, tenant-scoped) for team access.

The user/policy controls promotion from local-only to synced. Synced records carry `rev`, `updated_by`, `deleted_at` for merge + audit. Client SQLite is authoritative for a user's own local captures; server is authoritative for shared/team records.

## Per-tenant configuration & policy (seams to add early, may be no-ops)

- Allowed LLM providers / models (an enterprise may forbid cloud providers → local Ollama only).
- Retention policy (auto-delete audio/transcripts after N days) and redaction rules.
- Data residency (EU region) selection.
- **Metering counters** (seats, transcription minutes, storage) — even if unused in Phase 1, so billing is not a later rewrite.
- Branding (name/logo) for white-label.

## Migration path summary

| Phase | Tenancy | Auth | Storage | What changes in code |
|---|---|---|---|---|
| 1 — Local-first enterprise | 1 implicit workspace | none (local user) | client SQLite (SQLCipher) | seams present, resolvers are constants |
| 2 — Enterprise self-host | real multi-tenant | OIDC (Keycloak/Authentik) | + server Postgres + RLS + audit | server implements the resolvers; sync turns on |
| 3 — Managed SaaS | real multi-tenant, hardened | hosted IdP | scaled Postgres, per-tenant metering/billing | ops/scale/isolation hardening, not app rewrite |

Because seams 1–4 exist from Phase 1, Phase 2 is **additive** (build the server + fill in the resolvers), not a rewrite of the client.

## Anti-patterns (auto-reject in review / `/tenant-check`)
- A tenant-data table without `tenant_id`.
- A server query without RLS **and** without explicit tenant scoping.
- Reading the current user/org from anywhere but `AuthContext`.
- Any cross-tenant join, list-all, or admin bypass.
- Introducing a Ring-2 dependency into a Ring-1 (local) feature path.
