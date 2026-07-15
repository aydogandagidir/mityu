/// Summary module - handles all meeting summary generation functionality
///
/// This module contains:
/// - LLM client for communicating with various AI providers (OpenAI, Claude, Groq, Ollama, OpenRouter, CustomOpenAI)
/// - Processor for chunking transcripts and generating summaries
/// - Service layer for orchestrating summary generation
/// - Templates for structured meeting summary generation
/// - Tauri commands for frontend integration
use serde::{Deserialize, Serialize};

/// Custom OpenAI-compatible endpoint configuration. Non-secret fields are
/// stored as JSON; `api_key` is hydrated from the OS credential store and must
/// never be serialized into SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomOpenAIConfig {
    /// Base URL of the OpenAI-compatible API endpoint (e.g., "http://localhost:8000/v1")
    pub endpoint: String,
    /// API key for authentication (optional if server doesn't require it)
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    /// Model identifier to use (e.g., "gpt-4", "llama-3-70b", "mistral-7b")
    pub model: String,
    /// Maximum tokens for completion (optional)
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<i32>,
    /// Temperature parameter (0.0-2.0, optional)
    pub temperature: Option<f32>,
    /// Top-P sampling parameter (0.0-1.0, optional)
    #[serde(rename = "topP")]
    pub top_p: Option<f32>,
}

/// Validate and normalize an LLM base URL. Remote hosts must use HTTPS;
/// plaintext HTTP is allowed only for loopback development servers.
/// Credentials, query strings, and fragments are rejected so secrets cannot be
/// smuggled into persisted configuration or diagnostic output.
pub fn validate_llm_endpoint(endpoint: &str) -> Result<String, String> {
    let parsed = url::Url::parse(endpoint.trim())
        .map_err(|_| "Endpoint must be a valid absolute URL".to_string())?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Endpoint URL must not include user credentials".to_string());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("Endpoint URL must not include a query string or fragment".to_string());
    }

    let is_loopback = match parsed.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    match parsed.scheme() {
        "https" => {}
        "http" if is_loopback => {}
        "http" => {
            return Err(
                "Remote LLM endpoints must use HTTPS; HTTP is allowed only on loopback".to_string(),
            )
        }
        _ => return Err("Endpoint scheme must be HTTPS or loopback HTTP".to_string()),
    }

    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

/// Backward-compatible, provider-specific entry point used by the Tauri API.
pub fn validate_custom_openai_endpoint(endpoint: &str) -> Result<String, String> {
    validate_llm_endpoint(endpoint)
}

#[cfg(test)]
mod endpoint_tests {
    use super::{validate_custom_openai_endpoint, validate_llm_endpoint};

    #[test]
    fn custom_endpoint_requires_tls_except_loopback() {
        assert_eq!(
            validate_custom_openai_endpoint("http://localhost:8000/v1").unwrap(),
            "http://localhost:8000/v1"
        );
        assert!(validate_custom_openai_endpoint("http://127.0.0.1:8000/v1").is_ok());
        assert!(validate_custom_openai_endpoint("http://[::1]:8000/v1").is_ok());
        assert!(validate_custom_openai_endpoint("https://llm.example.com/v1").is_ok());
        assert!(validate_custom_openai_endpoint("http://llm.example.com/v1").is_err());
    }

    #[test]
    fn custom_endpoint_rejects_embedded_secret_channels() {
        assert!(validate_custom_openai_endpoint("https://user:pass@llm.example/v1").is_err());
        assert!(validate_custom_openai_endpoint("https://llm.example/v1?token=secret").is_err());
        assert!(validate_custom_openai_endpoint("https://llm.example/v1#secret").is_err());
    }

    #[test]
    fn generic_llm_endpoint_rejects_remote_plaintext_and_fake_loopback_hosts() {
        assert!(validate_llm_endpoint("http://ollama.example.com:11434").is_err());
        assert!(validate_llm_endpoint("http://localhost.evil.example:11434").is_err());
        assert!(validate_llm_endpoint("http://127.0.0.1.evil.example:11434").is_err());
        assert!(validate_llm_endpoint("https://ollama.example.com:11434").is_ok());
    }
}

pub mod commands;
pub mod draft;
pub(crate) mod language_detection;
pub mod llm_client;
pub(crate) mod metadata;
pub mod processor;
pub mod service;
pub mod structured;
pub mod summary_engine;
pub mod template_commands;
pub mod templates;

// Re-export Tauri commands (with their generated __cmd__ variants)
pub use commands::{
    __cmd__api_approve_action_item, __cmd__api_approve_summary, __cmd__api_approve_summary_block,
    __cmd__api_cancel_summary, __cmd__api_detect_transcript_summary_language,
    __cmd__api_edit_action_item, __cmd__api_edit_summary_block,
    __cmd__api_get_meeting_detected_summary_language, __cmd__api_get_meeting_summary_language,
    __cmd__api_get_summary, __cmd__api_get_summary_draft, __cmd__api_list_approved_action_items,
    __cmd__api_process_transcript, __cmd__api_reject_action_item, __cmd__api_reject_summary_block,
    __cmd__api_restore_action_item, __cmd__api_restore_summary_block,
    __cmd__api_save_meeting_detected_summary_language, __cmd__api_save_meeting_summary,
    __cmd__api_save_meeting_summary_language, __tauri_command_name_api_approve_action_item,
    __tauri_command_name_api_approve_summary, __tauri_command_name_api_approve_summary_block,
    __tauri_command_name_api_cancel_summary,
    __tauri_command_name_api_detect_transcript_summary_language,
    __tauri_command_name_api_edit_action_item, __tauri_command_name_api_edit_summary_block,
    __tauri_command_name_api_get_meeting_detected_summary_language,
    __tauri_command_name_api_get_meeting_summary_language, __tauri_command_name_api_get_summary,
    __tauri_command_name_api_get_summary_draft,
    __tauri_command_name_api_list_approved_action_items,
    __tauri_command_name_api_process_transcript, __tauri_command_name_api_reject_action_item,
    __tauri_command_name_api_reject_summary_block, __tauri_command_name_api_restore_action_item,
    __tauri_command_name_api_restore_summary_block,
    __tauri_command_name_api_save_meeting_detected_summary_language,
    __tauri_command_name_api_save_meeting_summary,
    __tauri_command_name_api_save_meeting_summary_language, api_approve_action_item,
    api_approve_summary, api_approve_summary_block, api_cancel_summary,
    api_detect_transcript_summary_language, api_edit_action_item, api_edit_summary_block,
    api_get_meeting_detected_summary_language, api_get_meeting_summary_language, api_get_summary,
    api_get_summary_draft, api_list_approved_action_items, api_process_transcript,
    api_reject_action_item, api_reject_summary_block, api_restore_action_item,
    api_restore_summary_block, api_save_meeting_detected_summary_language,
    api_save_meeting_summary, api_save_meeting_summary_language,
};

// Re-export template commands
pub use template_commands::{
    __cmd__api_get_template_details, __cmd__api_list_templates, __cmd__api_validate_template,
    __tauri_command_name_api_get_template_details, __tauri_command_name_api_list_templates,
    __tauri_command_name_api_validate_template, api_get_template_details, api_list_templates,
    api_validate_template,
};

// Re-export commonly used items
pub use llm_client::LLMProvider;
pub use processor::{
    chunk_text, clean_llm_markdown_output, extract_meeting_name_from_markdown,
    generate_meeting_summary, rough_token_count,
};
pub use service::SummaryService;
