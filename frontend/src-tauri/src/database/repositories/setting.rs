//! Tenant-scoped settings repository (docs/CONTRACTS.md §2, BACKLOG B2 phase 2).
//!
//! `settings` / `transcript_settings` are per-workspace config tables (NOT
//! synced: no `rev`/`updated_by`/`deleted_at`), but they carry `workspace_id`
//! plus nullable `created_at`/`updated_at` — every write here MUST populate the
//! timestamps and every statement is scoped by `workspace_id = ctx.tenant_id`.
//!
//! Legacy single-row shape: both tables keep the historical `id = '1'` primary
//! key. Until a Phase-2 migration widens the key to (id, workspace_id), the
//! upserts guard their DO UPDATE with `WHERE workspace_id = excluded.workspace_id`
//! so a foreign workspace can never clobber another workspace's row (the
//! statement degrades to a no-op instead).
//!
//! ## BYOK secrets (CLAUDE.md §0.7/§3, docs/SECURITY_PRIVACY.md "Secrets")
//! The `*ApiKey` columns of `settings` / `transcript_settings` are NOT the store
//! of record for LLM/STT keys. The real secret lives in the OS credential store
//! (see [`crate::secrets`]); the column holds only the non-secret
//! [`crate::secrets::KEYCHAIN_MARKER`] reference so the row/timestamps and schema
//! stay intact and older binaries keep parsing the table. `save_api_key` /
//! `get_api_key` (and the transcript equivalents) round-trip through the keychain;
//! [`SettingsRepository::migrate_plaintext_keys_to_keychain`] moves any legacy
//! plaintext still sitting in a column into the keychain on startup.

use crate::context::AuthContext;
use crate::database::models::{Setting, TranscriptSetting};
use crate::secrets::{self, SecretDomain, KEYCHAIN_MARKER};
use crate::summary::CustomOpenAIConfig;
use chrono::Utc;
use sqlx::SqlitePool;

/// Map a keychain (`anyhow`) failure onto `sqlx::Error` so the repository keeps
/// its existing `Result<_, sqlx::Error>` signatures (callers already surface
/// these to the UI). Fail closed: a keychain failure becomes a real error, never
/// a silent success or a plaintext fallback. Key *values* are never included.
fn keychain_err(e: anyhow::Error) -> sqlx::Error {
    sqlx::Error::Protocol(format!("{e:#}"))
}

#[derive(serde::Deserialize, Debug)]
pub struct SaveModelConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "whisperModel")]
    pub whisper_model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "ollamaEndpoint")]
    pub ollama_endpoint: Option<String>,
}

#[derive(serde::Deserialize, Debug)]
pub struct SaveTranscriptConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
}

pub struct SettingsRepository;

// Transcript providers: localWhisper, deepgram, elevenLabs, groq, openai
// Summary providers: openai, claude, ollama, groq, added openrouter
// NOTE: Handle data exclusion in the higher layer as this is database abstraction layer(using SELECT *)

impl SettingsRepository {
    pub async fn get_model_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<Option<Setting>, sqlx::Error> {
        let setting =
            sqlx::query_as::<_, Setting>("SELECT * FROM settings WHERE workspace_id = ? LIMIT 1")
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(pool)
                .await?;
        Ok(setting)
    }

    pub async fn save_model_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
        model: &str,
        whisper_model: &str,
        ollama_endpoint: Option<&str>,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = Utc::now();
        // Using id '1' for backward compatibility
        sqlx::query(
            r#"
            INSERT INTO settings (id, workspace_id, provider, model, whisperModel, ollamaEndpoint, created_at, updated_at)
            VALUES ('1', ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                model = excluded.model,
                whisperModel = excluded.whisperModel,
                ollamaEndpoint = excluded.ollamaEndpoint,
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
        )
        .bind(ctx.tenant_id.as_str())
        .bind(provider)
        .bind(model)
        .bind(whisper_model)
        .bind(ollama_endpoint)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Column holding the non-secret keychain marker for a summary (`settings`)
    /// provider. `Ok(None)` for providers that never carry a key (built-in AI) or
    /// that are handled elsewhere (custom-openai → JSON config). `Err` for an
    /// unknown provider. The column NO LONGER stores the secret itself.
    fn summary_key_column(
        provider: &str,
    ) -> std::result::Result<Option<&'static str>, sqlx::Error> {
        Ok(Some(match provider {
            "openai" => "openaiApiKey",
            "claude" => "anthropicApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(None), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(format!(
                    "Invalid provider: {}",
                    provider
                )))
            }
        }))
    }

    pub async fn save_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        // Custom OpenAI uses JSON config (customOpenAIConfig) instead of a separate API key column
        if provider == "custom-openai" {
            return Err(sqlx::Error::Protocol(
                "custom-openai provider should use save_custom_openai_config() instead of save_api_key()".into(),
            ));
        }

        let api_key_column = match Self::summary_key_column(provider)? {
            Some(col) => col,
            None => return Ok(()), // builtin-ai: nothing to store
        };

        // Secrets never touch SQLite: write the real key to the OS credential
        // store first (fail closed — abort before mutating the DB if unavailable),
        // then persist only the non-secret marker in the column.
        secrets::set_api_key(ctx, SecretDomain::Summary, provider, api_key)
            .map_err(keychain_err)?;

        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO settings (id, workspace_id, provider, model, whisperModel, created_at, updated_at, "{col}")
            VALUES ('1', ?, 'openai', 'gpt-4o-2024-11-20', 'large-v3', ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                "{col}" = excluded."{col}",
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
            col = api_key_column
        );
        sqlx::query(&query)
            .bind(ctx.tenant_id.as_str())
            .bind(now)
            .bind(now)
            .bind(KEYCHAIN_MARKER)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn get_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
    ) -> std::result::Result<Option<String>, sqlx::Error> {
        // Custom OpenAI uses JSON config - extract API key from there
        if provider == "custom-openai" {
            let config = Self::get_custom_openai_config(pool, ctx).await?;
            return Ok(config.and_then(|c| c.api_key));
        }

        // Validate provider + gate builtin-ai / unknowns exactly as before. The
        // column value is now only a marker; the secret comes from the keychain.
        if Self::summary_key_column(provider)?.is_none() {
            return Ok(None); // builtin-ai: no API key
        }

        // Lazily migrate a legacy plaintext key still sitting in this column, so a
        // read after upgrade succeeds even before the startup sweep has run.
        Self::migrate_summary_column_if_plaintext(pool, ctx, provider).await?;

        secrets::get_api_key(ctx, SecretDomain::Summary, provider).map_err(keychain_err)
    }

    pub async fn get_transcript_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<Option<TranscriptSetting>, sqlx::Error> {
        let setting = sqlx::query_as::<_, TranscriptSetting>(
            "SELECT * FROM transcript_settings WHERE workspace_id = ? LIMIT 1",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        Ok(setting)
    }

    /// Returns `(provider, model)` from the workspace's transcript settings, if
    /// configured. Thin projection used by the audio import/retranscription
    /// flows so they do not issue SQL themselves.
    pub async fn get_transcript_provider_model(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<Option<(String, String)>, sqlx::Error> {
        sqlx::query_as(
            "SELECT provider, model FROM transcript_settings \
             WHERE id = '1' AND workspace_id = ? LIMIT 1",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await
    }

    pub async fn save_transcript_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
        model: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO transcript_settings (id, workspace_id, provider, model, created_at, updated_at)
            VALUES ('1', ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                provider = excluded.provider,
                model = excluded.model,
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
        )
        .bind(ctx.tenant_id.as_str())
        .bind(provider)
        .bind(model)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Column holding the non-secret keychain marker for a transcript
    /// (`transcript_settings`) provider. `Ok(None)` for providers that carry no
    /// key (parakeet). `Err` for an unknown provider.
    fn transcript_key_column(
        provider: &str,
    ) -> std::result::Result<Option<&'static str>, sqlx::Error> {
        Ok(Some(match provider {
            "localWhisper" => "whisperApiKey",
            "parakeet" => return Ok(None), // Parakeet doesn't need an API key
            "deepgram" => "deepgramApiKey",
            "elevenLabs" => "elevenLabsApiKey",
            "groq" => "groqApiKey",
            "openai" => "openaiApiKey",
            _ => {
                return Err(sqlx::Error::Protocol(format!(
                    "Invalid provider: {}",
                    provider
                )))
            }
        }))
    }

    pub async fn save_transcript_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        let api_key_column = match Self::transcript_key_column(provider)? {
            Some(col) => col,
            None => return Ok(()), // parakeet: no API key needed
        };

        // Secret to the OS credential store first (fail closed), marker to the DB.
        secrets::set_api_key(ctx, SecretDomain::Transcript, provider, api_key)
            .map_err(keychain_err)?;

        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO transcript_settings (id, workspace_id, provider, model, created_at, updated_at, "{col}")
            VALUES ('1', ?, 'parakeet', '{model}', ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                "{col}" = excluded."{col}",
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
            col = api_key_column,
            model = crate::config::DEFAULT_PARAKEET_MODEL
        );
        sqlx::query(&query)
            .bind(ctx.tenant_id.as_str())
            .bind(now)
            .bind(now)
            .bind(KEYCHAIN_MARKER)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn get_transcript_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
    ) -> std::result::Result<Option<String>, sqlx::Error> {
        if Self::transcript_key_column(provider)?.is_none() {
            return Ok(None); // parakeet: no API key
        }

        // Lazily migrate any legacy plaintext still in this column before reading.
        Self::migrate_transcript_column_if_plaintext(pool, ctx, provider).await?;

        secrets::get_api_key(ctx, SecretDomain::Transcript, provider).map_err(keychain_err)
    }

    pub async fn delete_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = Utc::now();

        // Custom OpenAI uses JSON config - clear the entire config
        if provider == "custom-openai" {
            sqlx::query(
                "UPDATE settings SET customOpenAIConfig = NULL, updated_at = ? \
                 WHERE id = '1' AND workspace_id = ?",
            )
            .bind(now)
            .bind(ctx.tenant_id.as_str())
            .execute(pool)
            .await?;
            return Ok(());
        }

        let api_key_column = match Self::summary_key_column(provider)? {
            Some(col) => col,
            None => return Ok(()), // builtin-ai: nothing to delete
        };

        // Remove the real secret from the OS credential store (no-op if absent),
        // then clear the column marker. Keychain first so a store failure aborts
        // before we drop the DB reference (fail closed, no orphaned secret).
        secrets::delete_api_key(ctx, SecretDomain::Summary, provider).map_err(keychain_err)?;

        let query = format!(
            "UPDATE settings SET {} = NULL, updated_at = ? WHERE id = '1' AND workspace_id = ?",
            api_key_column
        );
        sqlx::query(&query)
            .bind(now)
            .bind(ctx.tenant_id.as_str())
            .execute(pool)
            .await?;

        Ok(())
    }

    // ===== CUSTOM OPENAI CONFIG METHODS =====

    /// Gets the custom OpenAI configuration from JSON
    ///
    /// # Returns
    /// * `Ok(Some(CustomOpenAIConfig))` - Config exists and is valid JSON
    /// * `Ok(None)` - No config stored
    /// * `Err(sqlx::Error)` - Database error
    pub async fn get_custom_openai_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<Option<CustomOpenAIConfig>, sqlx::Error> {
        use sqlx::Row;

        let row = sqlx::query(
            r#"
            SELECT customOpenAIConfig
            FROM settings
            WHERE id = '1' AND workspace_id = ?
            LIMIT 1
            "#,
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;

        match row {
            Some(record) => {
                let config_json: Option<String> = record.get("customOpenAIConfig");

                if let Some(json) = config_json {
                    // Parse JSON into CustomOpenAIConfig
                    let config: CustomOpenAIConfig = serde_json::from_str(&json).map_err(|e| {
                        sqlx::Error::Protocol(format!("Invalid JSON in customOpenAIConfig: {}", e))
                    })?;

                    Ok(Some(config))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Saves the custom OpenAI configuration as JSON
    ///
    /// # Arguments
    /// * `pool` - Database connection pool
    /// * `ctx` - Workspace/user identity the write is scoped to
    /// * `config` - CustomOpenAIConfig to save (includes endpoint, apiKey, model, maxTokens, temperature, topP)
    ///
    /// # Returns
    /// * `Ok(())` - Config saved successfully
    /// * `Err(sqlx::Error)` - Database or JSON serialization error
    pub async fn save_custom_openai_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
        config: &CustomOpenAIConfig,
    ) -> std::result::Result<(), sqlx::Error> {
        // Serialize config to JSON
        let config_json = serde_json::to_string(config).map_err(|e| {
            sqlx::Error::Protocol(format!("Failed to serialize config to JSON: {}", e))
        })?;

        let now = Utc::now();

        // Upsert into settings table
        sqlx::query(
            r#"
            INSERT INTO settings (id, workspace_id, provider, model, whisperModel, customOpenAIConfig, created_at, updated_at)
            VALUES ('1', ?, 'custom-openai', ?, 'large-v3', ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                customOpenAIConfig = excluded.customOpenAIConfig,
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
        )
        .bind(ctx.tenant_id.as_str())
        .bind(&config.model)
        .bind(config_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    // ===== REDACTION CONFIG (per-workspace PII/keyword redaction policy, BACKLOG C6) =====

    /// Reads the workspace's [`RedactionConfig`] from the `settings.redactionConfig`
    /// JSON column. Returns [`RedactionConfig::default`] (disabled — the
    /// non-breaking, local-first default) when no row/column value exists or when a
    /// stored blob fails to parse, so a missing/corrupt config can never *enable*
    /// redaction implicitly. Scoped by `workspace_id = ctx.tenant_id`. This holds no
    /// secret, so it is a plain column (unlike the `*ApiKey` markers above).
    pub async fn get_redaction_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<crate::redaction::RedactionConfig, sqlx::Error> {
        let json: Option<String> = sqlx::query_scalar(
            "SELECT redactionConfig FROM settings WHERE id = '1' AND workspace_id = ? LIMIT 1",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?
        .flatten();

        match json {
            Some(raw) if !raw.trim().is_empty() => match serde_json::from_str(&raw) {
                Ok(cfg) => Ok(cfg),
                Err(e) => {
                    // Fail safe: never silently enable redaction from a bad blob, and
                    // never log the blob itself.
                    tracing::warn!(
                        workspace_id = %ctx.tenant_id,
                        error = %e,
                        "redactionConfig failed to parse; falling back to disabled default"
                    );
                    Ok(crate::redaction::RedactionConfig::default())
                }
            },
            _ => Ok(crate::redaction::RedactionConfig::default()),
        }
    }

    /// Upserts the workspace's [`RedactionConfig`] as JSON into the
    /// `settings.redactionConfig` column. Mirrors [`Self::save_custom_openai_config`]:
    /// legacy single-row (`id = '1'`) with the `WHERE workspace_id = excluded.workspace_id`
    /// guard so a foreign workspace cannot clobber another's row. Stamps `updated_at`.
    pub async fn set_redaction_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
        config: &crate::redaction::RedactionConfig,
    ) -> std::result::Result<(), sqlx::Error> {
        let config_json = serde_json::to_string(config).map_err(|e| {
            sqlx::Error::Protocol(format!(
                "Failed to serialize RedactionConfig to JSON: {}",
                e
            ))
        })?;

        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO settings (id, workspace_id, provider, model, whisperModel, redactionConfig, created_at, updated_at)
            VALUES ('1', ?, 'ollama', 'llama3.2:latest', 'large-v3', ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                redactionConfig = excluded.redactionConfig,
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
        )
        .bind(ctx.tenant_id.as_str())
        .bind(config_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    // ===== LEARNING CONFIG (per-workspace learning policy, ADR-0024 §7) =====

    /// Reads the workspace's [`LearningConfig`] from the `settings.learningConfig`
    /// JSON column. Scoped by `workspace_id = ctx.tenant_id`; holds no secret, so
    /// it is a plain column.
    ///
    /// Mirrors [`Self::get_redaction_config`] with ONE deliberate difference, and
    /// it is the important one. Redaction's safe fallback happens to equal its
    /// default (disabled); learning's does not — its default has auto-activation
    /// ON. So absent and corrupt are handled differently:
    ///
    /// - **absent** (no row / no value) → [`LearningConfig::default`]: no
    ///   preference was ever expressed, so the fresh-install product default
    ///   applies.
    /// - **corrupt** (a stored blob that will not parse) →
    ///   [`LearningConfig::disabled`]: the user HAD preferences and we cannot read
    ///   them, so falling back to the default could silently re-enable
    ///   auto-activation for someone who deliberately switched it off. Same
    ///   principle as "never silently enable redaction from a bad blob"; the
    ///   fallback differs only because the two features' safe states differ.
    ///
    /// Every field of the blob carries a serde default, so an older or partial
    /// blob parses (filling gaps from the product default) rather than tripping
    /// the corrupt path.
    pub async fn get_learning_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<crate::learning::config::LearningConfig, sqlx::Error> {
        let json: Option<String> = sqlx::query_scalar(
            "SELECT learningConfig FROM settings WHERE id = '1' AND workspace_id = ? LIMIT 1",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?
        .flatten();

        match json {
            Some(raw) if !raw.trim().is_empty() => match serde_json::from_str(&raw) {
                Ok(cfg) => Ok(cfg),
                Err(e) => {
                    // Never log the blob itself.
                    tracing::warn!(
                        workspace_id = %ctx.tenant_id,
                        error = %e,
                        "learningConfig failed to parse; falling back to the DISABLED config \
                         (not the default — a lost preference must not become an implicit yes)"
                    );
                    Ok(crate::learning::config::LearningConfig::disabled())
                }
            },
            _ => Ok(crate::learning::config::LearningConfig::default()),
        }
    }

    /// Upserts the workspace's [`LearningConfig`] as JSON into the
    /// `settings.learningConfig` column. Mirrors [`Self::set_redaction_config`]:
    /// legacy single-row (`id = '1'`) with the
    /// `WHERE workspace_id = excluded.workspace_id` guard so a foreign workspace
    /// cannot clobber another's row.
    pub async fn set_learning_config(
        pool: &SqlitePool,
        ctx: &AuthContext,
        config: &crate::learning::config::LearningConfig,
    ) -> std::result::Result<(), sqlx::Error> {
        let config_json = serde_json::to_string(config).map_err(|e| {
            sqlx::Error::Protocol(format!("Failed to serialize LearningConfig to JSON: {}", e))
        })?;

        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO settings (id, workspace_id, provider, model, whisperModel, learningConfig, created_at, updated_at)
            VALUES ('1', ?, 'ollama', 'llama3.2:latest', 'large-v3', ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                learningConfig = excluded.learningConfig,
                updated_at = excluded.updated_at
            WHERE workspace_id = excluded.workspace_id
            "#,
        )
        .bind(ctx.tenant_id.as_str())
        .bind(config_json)
        .bind(now)
        .bind(now)
        .execute(pool)
        .await?;

        Ok(())
    }

    // ===== KEYCHAIN MIGRATION (legacy plaintext columns → OS credential store) =====

    /// `(provider, column)` pairs for summary (`settings`) key columns. Drives
    /// both the sweep and provider→column mapping for migration. `geminiApiKey`
    /// exists in the schema but is not yet wired to a provider; it is swept
    /// anyway so no plaintext is left behind if a future column gets populated.
    const SUMMARY_KEY_COLUMNS: &'static [(&'static str, &'static str)] = &[
        ("openai", "openaiApiKey"),
        ("claude", "anthropicApiKey"),
        ("ollama", "ollamaApiKey"),
        ("groq", "groqApiKey"),
        ("openrouter", "openRouterApiKey"),
        ("gemini", "geminiApiKey"),
    ];

    /// `(provider, column)` pairs for transcript (`transcript_settings`) columns.
    const TRANSCRIPT_KEY_COLUMNS: &'static [(&'static str, &'static str)] = &[
        ("localWhisper", "whisperApiKey"),
        ("deepgram", "deepgramApiKey"),
        ("elevenLabs", "elevenLabsApiKey"),
        ("groq", "groqApiKey"),
        ("openai", "openaiApiKey"),
    ];

    /// True when a column value is a real legacy plaintext key (present, non-empty,
    /// and not already the keychain marker) that still needs migrating.
    fn is_legacy_plaintext(value: &Option<String>) -> bool {
        matches!(value, Some(v) if !v.is_empty() && v != KEYCHAIN_MARKER)
    }

    /// Move the value of one `(table, column)` cell for the given workspace into
    /// the keychain and overwrite the column with the marker. Idempotent: a
    /// missing/empty/already-migrated cell is a no-op. Runs fully offline.
    async fn migrate_column_cell(
        pool: &SqlitePool,
        ctx: &AuthContext,
        table: &str,
        column: &str,
        domain: SecretDomain,
        provider: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        let current: Option<String> = sqlx::query_scalar(&format!(
            "SELECT \"{column}\" FROM {table} WHERE id = '1' AND workspace_id = ? LIMIT 1"
        ))
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?
        .flatten();

        if !Self::is_legacy_plaintext(&current) {
            return Ok(());
        }
        let plaintext = current.expect("is_legacy_plaintext guarantees Some");

        // Store to the OS credential store first; only blank the column once the
        // secret is safely in the keychain (fail closed — never lose the key).
        secrets::set_api_key(ctx, domain, provider, &plaintext).map_err(keychain_err)?;

        let now = Utc::now();
        sqlx::query(&format!(
            "UPDATE {table} SET \"{column}\" = ?, updated_at = ? \
             WHERE id = '1' AND workspace_id = ?"
        ))
        .bind(KEYCHAIN_MARKER)
        .bind(now)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;

        tracing::info!(
            provider,
            domain = ?domain,
            workspace_id = %ctx.tenant_id,
            "migrated a legacy plaintext API key from SQLite into the OS credential store"
        );
        Ok(())
    }

    /// Lazy migration for a single summary provider's column (called on read).
    async fn migrate_summary_column_if_plaintext(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        if let Some(column) = Self::summary_key_column(provider)? {
            Self::migrate_column_cell(
                pool,
                ctx,
                "settings",
                column,
                SecretDomain::Summary,
                provider,
            )
            .await?;
        }
        Ok(())
    }

    /// Lazy migration for a single transcript provider's column (called on read).
    async fn migrate_transcript_column_if_plaintext(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        if let Some(column) = Self::transcript_key_column(provider)? {
            Self::migrate_column_cell(
                pool,
                ctx,
                "transcript_settings",
                column,
                SecretDomain::Transcript,
                provider,
            )
            .await?;
        }
        Ok(())
    }

    /// One-time startup sweep: move every legacy plaintext API key still stored
    /// in a `settings` / `transcript_settings` column into the OS credential
    /// store, then overwrite the column with [`KEYCHAIN_MARKER`]. Iterates over
    /// **every workspace row** (using each row's own `workspace_id`, not just
    /// `local`) so a multi-workspace DB migrates correctly, and is fully idempotent
    /// and offline. Safe to call on every startup.
    ///
    /// Returns the number of keys migrated (useful for logging/tests).
    pub async fn migrate_plaintext_keys_to_keychain(
        pool: &SqlitePool,
    ) -> std::result::Result<usize, sqlx::Error> {
        let mut migrated = 0usize;

        for (table, columns, domain) in [
            ("settings", Self::SUMMARY_KEY_COLUMNS, SecretDomain::Summary),
            (
                "transcript_settings",
                Self::TRANSCRIPT_KEY_COLUMNS,
                SecretDomain::Transcript,
            ),
        ] {
            // Every distinct workspace that owns a row in this table.
            let workspaces: Vec<String> =
                sqlx::query_scalar(&format!("SELECT DISTINCT workspace_id FROM {table}"))
                    .fetch_all(pool)
                    .await?;

            for workspace_id in workspaces {
                // Build a data-migration context from the persisted workspace id.
                // (Not identity resolution — we are moving already-owned rows.)
                let ctx = crate::context::AuthContext {
                    tenant_id: crate::context::TenantId::new(workspace_id.clone()),
                    user_id: crate::context::UserId::new(crate::context::LOCAL_USER_ID),
                    roles: vec![crate::context::Role::Owner],
                    request_id: crate::context::RequestId::generate(),
                };

                for (provider, column) in columns {
                    let before: Option<String> = sqlx::query_scalar(&format!(
                        "SELECT \"{column}\" FROM {table} WHERE id = '1' AND workspace_id = ? LIMIT 1"
                    ))
                    .bind(&workspace_id)
                    .fetch_optional(pool)
                    .await?
                    .flatten();

                    if Self::is_legacy_plaintext(&before) {
                        Self::migrate_column_cell(pool, &ctx, table, column, domain, provider)
                            .await?;
                        migrated += 1;
                    }
                }
            }
        }

        if migrated > 0 {
            tracing::info!(
                count = migrated,
                "startup: migrated legacy plaintext API keys from SQLite into the OS credential store"
            );
        }
        Ok(migrated)
    }
}
