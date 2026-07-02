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

use crate::context::AuthContext;
use crate::database::models::{Setting, TranscriptSetting};
use crate::summary::CustomOpenAIConfig;
use chrono::Utc;
use sqlx::SqlitePool;

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

        let api_key_column = match provider {
            "openai" => "openaiApiKey",
            "claude" => "anthropicApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(()), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

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
            .bind(api_key)
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

        let api_key_column = match provider {
            "openai" => "openaiApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "claude" => "anthropicApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(None), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        let query = format!(
            "SELECT {} FROM settings WHERE id = '1' AND workspace_id = ? LIMIT 1",
            api_key_column
        );
        let api_key = sqlx::query_scalar(&query)
            .bind(ctx.tenant_id.as_str())
            .fetch_optional(pool)
            .await?;
        Ok(api_key)
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

    pub async fn save_transcript_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
        api_key: &str,
    ) -> std::result::Result<(), sqlx::Error> {
        let api_key_column = match provider {
            "localWhisper" => "whisperApiKey",
            "parakeet" => return Ok(()), // Parakeet doesn't need an API key, return early
            "deepgram" => "deepgramApiKey",
            "elevenLabs" => "elevenLabsApiKey",
            "groq" => "groqApiKey",
            "openai" => "openaiApiKey",
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

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
            .bind(api_key)
            .execute(pool)
            .await?;

        Ok(())
    }

    pub async fn get_transcript_api_key(
        pool: &SqlitePool,
        ctx: &AuthContext,
        provider: &str,
    ) -> std::result::Result<Option<String>, sqlx::Error> {
        let api_key_column = match provider {
            "localWhisper" => "whisperApiKey",
            "parakeet" => return Ok(None), // Parakeet doesn't need an API key
            "deepgram" => "deepgramApiKey",
            "elevenLabs" => "elevenLabsApiKey",
            "groq" => "groqApiKey",
            "openai" => "openaiApiKey",
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

        let query = format!(
            "SELECT {} FROM transcript_settings WHERE id = '1' AND workspace_id = ? LIMIT 1",
            api_key_column
        );
        let api_key = sqlx::query_scalar(&query)
            .bind(ctx.tenant_id.as_str())
            .fetch_optional(pool)
            .await?;
        Ok(api_key)
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

        let api_key_column = match provider {
            "openai" => "openaiApiKey",
            "ollama" => "ollamaApiKey",
            "groq" => "groqApiKey",
            "claude" => "anthropicApiKey",
            "openrouter" => "openRouterApiKey",
            "builtin-ai" => return Ok(()), // No API key needed
            _ => {
                return Err(sqlx::Error::Protocol(
                    format!("Invalid provider: {}", provider).into(),
                ))
            }
        };

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
}
