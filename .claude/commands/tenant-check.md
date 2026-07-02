---
description: Audit server code for tenant isolation and local-first integrity. Read-only.
---
Run a tenant-safety + local-first audit (delegate to multitenancy-guardian):

1. Grep `server/` for queries over tenant data (meetings, transcripts, summaries, action_items, documents) and confirm EACH is either behind Postgres RLS or explicitly tenant-scoped. List any that are not, with file:line.
2. Confirm no cross-tenant joins/reads and no god-mode/admin-bypass endpoint.
3. Confirm every server entity has `tenant_id`; every endpoint derives identity from AuthContext (never hardcoded/assumed single-user).
4. Confirm new endpoints are RBAC-checked and audited.
5. Confirm the desktop app still works with the server OFF and the core path with the network OFF.
Output a PASS/BLOCKER/WARN table with file:line and the exact violated rule from docs/MULTITENANCY.md. Do not edit code; recommend fixes.
