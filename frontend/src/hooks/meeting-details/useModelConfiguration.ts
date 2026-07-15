import { useState, useEffect, useCallback } from 'react';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { invoke as invokeTauri } from '@tauri-apps/api/core';
import { configService } from '@/services/configService';
import { toast } from 'sonner';
import Analytics from '@/lib/analytics';

interface UseModelConfigurationProps {
  serverAddress: string | null;
}

export function useModelConfiguration({ serverAddress }: UseModelConfigurationProps) {
  // Note: No hardcoded defaults - DB is the source of truth
  const [modelConfig, setModelConfig] = useState<ModelConfig>({
    provider: 'ollama',
    model: '', // Empty until loaded from DB
    whisperModel: 'large-v3'
  });
  const [isLoading, setIsLoading] = useState(true);
  const [, setError] = useState<string>('');

  // Fetch model configuration on mount and when serverAddress changes
  useEffect(() => {
    const fetchModelConfig = async () => {
      setIsLoading(true);
      try {
        console.log('🔄 Fetching model configuration from database...');
        const data = await configService.getModelConfig() as any;
        if (data && data.provider !== null) {
          console.log('Model configuration loaded');
          // Fetch custom OpenAI config if provider is custom-openai
          if (data.provider === 'custom-openai') {
            try {
              const customConfig = await configService.getCustomOpenAIConfig() as any;
              if (customConfig) {
                data.customOpenAIDisplayName = customConfig.displayName || null;
                data.customOpenAIEndpoint = customConfig.endpoint || null;
                data.customOpenAIModel = customConfig.model || null;
                data.customOpenAIApiKey = null;
                data.customOpenAIHasApiKey = customConfig.hasApiKey || false;
                data.maxTokens = customConfig.maxTokens || null;
                data.temperature = customConfig.temperature || null;
                data.topP = customConfig.topP || null;
                // For custom-openai, model field should match customOpenAIModel
                data.model = customConfig.model || data.model;
                console.log('Custom OpenAI configuration loaded');
              }
            } catch {
              console.error('Failed to fetch custom OpenAI config');
            }
          }

          setModelConfig(data);
        } else {
          console.warn('⚠️ No model config found in database, using defaults');
        }
      } catch {
        console.error('Failed to fetch model config');
      } finally {
        setIsLoading(false);
        console.log('✅ Model configuration loading complete');
      }
    };

    fetchModelConfig();
  }, [serverAddress]);

  // Listen for model config updates from other components
  useEffect(() => {
    const setupListener = async () => {
      const { listen } = await import('@tauri-apps/api/event');
      const unlisten = await listen<ModelConfig>('model-config-updated', (event) => {
        console.log('Meeting details received model-config-updated event');
        setModelConfig(event.payload);
      });

      return unlisten;
    };

    let cleanup: (() => void) | undefined;
    setupListener().then(fn => cleanup = fn);

    return () => {
      cleanup?.();
    };
  }, []);

  // Save model configuration
  const handleSaveModelConfig = useCallback(async (updatedConfig?: ModelConfig) => {
    try {
      const configToSave = updatedConfig || modelConfig;
      const payload = {
        provider: configToSave.provider,
        model: configToSave.model,
        whisperModel: configToSave.whisperModel,
        apiKey: configToSave.apiKey ?? null,
        ollamaEndpoint: configToSave.ollamaEndpoint ?? null
      };
      console.log('Saving model configuration');

      // Track model configuration change
      if (updatedConfig && (
        updatedConfig.provider !== modelConfig.provider ||
        updatedConfig.model !== modelConfig.model
      )) {
        await Analytics.trackModelChanged(
          modelConfig.provider,
          modelConfig.model,
          updatedConfig.provider,
          updatedConfig.model
        );
      }

      await invokeTauri('api_save_model_config', {
        provider: payload.provider,
        model: payload.model,
        whisperModel: payload.whisperModel,
        apiKey: payload.apiKey,
        ollamaEndpoint: payload.ollamaEndpoint,
      });

      console.log('Save model config success');
      setModelConfig(payload);

      // Emit event to sync other components
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', payload);

      toast.success("Summary settings Saved successfully");

      await Analytics.trackSettingsChanged('model_config');
    } catch (error) {
      console.error('Failed to save model config');
      toast.error("Failed to save summary settings", { description: String(error) });
      if (error instanceof Error) {
        setError(error.message);
      } else {
        setError('Failed to save model config: Unknown error');
      }
    }
  }, [modelConfig]);

  return {
    modelConfig,
    setModelConfig,
    handleSaveModelConfig,
    isLoading,
  };
}
