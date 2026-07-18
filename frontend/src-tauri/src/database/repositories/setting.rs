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
        // Custom OpenAI keeps only non-secret endpoint/model settings in JSON;
        // hydrate its key from the workspace-scoped OS credential store.
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

        // Custom OpenAI has a dedicated keychain entry. Remove the secret
        // before clearing its non-secret JSON config.
        if provider == "custom-openai" {
            secrets::delete_api_key(ctx, SecretDomain::Summary, provider).map_err(keychain_err)?;
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

        // Lazy upgrade covers calls made before/without the startup sweep.
        Self::migrate_custom_openai_json_if_plaintext(pool, ctx).await?;

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
                    let mut config: CustomOpenAIConfig =
                        serde_json::from_str(&json).map_err(|e| {
                            sqlx::Error::Protocol(format!(
                                "Invalid JSON in customOpenAIConfig: {}",
                                e
                            ))
                        })?;

                    config.api_key =
                        secrets::get_api_key(ctx, SecretDomain::Summary, "custom-openai")
                            .map_err(keychain_err)?;

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
        match config
            .api_key
            .as_deref()
            .filter(|key| !key.trim().is_empty())
        {
            Some(api_key) => {
                secrets::set_api_key(ctx, SecretDomain::Summary, "custom-openai", api_key)
                    .map_err(keychain_err)?
            }
            // An omitted key means "preserve the credential already stored".
            // Explicit deletion uses `delete_api_key`; a blank settings form
            // must never silently erase a secret it was intentionally not
            // allowed to read back from the OS keychain.
            None => {}
        }

        // The JSON column is configuration only; it must never contain the key.
        let mut non_secret_config = config.clone();
        non_secret_config.api_key = None;
        let config_json = serde_json::to_string(&non_secret_config).map_err(|e| {
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

    // ===== LEARNING CONFIG (per-workspace learning policy, ADR-0030 §7) =====

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

    /// Remove a legacy custom-openai key embedded in JSON. The key is copied to
    /// the OS credential store when available. If that store is unavailable,
    /// plaintext is still removed from SQLite and user re-entry is required.
    async fn migrate_custom_openai_json_if_plaintext(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> std::result::Result<bool, sqlx::Error> {
        let current: Option<String> = sqlx::query_scalar(
            "SELECT customOpenAIConfig FROM settings \
             WHERE id = '1' AND workspace_id = ? LIMIT 1",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?
        .flatten();

        let Some(raw) = current else {
            return Ok(false);
        };
        let mut config: CustomOpenAIConfig = match serde_json::from_str(&raw) {
            Ok(config) => config,
            Err(_) => {
                sqlx::query(
                    "UPDATE settings SET customOpenAIConfig = NULL, updated_at = ? \
                     WHERE id = '1' AND workspace_id = ?",
                )
                .bind(Utc::now())
                .bind(ctx.tenant_id.as_str())
                .execute(pool)
                .await?;
                tracing::warn!(
                    workspace_id = %ctx.tenant_id,
                    "removed invalid legacy custom OpenAI JSON from SQLite because it could contain a plaintext secret"
                );
                return Ok(true);
            }
        };

        let Some(legacy_key) = config.api_key.take() else {
            return Ok(false);
        };
        let key_saved = if legacy_key.trim().is_empty() || legacy_key == KEYCHAIN_MARKER {
            false
        } else {
            secrets::set_api_key(ctx, SecretDomain::Summary, "custom-openai", &legacy_key).is_ok()
        };

        let sanitized = serde_json::to_string(&config).map_err(|error| {
            sqlx::Error::Protocol(format!("Failed to sanitize custom OpenAI config: {error}"))
        })?;
        sqlx::query(
            "UPDATE settings SET customOpenAIConfig = ?, updated_at = ? \
             WHERE id = '1' AND workspace_id = ?",
        )
        .bind(sanitized)
        .bind(Utc::now())
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;

        if key_saved {
            tracing::info!(
                workspace_id = %ctx.tenant_id,
                "migrated legacy custom OpenAI key to the OS credential store"
            );
        } else {
            tracing::warn!(
                workspace_id = %ctx.tenant_id,
                "removed legacy custom OpenAI key from SQLite; OS credential store was unavailable or the value was empty"
            );
        }
        Ok(true)
    }

    /// Move the value of one `(table, column)` cell for the given workspace into
    /// the keychain and overwrite the column with the marker. If the credential
    /// store is unavailable, scrub the plaintext and require user re-entry.
    /// Idempotent: a
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

        // Attempt the OS credential store first. Regardless of availability,
        // remove the legacy plaintext from SQLite; a failed store requires user
        // re-entry and is surfaced in the warning below.
        let key_saved = secrets::set_api_key(ctx, domain, provider, &plaintext).is_ok();

        let now = Utc::now();
        sqlx::query(&format!(
            "UPDATE {table} SET \"{column}\" = ?, updated_at = ? \
             WHERE id = '1' AND workspace_id = ?"
        ))
        .bind(if key_saved {
            Some(KEYCHAIN_MARKER)
        } else {
            None
        })
        .bind(now)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;

        if key_saved {
            tracing::info!(
                provider,
                domain = ?domain,
                workspace_id = %ctx.tenant_id,
                "migrated a legacy plaintext API key from SQLite into the OS credential store"
            );
        } else {
            tracing::warn!(
                provider,
                domain = ?domain,
                workspace_id = %ctx.tenant_id,
                "removed legacy plaintext API key from SQLite; OS credential store unavailable"
            );
        }
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

    /// One-time startup sweep for the caller's authorized workspace. Move every
    /// legacy plaintext API key still stored in a `settings` /
    /// `transcript_settings` column into the OS credential store, then overwrite
    /// the column with [`KEYCHAIN_MARKER`], or clear it when the credential store
    /// is unavailable. Identity comes only from [`AuthContext`]; this method
    /// never enumerates or synthesizes identities for other workspaces. It is
    /// fully idempotent and offline, so it is safe to call on every startup and
    /// again after a future authenticated workspace switch.
    ///
    /// Returns the number of keys migrated (useful for logging/tests).
    pub async fn migrate_plaintext_keys_to_keychain(
        pool: &SqlitePool,
        ctx: &AuthContext,
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
            for (provider, column) in columns {
                let before: Option<String> = sqlx::query_scalar(&format!(
                    "SELECT \"{column}\" FROM {table} WHERE id = '1' AND workspace_id = ? LIMIT 1"
                ))
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(pool)
                .await?
                .flatten();

                if Self::is_legacy_plaintext(&before) {
                    Self::migrate_column_cell(pool, ctx, table, column, domain, provider).await?;
                    migrated += 1;
                }
            }

            if table == "settings"
                && Self::migrate_custom_openai_json_if_plaintext(pool, ctx).await?
            {
                migrated += 1;
            }
        }

        if migrated > 0 {
            tracing::info!(
                count = migrated,
                "startup: removed legacy plaintext API keys from SQLite"
            );
        }
        Ok(migrated)
    }
}
