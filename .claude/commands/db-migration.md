---
description: Author a forward-only, idempotent schema migration for client SQLite and/or server Postgres.
argument-hint: <what the schema change is>
---
Author a migration for: **$ARGUMENTS** (delegate to db-migration-engineer).

1. Read docs/DATA_MODEL.md. Keep client SQLite and server Postgres **logically compatible**.
2. Create a NEW migration (increasing id + clear name). Never edit an applied one.
3. Ensure the table has `id uuid`, `workspace_id/tenant_id`, `created_at`, `updated_at` (+ `updated_by`, `version/rev`, `deleted_at` if synced).
4. If it is a server table, add its **RLS policy in the same migration**.
5. If it is a **synced** table, write a **sync-compatibility note** (additive columns w/ defaults; two-step for renames/drops).
6. Provide up + documented down. Test apply on empty AND populated DBs.
7. Update docs/DATA_MODEL.md.
