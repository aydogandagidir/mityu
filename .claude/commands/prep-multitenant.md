---
description: Assess and prepare a module/entity for the future multi-tenant SaaS without breaking local-first today.
argument-hint: <module or entity to prepare, e.g. "meetings storage">
---
Prepare **$ARGUMENTS** for eventual multi-tenant SaaS while staying single-tenant/local-first now.

1. **Map current state.** Where does this entity live (client SQLite? Rust module? UI?)? Does it already carry `workspace_id`? Read docs/MULTITENANCY.md + docs/DATA_MODEL.md.
2. **Identify tenant-hostile assumptions:** global singletons, tenant-less tables, implicit "the one user", cross-record reads that would leak across tenants, config that should be per-tenant.
3. **Make it tenant-aware, non-breaking:**
   - Add `workspace_id`/`tenant_id` (default to the single local workspace id in local-first) via a forward-only migration (use /db-migration).
   - Wrap access behind a repository that takes tenant context (a constant today, real AuthContext on the server).
   - Introduce an `AuthContext`/`WorkspaceContext` seam even if it currently resolves to the single local user.
   - Add per-tenant metering counters as no-ops if relevant.
4. **Prove local-first still holds** (network OFF, server OFF).
5. Record the decision + the migration path in docs/DECISIONS.md. Invoke multitenancy-guardian to review.
Do NOT stand up the server or add auth in this command — this is preparation of the seam only.
