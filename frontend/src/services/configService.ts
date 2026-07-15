/**
 * Configuration Service
 *
 * Handles all configuration-related Tauri backend calls.
 * Pure 1-to-1 wrapper - no error handling changes, exact same behavior as direct invoke calls.
 */

import { invoke } from '@tauri-apps/api/core';
import { TranscriptModelProps } from '@/components/TranscriptSettings';

export interface ModelConfig {
  provider: 'ollama' | 'groq' | 'claude' | 'openrouter' | 'openai' | 'builtin-ai' | 'custom-openai';
  model: string;
  whisperModel: string;
  /**
   * @deprecated Use providerApiKeys from ConfigContext instead.
   * This field may contain stale data when provider changes without saving.
   */
  apiKey?: string | null;
  hasApiKey?: boolean;
  ollamaEndpoint?: string | null;
  // Custom OpenAI fields (only populated when provider is 'custom-openai')
  customOpenAIEndpoint?: string | null;
  customOpenAIModel?: string | null;
  customOpenAIApiKey?: string | null;
  customOpenAIHasApiKey?: boolean;
  maxTokens?: number | null;
  temperature?: number | null;
  topP?: number | null;
}

export interface CustomOpenAIConfig {
  endpoint: string;
  apiKey: string | null;
  hasApiKey?: boolean;
  model: string;
  maxTokens: number | null;
  temperature: number | null;
  topP: number | null;
}

export interface RecordingPreferences {
  preferred_mic_device: string | null;
  preferred_system_device: string | null;
}

/**
 * Per-workspace opt-in PII/keyword redaction policy (BACKLOG C6).
 *
 * OFF by default (`enabled: false`) — existing local data and flows are unchanged
 * unless the user turns this on. When enabled, transcript text is scrubbed both
 * before it is persisted and before it is sent to a summary LLM provider.
 *
 * NOTE: the field names are snake_case to match the Rust `RedactionConfig` struct
 * (which has no serde rename); send them exactly as declared here.
 */
export interface RedactionConfig {
  /** Master switch. When false, redaction is a verbatim no-op. */
  enabled: boolean;
  /** Apply built-in PII patterns (email/phone/card/IBAN/TR TC Kimlik No). */
  use_default_patterns: boolean;
  /** Extra case-insensitive literal terms to redact (each -> `[REDACTED]`). */
  custom_terms: string[];
}

/**
 * Configuration Service
 * Singleton service for managing app configuration
 */
export class ConfigService {
  /**
   * Get saved transcript model configuration
   * @returns Promise with { provider, model, apiKey }
   */
  async getTranscriptConfig(): Promise<TranscriptModelProps> {
    return invoke<TranscriptModelProps>('api_get_transcript_config');
  }

  /**
   * Get saved summary model configuration
   * @returns Promise with { provider, model, whisperModel }
   */
  async getModelConfig(): Promise<ModelConfig> {
    return invoke<ModelConfig>('api_get_model_config');
  }

  /**
   * Get saved audio device preferences
   * @returns Promise with { preferred_mic_device, preferred_system_device }
   */
  async getRecordingPreferences(): Promise<RecordingPreferences> {
    return invoke<RecordingPreferences>('get_recording_preferences');
  }

  /**
   * Get custom OpenAI configuration
   * @returns Promise with CustomOpenAIConfig or null if not configured
   */
  async getCustomOpenAIConfig(): Promise<CustomOpenAIConfig | null> {
    return invoke<CustomOpenAIConfig | null>('api_get_custom_openai_config');
  }

  /**
   * Save custom OpenAI configuration
   * @param config - CustomOpenAIConfig to save
   * @returns Promise with result status
   */
  async saveCustomOpenAIConfig(config: CustomOpenAIConfig): Promise<{ status: string; message: string }> {
    return invoke<{ status: string; message: string }>('api_save_custom_openai_config', {
      endpoint: config.endpoint,
      apiKey: config.apiKey,
      model: config.model,
      maxTokens: config.maxTokens,
      temperature: config.temperature,
      topP: config.topP,
    });
  }

  /**
   * Test custom OpenAI connection
   * @param endpoint - API endpoint URL
   * @param apiKey - Optional API key
   * @param model - Model name
   * @returns Promise with test result
   */
  async testCustomOpenAIConnection(
    endpoint: string,
    apiKey: string | null,
    model: string
  ): Promise<{ status: string; message: string; http_status?: number }> {
    return invoke<{ status: string; message: string; http_status?: number }>('api_test_custom_openai_connection', {
      endpoint,
      apiKey,
      model,
    });
  }

  /**
   * Get the current workspace's redaction policy (BACKLOG C6).
   * Returns the disabled default when none has been configured.
   * @returns Promise with the RedactionConfig
   */
  async getRedactionConfig(): Promise<RedactionConfig> {
    return invoke<RedactionConfig>('api_get_redaction_config');
  }

  /**
   * Save the current workspace's redaction policy (BACKLOG C6).
   * Enabling is opt-in; when enabled, transcript text is redacted before
   * persistence and before it reaches a summary LLM provider.
   * @param config - RedactionConfig to persist (snake_case keys, see interface)
   * @returns Promise with result status
   */
  async setRedactionConfig(config: RedactionConfig): Promise<{ status: string; message: string }> {
    return invoke<{ status: string; message: string }>('api_set_redaction_config', { config });
  }
}

// Export singleton instance
export const configService = new ConfigService();
