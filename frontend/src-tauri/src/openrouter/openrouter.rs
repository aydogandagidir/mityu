use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::command;

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenRouterModel {
    pub id: String,
    pub name: String,
    pub context_length: Option<u32>,
    pub prompt_price: Option<String>,
    pub completion_price: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterApiModel {
    id: String,
    name: Option<String>,
    context_length: Option<u32>,
    #[serde(default)]
    top_provider: Option<TopProvider>,
    #[serde(default)]
    pricing: Option<Pricing>,
}

#[derive(Debug, Deserialize, Default)]
struct TopProvider {
    context_length: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct Pricing {
    prompt: Option<String>,
    completion: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    data: Vec<OpenRouterApiModel>,
}

/// How long the model list may take before the settings page gives up on it.
/// Matches the OpenAI, Groq and Anthropic listers.
const MODEL_LIST_TIMEOUT: Duration = Duration::from_secs(5);

/// Lists OpenRouter's models.
///
/// `async`, on purpose. This was a synchronous command using `reqwest::blocking`
/// with no timeout, and Tauri runs a synchronous command on the main thread: with
/// OpenRouter selected and no network, opening Settings -> Model froze the whole
/// window and every other `invoke` until the OS gave up on the connection -- tens
/// of seconds on Windows, indefinitely against a firewall that drops packets.
#[command]
pub async fn get_openrouter_models() -> Result<Vec<OpenRouterModel>, String> {
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Could not create the OpenRouter client: {}", e))?;
    let response = client
        .get("https://openrouter.ai/api/v1/models")
        .timeout(MODEL_LIST_TIMEOUT)
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                format!(
                    "OpenRouter did not answer within {} seconds",
                    MODEL_LIST_TIMEOUT.as_secs()
                )
            } else {
                format!("Failed to make HTTP request: {}", e)
            }
        })?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP request failed with status: {}",
            response.status()
        ));
    }

    let api_response: OpenRouterResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

    let models = api_response
        .data
        .into_iter()
        .map(|m| OpenRouterModel {
            id: m.id,
            name: m.name.unwrap_or_else(|| "Unknown".to_string()),
            context_length: m
                .top_provider
                .as_ref()
                .and_then(|tp| tp.context_length)
                .or(m.context_length),
            prompt_price: m.pricing.as_ref().and_then(|p| p.prompt.clone()),
            completion_price: m.pricing.as_ref().and_then(|p| p.completion.clone()),
        })
        .collect();

    Ok(models)
}
