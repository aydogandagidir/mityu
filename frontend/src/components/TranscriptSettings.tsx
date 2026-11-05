import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { focusRing } from './ui/focus-ring';
import { Eye, EyeOff, Lock, Unlock } from 'lucide-react';
import { ModelManager } from './WhisperModelManager';
import { ParakeetModelManager } from './ParakeetModelManager';
import { SettingsCard } from './settings/SettingsCard';
import { SwitchRow } from './settings/SwitchRow';
import {
    DEFAULT_NEMOTRON_ENDPOINT,
    DEFAULT_NEMOTRON_MODEL,
} from '@/constants/modelDefaults';

export interface NemotronSettingsPayload {
    model: string;
    endpoint: string;
    speaker_attribution: boolean;
    inline_pii_redaction: boolean;
    target_language: string | null;
    context_prompt: string | null;
}

export function parseNemotronModelConfig(rawModel: string | undefined): NemotronSettingsPayload {
    const trimmed = (rawModel || '').trim();
    if (trimmed.startsWith('{')) {
        try {
            const parsed = JSON.parse(trimmed);
            return {
                model: parsed.model || DEFAULT_NEMOTRON_MODEL,
                endpoint: parsed.endpoint || DEFAULT_NEMOTRON_ENDPOINT,
                speaker_attribution:
                    typeof parsed.speaker_attribution === 'boolean' ? parsed.speaker_attribution : true,
                inline_pii_redaction: Boolean(parsed.inline_pii_redaction),
                target_language: parsed.target_language || null,
                context_prompt: parsed.context_prompt || null,
            };
        } catch {
            // Fall through to defaults
        }
    }
    return {
        model: trimmed || DEFAULT_NEMOTRON_MODEL,
        endpoint: DEFAULT_NEMOTRON_ENDPOINT,
        speaker_attribution: true,
        inline_pii_redaction: false,
        target_language: null,
        context_prompt: null,
    };
}

export interface TranscriptModelProps {
    provider: 'localWhisper' | 'parakeet' | 'nemotron' | 'deepgram' | 'elevenLabs' | 'groq' | 'openai';
    model: string;
    apiKey?: string | null;
    hasApiKey?: boolean;
}

export interface TranscriptSettingsProps {
    transcriptModelConfig: TranscriptModelProps;
    setTranscriptModelConfig: (config: TranscriptModelProps) => void;
    onModelSelect?: () => void;
}

const NEMOTRON_TRANSLATION_TARGETS = [
    { value: 'none', label: 'None — Keep original spoken language' },
    { value: 'English', label: 'English' },
    { value: 'Turkish', label: 'Turkish (Türkçe)' },
    { value: 'Spanish', label: 'Spanish (Español)' },
    { value: 'German', label: 'German (Deutsch)' },
    { value: 'French', label: 'French (Français)' },
    { value: 'Japanese', label: 'Japanese (日本語)' },
    { value: 'Arabic', label: 'Arabic (العربية)' },
    { value: 'Portuguese', label: 'Portuguese (Português)' },
    { value: 'Italian', label: 'Italian (Italiano)' },
    { value: 'Korean', label: 'Korean (한국어)' },
    { value: 'Chinese', label: 'Chinese (中文)' },
];

export function TranscriptSettings({ transcriptModelConfig, setTranscriptModelConfig, onModelSelect }: TranscriptSettingsProps) {
    const [apiKey, setApiKey] = useState<string | null>(transcriptModelConfig.apiKey || null);
    const [hasStoredApiKey, setHasStoredApiKey] = useState<boolean>(Boolean(transcriptModelConfig.hasApiKey));
    const [showApiKey, setShowApiKey] = useState<boolean>(false);
    const [isApiKeyLocked, setIsApiKeyLocked] = useState<boolean>(true);
    const [showLockedHint, setShowLockedHint] = useState<boolean>(false);
    const [uiProvider, setUiProvider] = useState<TranscriptModelProps['provider']>(transcriptModelConfig.provider);
    const [nemotronCfg, setNemotronCfg] = useState<NemotronSettingsPayload>(() =>
        parseNemotronModelConfig(
            transcriptModelConfig.provider === 'nemotron' ? transcriptModelConfig.model : undefined
        )
    );

    useEffect(() => {
        setUiProvider(transcriptModelConfig.provider);
        if (transcriptModelConfig.provider === 'nemotron') {
            setNemotronCfg(parseNemotronModelConfig(transcriptModelConfig.model));
        }
    }, [transcriptModelConfig.provider, transcriptModelConfig.model]);

    useEffect(() => {
        if (transcriptModelConfig.provider === 'localWhisper' || transcriptModelConfig.provider === 'parakeet') {
            setApiKey(null);
        }
    }, [transcriptModelConfig.provider]);

    const fetchApiKeyPresence = async (provider: string) => {
        try {
            const exists = await invoke<boolean>('api_has_transcript_api_key', { provider });
            setApiKey('');
            setHasStoredApiKey(exists);
        } catch (err) {
            console.error('Error checking API key presence:', err);
            setApiKey(null);
            setHasStoredApiKey(false);
        }
    };

    const persistNemotronConfig = async (next: NemotronSettingsPayload, nextApiKey?: string | null) => {
        setNemotronCfg(next);
        const serialized = JSON.stringify(next);
        const keyToSave = nextApiKey !== undefined ? nextApiKey : apiKey;
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'nemotron',
            model: serialized,
            apiKey: keyToSave || null,
            hasApiKey: hasStoredApiKey || Boolean(keyToSave && keyToSave.trim()),
        });
        try {
            await invoke('api_save_transcript_config', {
                provider: 'nemotron',
                model: serialized,
                apiKey: keyToSave && keyToSave.trim() ? keyToSave.trim() : null,
            });
        } catch {
            // Non-Tauri browser preview safe no-op
        }
    };

    const modelOptions: Record<TranscriptModelProps['provider'], string[]> = {
        localWhisper: [],
        parakeet: [],
        nemotron: [DEFAULT_NEMOTRON_MODEL],
        deepgram: ['nova-2-phonecall'],
        elevenLabs: ['eleven_multilingual_v2'],
        groq: ['llama-3.3-70b-versatile'],
        openai: ['gpt-4o'],
    };
    const requiresApiKey =
        uiProvider === 'nemotron' ||
        uiProvider === 'deepgram' ||
        uiProvider === 'elevenLabs' ||
        uiProvider === 'openai' ||
        uiProvider === 'groq';

    const handleInputClick = () => {
        if (isApiKeyLocked) {
            setShowLockedHint(true);
            setTimeout(() => setShowLockedHint(false), 4000);
        }
    };

    const handleWhisperModelSelect = (modelName: string) => {
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'localWhisper',
            model: modelName
        });
        if (onModelSelect) {
            onModelSelect();
        }
    };

    const handleParakeetModelSelect = (modelName: string) => {
        setTranscriptModelConfig({
            ...transcriptModelConfig,
            provider: 'parakeet',
            model: modelName
        });
        if (onModelSelect) {
            onModelSelect();
        }
    };

    return (
        <div className="space-y-4 pb-6">
            <SettingsCard
                title="Transcription engine"
                description="Choose the speech recognition engine that processes your audio."
            >
                <div className="space-y-3">
                    <div>
                        <Label className="mb-1.5 block text-label text-foreground">
                            Transcript Model
                        </Label>
                        <div className="flex gap-2">
                            <Select
                                value={uiProvider}
                                onValueChange={(value) => {
                                    const provider = value as TranscriptModelProps['provider'];
                                    setUiProvider(provider);
                                    if (provider === 'nemotron') {
                                        const initial = parseNemotronModelConfig(
                                            transcriptModelConfig.provider === 'nemotron'
                                                ? transcriptModelConfig.model
                                                : undefined
                                        );
                                        void fetchApiKeyPresence('nemotron');
                                        void persistNemotronConfig(initial);
                                    } else if (provider !== 'localWhisper' && provider !== 'parakeet') {
                                        void fetchApiKeyPresence(provider);
                                    }
                                }}
                            >
                                <SelectTrigger className={focusRing('card')}>
                                    <SelectValue placeholder="Select provider" />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="parakeet">⚡ Parakeet (Fast local transcription)</SelectItem>
                                    <SelectItem value="localWhisper">🏠 Local Whisper (Multiple model sizes)</SelectItem>
                                    <SelectItem value="nemotron">🟢 NVIDIA Nemotron 3.5 (Multi-speaker &amp; translation)</SelectItem>
                                </SelectContent>
                            </Select>

                            {uiProvider !== 'localWhisper' && uiProvider !== 'parakeet' && uiProvider !== 'nemotron' && (
                                <Select
                                    value={transcriptModelConfig.model}
                                    onValueChange={(value) => {
                                        const model = value as TranscriptModelProps['model'];
                                        setTranscriptModelConfig({ ...transcriptModelConfig, provider: uiProvider, model });
                                    }}
                                >
                                    <SelectTrigger className={focusRing('card')}>
                                        <SelectValue placeholder="Select model" />
                                    </SelectTrigger>
                                    <SelectContent>
                                        {modelOptions[uiProvider].map((model) => (
                                            <SelectItem key={model} value={model}>{model}</SelectItem>
                                        ))}
                                    </SelectContent>
                                </Select>
                            )}
                        </div>
                        <p className="mt-2 text-caption text-muted-foreground">
                            Accuracy varies with language, speaker, microphone, overlap, and noise.
                            Target-environment validation has not been performed; verify important text
                            against the source audio.
                        </p>
                    </div>
                </div>
            </SettingsCard>

            {uiProvider === 'localWhisper' && (
                <SettingsCard
                    title="Local Whisper models"
                    description="Models are downloaded once and run on your device."
                >
                    <ModelManager
                        selectedModel={transcriptModelConfig.provider === 'localWhisper' ? transcriptModelConfig.model : undefined}
                        onModelSelect={handleWhisperModelSelect}
                        autoSave={true}
                    />
                </SettingsCard>
            )}

            {uiProvider === 'parakeet' && (
                <SettingsCard
                    title="Parakeet models"
                    description="Quantized on-device speech recognition models."
                >
                    <ParakeetModelManager
                        selectedModel={transcriptModelConfig.provider === 'parakeet' ? transcriptModelConfig.model : undefined}
                        onModelSelect={handleParakeetModelSelect}
                        autoSave={true}
                    />
                </SettingsCard>
            )}

            {uiProvider === 'nemotron' && (
                <>
                    <SettingsCard
                        title="NVIDIA Nemotron 3.5 Transcribe"
                        description="Multi-speaker transcription (up to 8 speakers), optional translation, and custom vocabulary."
                    >
                        <div className="space-y-4">
                            <div className="grid gap-3 sm:grid-cols-2">
                                <div>
                                    <Label className="mb-1 block text-label text-foreground">
                                        Server URL
                                    </Label>
                                    <Input
                                        value={nemotronCfg.endpoint}
                                        onChange={(e) =>
                                            void persistNemotronConfig({
                                                ...nemotronCfg,
                                                endpoint: e.target.value,
                                            })
                                        }
                                        placeholder={DEFAULT_NEMOTRON_ENDPOINT}
                                    />
                                    <p className="mt-1 text-caption text-muted-foreground">
                                        Default <span className="font-mono">{DEFAULT_NEMOTRON_ENDPOINT}</span> keeps all audio on your machine.
                                    </p>
                                </div>

                                <div>
                                    <Label className="mb-1 block text-label text-foreground">
                                        Model name
                                    </Label>
                                    <Input
                                        value={nemotronCfg.model}
                                        onChange={(e) =>
                                            void persistNemotronConfig({
                                                ...nemotronCfg,
                                                model: e.target.value,
                                            })
                                        }
                                        placeholder={DEFAULT_NEMOTRON_MODEL}
                                    />
                                    <p className="mt-1 text-caption text-muted-foreground">
                                        Model identifier served by your endpoint.
                                    </p>
                                </div>
                            </div>

                            <div className="divide-y [&>*+*]:border-t-border">
                                <SwitchRow
                                    label="Speaker separation (Speaker 1–8)"
                                    description="Separates up to 8 speakers and marks overlapping speech in real time. Labels stay anonymous (Speaker 1–8)."
                                    checked={nemotronCfg.speaker_attribution}
                                    onCheckedChange={(checked) =>
                                        void persistNemotronConfig({
                                            ...nemotronCfg,
                                            speaker_attribution: checked,
                                        })
                                    }
                                />
                                <SwitchRow
                                    label="Hide personal details (emails, phone numbers, IDs)"
                                    description="Replaces personal emails, phone numbers, and IDs with [REDACTED] markers during transcription."
                                    checked={nemotronCfg.inline_pii_redaction}
                                    onCheckedChange={(checked) =>
                                        void persistNemotronConfig({
                                            ...nemotronCfg,
                                            inline_pii_redaction: checked,
                                        })
                                    }
                                />
                            </div>

                            <div className="grid gap-3 sm:grid-cols-2">
                                <div>
                                    <Label className="mb-1 block text-label text-foreground">
                                        Translate transcript into (optional)
                                    </Label>
                                    <Select
                                        value={nemotronCfg.target_language || 'none'}
                                        onValueChange={(val) =>
                                            void persistNemotronConfig({
                                                ...nemotronCfg,
                                                target_language: val === 'none' ? null : val,
                                            })
                                        }
                                    >
                                        <SelectTrigger className={focusRing('card')}>
                                            <SelectValue placeholder="Keep spoken language" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            {NEMOTRON_TRANSLATION_TARGETS.map((t) => (
                                                <SelectItem key={t.value} value={t.value}>
                                                    {t.label}
                                                </SelectItem>
                                            ))}
                                        </SelectContent>
                                    </Select>
                                </div>

                                <div>
                                    <Label className="mb-1 block text-label text-foreground">
                                        Custom vocabulary
                                    </Label>
                                    <Input
                                        value={nemotronCfg.context_prompt || ''}
                                        onChange={(e) =>
                                            void persistNemotronConfig({
                                                ...nemotronCfg,
                                                context_prompt: e.target.value || null,
                                            })
                                        }
                                        placeholder="e.g., Mityu, product names, team acronyms"
                                    />
                                    <p className="mt-1 text-caption text-muted-foreground">
                                        Improves recognition of product names, acronyms, and technical terms.
                                    </p>
                                </div>
                            </div>
                        </div>
                    </SettingsCard>
                </>
            )}

            {requiresApiKey && (
                <SettingsCard
                    title={uiProvider === 'nemotron' ? 'API key (optional for local vLLM / NIM)' : 'API key'}
                    description={
                        uiProvider === 'nemotron'
                            ? 'Leave empty when running on localhost (127.0.0.1). Required only for remote endpoints; stored in your OS credential store.'
                            : 'Stored in your operating system credential store.'
                    }
                >
                    <div className="relative">
                        <Input
                            type={showApiKey ? "text" : "password"}
                            className={`pr-24 ${isApiKeyLocked ? 'bg-muted cursor-not-allowed' : ''}`}
                            value={apiKey || ''}
                            onChange={(e) => {
                                const value = e.target.value;
                                setApiKey(value);
                                if (uiProvider === 'nemotron') {
                                    void persistNemotronConfig(nemotronCfg, value);
                                } else {
                                    setTranscriptModelConfig({
                                        ...transcriptModelConfig,
                                        provider: uiProvider,
                                        apiKey: value || null,
                                        hasApiKey: hasStoredApiKey || Boolean(value.trim()),
                                    });
                                }
                            }}
                            disabled={isApiKeyLocked}
                            onClick={handleInputClick}
                            placeholder={hasStoredApiKey ? 'Stored securely — enter to replace' : 'Enter your API key'}
                        />
                        {isApiKeyLocked && (
                            <div
                                onClick={handleInputClick}
                                className="absolute inset-0 flex items-center justify-center rounded-md bg-muted/50 cursor-not-allowed"
                            />
                        )}
                        <div className="absolute inset-y-0 right-0 pr-1 flex items-center gap-0.5">
                            <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                onClick={() => {
                                    setShowLockedHint(false);
                                    setIsApiKeyLocked(!isApiKeyLocked);
                                }}
                                aria-label={isApiKeyLocked ? 'Unlock API key' : 'Lock API key'}
                                title={isApiKeyLocked ? "Unlock to edit" : "Lock to prevent editing"}
                                className={`transition-colors duration-fast ${focusRing('card')} ${showLockedHint ? 'text-destructive-ink' : ''}`}
                            >
                                {isApiKeyLocked ? <Lock className="h-4 w-4" aria-hidden="true" /> : <Unlock className="h-4 w-4" aria-hidden="true" />}
                            </Button>
                            <Button
                                type="button"
                                variant="ghost"
                                size="icon"
                                onClick={() => setShowApiKey(!showApiKey)}
                                aria-label={showApiKey ? 'Hide API key' : 'Show API key'}
                                title={showApiKey ? 'Hide API key' : 'Show API key'}
                                className={`transition-colors duration-fast ${focusRing('card')}`}
                            >
                                {showApiKey ? <EyeOff className="h-4 w-4" aria-hidden="true" /> : <Eye className="h-4 w-4" aria-hidden="true" />}
                            </Button>
                        </div>
                    </div>
                    {showLockedHint && (
                        <p role="status" className="mt-1 text-caption text-destructive-ink">
                            This key is locked. Use the lock button to edit it.
                        </p>
                    )}
                </SettingsCard>
            )}
        </div>
    );
}
