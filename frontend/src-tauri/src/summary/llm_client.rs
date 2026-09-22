use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::info;

const REQUEST_TIMEOUT_DURATION: Duration = Duration::from_secs(300);

// Generic structure for OpenAI-compatible API chat messages
#[derive(Debug, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

// Generic structure for OpenAI-compatible API chat requests
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// OpenAI-compatible structured-output control (BACKLOG C1.4, ADR-0019
    /// decision 4). `None` — the legacy default at every pre-C1.4 call site —
    /// is skipped by serde, so the serialized request bytes are IDENTICAL to
    /// the pre-C1.4 wire format (pinned by a test in `summary::structured`).
    /// Claude requests use [`ClaudeRequest`] and never carry this field;
    /// BuiltInAI short-circuits before any HTTP body is built and ignores it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
}

// Generic structure for OpenAI-compatible API chat responses
#[derive(Deserialize, Debug)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
pub struct Choice {
    pub message: MessageContent,
}

#[derive(Deserialize, Debug)]
pub struct MessageContent {
    /// `null` from a reasoning model whose answer went into `reasoning_content`
    /// -- and `api_test_custom_openai_connection` accepts exactly that shape as a
    /// valid endpoint. Reading it as a bare `String` made every generation
    /// against such a server fail with "expected a string", after the app's own
    /// connection test had said the endpoint was fine.
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning_content: Option<String>,
}

impl MessageContent {
    /// The text to use: `content` when it says something, otherwise
    /// `reasoning_content`, otherwise nothing.
    pub fn text(&self) -> Option<&str> {
        self.content
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .or_else(|| {
                self.reasoning_content
                    .as_deref()
                    .map(str::trim)
                    .filter(|c| !c.is_empty())
            })
    }
}

/// The output ceiling sent with every Claude request. Anthropic's Messages API
/// requires `max_tokens` and stops generating at it. The structured path asks for
/// one JSON object holding every section of a whole meeting plus its action items;
/// 2048 -- the value hardcoded until v1.2.3 -- truncated that mid-object for any
/// meeting past a short one, the truncated JSON failed to parse, and with the legacy
/// fallback disabled the generation ended as `failed` on every retry. 8192 is
/// accepted by every Claude model this app offers (4.x and 3.5 Sonnet).
pub const CLAUDE_MAX_OUTPUT_TOKENS: u32 = 8192;

// Claude-specific request structure
#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<ChatMessage>,
}

// Claude-specific response structure
#[derive(Deserialize, Debug)]
pub struct ClaudeChatResponse {
    pub content: Vec<ClaudeChatContent>,
}

#[derive(Deserialize, Debug)]
pub struct ClaudeChatContent {
    pub text: String,
}

/// LLM Provider enumeration for multi-provider support
#[derive(Debug, Clone, PartialEq)]
pub enum LLMProvider {
    OpenAI,
    Claude,
    Groq,
    Ollama,
    OpenRouter,
    BuiltInAI,
    CustomOpenAI,
}

impl LLMProvider {
    /// Parse provider from string (case-insensitive)
    // Pre-dates C1.4; renaming to the `FromStr` trait would churn every call
    // site for zero behavior gain, so the inherent name is kept.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(Self::OpenAI),
            "claude" => Ok(Self::Claude),
            "groq" => Ok(Self::Groq),
            "ollama" => Ok(Self::Ollama),
            "openrouter" => Ok(Self::OpenRouter),
            "builtin-ai" | "local-llama" | "localllama" => Ok(Self::BuiltInAI),
            "custom-openai" => Ok(Self::CustomOpenAI),
            _ => Err(format!("Unsupported LLM provider: {}", s)),
        }
    }
}

/// Generates a summary using the specified LLM provider
///
/// # Arguments
/// * `client` - Reqwest HTTP client (reused for performance)
/// * `provider` - The LLM provider to use
/// * `model_name` - The specific model to use (e.g., "gpt-4", "claude-3-opus")
/// * `api_key` - API key for the provider (not needed for Ollama)
/// * `system_prompt` - System instructions for the LLM
/// * `user_prompt` - User query/content to process
/// * `ollama_endpoint` - Optional custom Ollama endpoint (defaults to localhost:11434)
/// * `custom_openai_endpoint` - Optional custom OpenAI-compatible endpoint
/// * `max_tokens` - Optional max tokens (for CustomOpenAI provider)
/// * `temperature` - Optional temperature (for CustomOpenAI provider)
/// * `top_p` - Optional top_p (for CustomOpenAI provider)
/// * `app_data_dir` - Optional app data directory (for BuiltInAI provider)
/// * `cancellation_token` - Optional token to cancel the request
///
/// # Returns
/// The generated summary text or an error message
#[allow(clippy::too_many_arguments)]
pub async fn generate_summary(
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    custom_openai_endpoint: Option<&str>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    app_data_dir: Option<&PathBuf>,
    cancellation_token: Option<&CancellationToken>,
) -> Result<String, String> {
    // Legacy entry point — delegates with `response_format: None`, which keeps
    // the serialized request bytes identical to the pre-C1.4 wire format (the
    // field is skipped by serde). Every pre-C1.4 call site goes through here.
    generate_summary_with_response_format(
        client,
        provider,
        model_name,
        api_key,
        system_prompt,
        user_prompt,
        ollama_endpoint,
        custom_openai_endpoint,
        max_tokens,
        temperature,
        top_p,
        app_data_dir,
        cancellation_token,
        None,
    )
    .await
}

/// [`generate_summary`] plus an optional OpenAI-compatible `response_format`
/// value (BACKLOG C1.4, ADR-0019 decision 4 — structured output modes).
///
/// * `response_format: None` — behavior and wire bytes identical to
///   [`generate_summary`] (the legacy path).
/// * `Some(...)` — serialized into the OpenAI-compatible request body (e.g.
///   `{"type":"json_object"}` or a strict `{"type":"json_schema",...}`).
///   **Claude ignores it** (its request shape has no such field — prompt-only
///   JSON mode) and **BuiltInAI ignores it** (local sidecar short-circuit; no
///   constrained decoding in llama-helper).
#[allow(clippy::too_many_arguments)]
pub async fn generate_summary_with_response_format(
    client: &Client,
    provider: &LLMProvider,
    model_name: &str,
    api_key: &str,
    system_prompt: &str,
    user_prompt: &str,
    ollama_endpoint: Option<&str>,
    custom_openai_endpoint: Option<&str>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    app_data_dir: Option<&PathBuf>,
    cancellation_token: Option<&CancellationToken>,
    response_format: Option<serde_json::Value>,
) -> Result<String, String> {
    // Check if cancelled before starting
    if let Some(token) = cancellation_token {
        if token.is_cancelled() {
            return Err("Summary generation was cancelled".to_string());
        }
    }

    // Handle BuiltInAI provider separately (uses local sidecar, no HTTP API)
    if provider == &LLMProvider::BuiltInAI {
        let app_data_dir = app_data_dir
            .ok_or_else(|| "app_data_dir is required for BuiltInAI provider".to_string())?;

        return crate::summary::summary_engine::generate_with_builtin(
            app_data_dir,
            model_name,
            system_prompt,
            user_prompt,
            cancellation_token,
        )
        .await
        .map_err(|e| e.to_string());
    }

    let (api_url, mut headers) = match provider {
        LLMProvider::OpenAI => (
            "https://api.openai.com/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Groq => (
            "https://api.groq.com/openai/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::OpenRouter => (
            "https://openrouter.ai/api/v1/chat/completions".to_string(),
            header::HeaderMap::new(),
        ),
        LLMProvider::Ollama => {
            let host = crate::summary::validate_llm_endpoint(
                ollama_endpoint.unwrap_or("http://localhost:11434"),
            )?;
            (
                format!("{}/v1/chat/completions", host),
                header::HeaderMap::new(),
            )
        }
        LLMProvider::CustomOpenAI => {
            let endpoint = custom_openai_endpoint
                .ok_or_else(|| "Custom OpenAI endpoint not configured".to_string())?;
            let endpoint = crate::summary::validate_llm_endpoint(endpoint)?;
            (
                format!("{}/chat/completions", endpoint),
                header::HeaderMap::new(),
            )
        }
        LLMProvider::Claude => {
            let mut header_map = header::HeaderMap::new();
            header_map.insert(
                "x-api-key",
                api_key
                    .parse()
                    .map_err(|_| "Invalid API key format".to_string())?,
            );
            header_map.insert(
                "anthropic-version",
                "2023-06-01"
                    .parse()
                    .map_err(|_| "Invalid anthropic version".to_string())?,
            );
            (
                "https://api.anthropic.com/v1/messages".to_string(),
                header_map,
            )
        }
        LLMProvider::BuiltInAI => {
            // This case is handled earlier with early returns
            unreachable!("BuiltInAI is handled before this match statement")
        }
    };

    // Add authorization header for non-Claude providers
    if provider != &LLMProvider::Claude && !api_key.trim().is_empty() {
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {}", api_key)
                .parse()
                .map_err(|_| "Invalid authorization header".to_string())?,
        );
    }
    headers.insert(
        header::CONTENT_TYPE,
        "application/json"
            .parse()
            .map_err(|_| "Invalid content type".to_string())?,
    );

    // Build request body based on provider
    let request_body = if provider != &LLMProvider::Claude {
        // For CustomOpenAI, apply optional parameters if provided
        let (max_tokens_val, temperature_val, top_p_val) = if provider == &LLMProvider::CustomOpenAI
        {
            (max_tokens, temperature, top_p)
        } else {
            (None, None, None)
        };

        serde_json::json!(ChatRequest {
            model: model_name.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                }
            ],
            max_tokens: max_tokens_val,
            temperature: temperature_val,
            top_p: top_p_val,
            response_format,
        })
    } else {
        serde_json::json!(ClaudeRequest {
            system: system_prompt.to_string(),
            model: model_name.to_string(),
            max_tokens: CLAUDE_MAX_OUTPUT_TOKENS,
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }]
        })
    };

    info!("LLM request started for {}", provider_name(provider));

    // Send request with timeout and cancellation support
    // Custom endpoints can redirect across hosts or protocols. Disabling
    // redirects prevents credentials and transcript content from being
    // forwarded to a destination the user did not configure.
    let restricted_client;
    let request_client = if matches!(provider, LLMProvider::Ollama | LLMProvider::CustomOpenAI) {
        restricted_client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Failed to initialize the LLM HTTP client".to_string())?;
        &restricted_client
    } else {
        client
    };
    let request_future = request_client
        .post(api_url)
        .headers(headers)
        .json(&request_body)
        .timeout(REQUEST_TIMEOUT_DURATION)
        .send();

    // Use tokio::select to race between cancellation and request completion
    let response = if let Some(token) = cancellation_token {
        tokio::select! {
            result = request_future => {
                result.map_err(|e| {
                    if e.is_timeout() {
                        format!(
                            "LLM request timed out after {} seconds",
                            REQUEST_TIMEOUT_DURATION.as_secs()
                        )
                    } else if e.is_connect() {
                        "Could not connect to the configured LLM provider".to_string()
                    } else {
                        "Failed to send request to the configured LLM provider".to_string()
                    }
                })?
            }
            _ = token.cancelled() => {
                return Err("Summary generation was cancelled".to_string());
            }
        }
    } else {
        request_future.await.map_err(|e| {
            if e.is_timeout() {
                format!(
                    "LLM request timed out after {} seconds",
                    REQUEST_TIMEOUT_DURATION.as_secs()
                )
            } else if e.is_connect() {
                "Could not connect to the configured LLM provider".to_string()
            } else {
                "Failed to send request to the configured LLM provider".to_string()
            }
        })?
    };

    if !response.status().is_success() {
        return Err(format!(
            "LLM API request failed with HTTP status {}",
            response.status()
        ));
    }

    // Parse response based on provider
    if provider == &LLMProvider::Claude {
        let chat_response = response
            .json::<ClaudeChatResponse>()
            .await
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        info!("🐞 LLM Response received from Claude");

        let content = chat_response
            .content
            .first()
            .ok_or("No content in LLM response")?
            .text
            .trim();
        Ok(content.to_string())
    } else {
        let chat_response = response
            .json::<ChatResponse>()
            .await
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        info!("🐞 LLM Response received from {}", provider_name(provider));

        let content = chat_response
            .choices
            .first()
            .ok_or("No content in LLM response")?
            .message
            .text()
            .ok_or("No content in LLM response")?;
        Ok(content.to_string())
    }
}

#[cfg(test)]
mod response_shape_tests {
    use super::*;

    #[test]
    fn a_reasoning_model_reply_with_null_content_is_still_readable() {
        let raw =
            r#"{"choices":[{"message":{"content":null,"reasoning_content":"  the answer  "}}]}"#;
        let parsed: ChatResponse = serde_json::from_str(raw).expect("must parse");
        assert_eq!(parsed.choices[0].message.text(), Some("the answer"));
    }

    #[test]
    fn content_wins_over_reasoning_when_both_are_present() {
        let raw = r#"{"choices":[{"message":{"content":"final","reasoning_content":"draft"}}]}"#;
        let parsed: ChatResponse = serde_json::from_str(raw).expect("must parse");
        assert_eq!(parsed.choices[0].message.text(), Some("final"));
    }

    #[test]
    fn the_plain_openai_shape_still_parses() {
        let raw = r#"{"choices":[{"message":{"role":"assistant","content":"hello"}}]}"#;
        let parsed: ChatResponse = serde_json::from_str(raw).expect("must parse");
        assert_eq!(parsed.choices[0].message.text(), Some("hello"));
    }

    #[test]
    fn an_empty_reply_is_no_content() {
        let raw = r#"{"choices":[{"message":{"content":"   "}}]}"#;
        let parsed: ChatResponse = serde_json::from_str(raw).expect("must parse");
        assert_eq!(parsed.choices[0].message.text(), None);
    }
}

/// Helper function to get provider name for logging
fn provider_name(provider: &LLMProvider) -> &str {
    match provider {
        LLMProvider::OpenAI => "OpenAI",
        LLMProvider::Claude => "Claude",
        LLMProvider::Groq => "Groq",
        LLMProvider::Ollama => "Ollama",
        LLMProvider::BuiltInAI => "Built-in AI",
        LLMProvider::OpenRouter => "OpenRouter",
        LLMProvider::CustomOpenAI => "Custom OpenAI",
    }
}

#[cfg(test)]
mod endpoint_boundary_tests {
    use super::*;

    #[tokio::test]
    async fn legacy_remote_http_custom_endpoint_fails_before_network_send() {
        let result = generate_summary_with_response_format(
            &Client::new(),
            &LLMProvider::CustomOpenAI,
            "test-model",
            "test-key",
            "system",
            "user",
            None,
            Some("http://llm.example.com/v1"),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        assert!(result
            .expect_err("remote plaintext endpoint must be rejected")
            .contains("must use HTTPS"));
    }

    #[tokio::test]
    async fn legacy_remote_http_ollama_endpoint_fails_before_network_send() {
        let result = generate_summary_with_response_format(
            &Client::new(),
            &LLMProvider::Ollama,
            "test-model",
            "",
            "system",
            "user",
            Some("http://localhost.evil.example:11434"),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await;

        assert!(result
            .expect_err("fake loopback hostname must be rejected")
            .contains("must use HTTPS"));
    }
}
