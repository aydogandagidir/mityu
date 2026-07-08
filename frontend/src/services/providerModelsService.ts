/**
 * Provider Models Service
 *
 * Typed wrappers over the Tauri commands that list/manage LLM provider models
 * and read stored provider credentials.
 * Pure 1-to-1 wrappers over invoke() - no behavior changes vs. a direct invoke call.
 *
 * These command names and their result shapes were previously duplicated across
 * components, contexts and hooks (`get_ollama_models` alone had five call sites,
 * `api_get_api_key` six), and `OllamaModel` was declared twice - once privately in
 * ModelSettingsModal and once exported from ConfigContext. This module is the
 * single definition of both the commands and their result types.
 *
 * NOTE: distinct from `configService`, which owns the *saved* summary/transcript
 * model configuration (`api_get_model_config`, custom-OpenAI config, redaction).
 * This module owns the *available* models fetched from each provider.
 */

import { invoke } from '@tauri-apps/api/core';

/** A model reported by a local Ollama server. */
export interface OllamaModel {
  name: string;
  id: string;
  size: string;
  modified: string;
}

/** A model listed by the OpenRouter catalogue. */
export interface OpenRouterModel {
  id: string;
  name: string;
  context_length?: number;
  prompt_price?: string;
  completion_price?: string;
}

/** A model listed by the OpenAI API. */
export interface OpenAIModel {
  id: string;
}

/** A model listed by the Anthropic API. */
export interface AnthropicModel {
  id: string;
  display_name?: string;
}

/** A model listed by the Groq API. */
export interface GroqModel {
  id: string;
  owned_by?: string;
}

/**
 * List models available on the user's Ollama server.
 * `endpoint` is `null` to use the backend's default endpoint.
 */
export async function getOllamaModels(endpoint: string | null): Promise<OllamaModel[]> {
  return invoke<OllamaModel[]>('get_ollama_models', { endpoint });
}

/** Pull (download) a model onto the user's Ollama server. */
export async function pullOllamaModel(modelName: string, endpoint: string | null): Promise<void> {
  await invoke('pull_ollama_model', { modelName, endpoint });
}

/** Delete a model from the user's Ollama server. */
export async function deleteOllamaModel(modelName: string, endpoint: string | null): Promise<void> {
  await invoke('delete_ollama_model', { modelName, endpoint });
}

/** List models from the OpenRouter catalogue (no credentials required). */
export async function getOpenRouterModels(): Promise<OpenRouterModel[]> {
  return invoke<OpenRouterModel[]>('get_openrouter_models');
}

/** List models available to the given OpenAI API key. */
export async function getOpenAIModels(apiKey: string): Promise<OpenAIModel[]> {
  return invoke<OpenAIModel[]>('get_openai_models', { apiKey });
}

/** List models available to the given Anthropic API key. */
export async function getAnthropicModels(apiKey: string): Promise<AnthropicModel[]> {
  return invoke<AnthropicModel[]>('get_anthropic_models', { apiKey });
}

/** List models available to the given Groq API key. */
export async function getGroqModels(apiKey: string): Promise<GroqModel[]> {
  return invoke<GroqModel[]>('get_groq_models', { apiKey });
}

/**
 * Read the stored API key for a provider.
 *
 * The key itself lives in the OS keychain (ADR-0011); this returns it for the
 * settings UI to display/reuse. Rejects when no key is stored - existing callers
 * either `catch` and fall back to `null`, or let it propagate.
 */
export async function getApiKey(provider: string): Promise<string> {
  return invoke<string>('api_get_api_key', { provider });
}

/** Whether summaries are auto-generated when a recording finishes. */
export async function getAutoGenerateSetting(): Promise<boolean> {
  return invoke<boolean>('api_get_auto_generate_setting');
}
