import React, { useState, useEffect } from "react";
import { getVersion } from '@tauri-apps/api/app';
import { openExternalUrl } from '@/services/systemService';
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch";
import { UpdateDialog } from "./UpdateDialog";
import { updateService, UpdateInfo } from '@/services/updateService';
import { Button } from './ui/button';
import { Loader2, CheckCircle2, Sparkles, ShieldCheck, Waves, Cpu } from 'lucide-react';
import { toast } from 'sonner';
import { APP_VERSION } from '@/lib/appVersion';


export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>(APP_VERSION);
    const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
    const [isChecking, setIsChecking] = useState(false);
    const [showUpdateDialog, setShowUpdateDialog] = useState(false);

    useEffect(() => {
        // Get current version on mount
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    const handleContactClick = async () => {
        try {
            await openExternalUrl('https://bluedev.dev');
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    const openExternalLink = async (url: string) => {
        try {
            await openExternalUrl(url);
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    const handleCheckForUpdates = async () => {
        setIsChecking(true);
        try {
            const info = await updateService.checkForUpdates(true);
            setUpdateInfo(info);
            if (info.available) {
                setShowUpdateDialog(true);
            } else {
                toast.success('You are running the latest version');
            }
        } catch (error: any) {
            console.error('Failed to check for updates:', error);
            toast.error('Failed to check for updates: ' + (error.message || 'Unknown error'));
        } finally {
            setIsChecking(false);
        }
    };

    return (
        <div className="h-[80vh] space-y-4 overflow-y-auto p-4">
            {/* Architectural Workstation Header */}
            <div className="rounded-lg border border-border bg-card p-4 text-center shadow-elev-1">
                <div className="mb-3">
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img
                        src="/mityu-mark.svg"
                        alt="Mityu logo"
                        width={56}
                        height={56}
                        className="mx-auto rounded-lg shadow-elev-1"
                    />
                </div>
                <div className="flex items-center justify-center gap-2">
                    <span className="font-mono text-caption font-semibold tabular-nums text-foreground">
                        v{currentVersion}
                    </span>
                </div>
                <p className="mt-2 text-body text-muted-foreground">
                    Meetings captured and reviewed — entirely on your machine. No bots, no uploads, no account.
                </p>
                <div className="mt-3.5">
                    <Button
                        onClick={handleCheckForUpdates}
                        disabled={isChecking}
                        variant="outline"
                        size="sm"
                    >
                        {isChecking ? (
                            <>
                                <Loader2 className="mr-2 size-3.5 animate-spin" />
                                Checking...
                            </>
                        ) : (
                            <>
                                <CheckCircle2 className="mr-2 size-3.5" />
                                Check for Updates
                            </>
                        )}
                    </Button>
                    {updateInfo?.available && (
                        <div className="mt-2 text-caption font-medium text-primary">
                            Update available: v{updateInfo.version}
                        </div>
                    )}
                </div>
            </div>

            <div className="rounded-lg border border-warning-border bg-warning-surface p-3 text-caption leading-relaxed text-warning-ink">
                <span className="font-semibold">Validation and storage notice:</span>{' '}
                transcription quality varies by language, microphone, overlap, and noise; the target-environment benchmark and human pilot have not been performed. Review important text against its source audio. Speaker separation is a best-effort estimate that runs live with NVIDIA Nemotron 3.5 Transcribe (up to eight anonymous speakers) or after the recording ends. The live copilot is a beta: off by default, and not yet checked against a real model. Raw meeting audio remains local on this device until you delete the meeting.
            </div>

            {/* Why Mityu - three pillars, aligned with the landing page copy */}
            <div className="space-y-3">
                <h2 className="text-title text-foreground">Why Mityu</h2>
                <div className="grid grid-cols-1 gap-2.5">
                    <div className="rounded-lg border border-border bg-card p-3.5 shadow-elev-1">
                        <div className="mb-1 flex items-center justify-between gap-2">
                            <h3 className="flex items-center gap-1.5 text-title-sm text-foreground">
                                <Cpu className="size-4 text-primary" aria-hidden="true" />
                                On-device
                            </h3>
                        </div>
                        <p className="text-caption leading-relaxed text-muted-foreground">Recording and transcription run locally with Whisper large-v3, Parakeet, or NVIDIA Nemotron 3.5 Transcribe — no network needed. Speaker separation and talk time run on this device after the recording ends (or live with Nemotron 3.5 up to eight speakers), with anonymous labels. Encrypted at rest with SQLCipher when secure OS key storage is available, with opt-in PII redaction.</p>
                    </div>
                    <div className="rounded-lg border border-border bg-card p-3.5 shadow-elev-1">
                        <div className="mb-1 flex items-center justify-between gap-2">
                            <h3 className="flex items-center gap-1.5 text-title-sm text-foreground">
                                <Waves className="size-4 text-primary" aria-hidden="true" />
                                Any meeting app
                            </h3>
                        </div>
                        <p className="text-caption leading-relaxed text-muted-foreground">Mityu captures your microphone and system audio on the machine itself, so it works with Zoom, Google Meet, or Microsoft Teams — no bot joins your call. Windows 10/11 (64-bit) with an AVX2-capable processor; macOS in development.</p>
                    </div>
                    <div className="rounded-lg border border-border bg-card p-3.5 shadow-elev-1">
                        <div className="mb-1 flex items-center justify-between gap-2">
                            <h3 className="flex items-center gap-1.5 text-title-sm text-foreground">
                                <ShieldCheck className="size-4 text-primary" aria-hidden="true" />
                                Human-approved
                            </h3>
                        </div>
                        <p className="text-caption leading-relaxed text-muted-foreground">Every AI-generated line is a labeled draft linked to its source in the transcript — nothing is final until you approve it. Ask this meeting answers only from that meeting&apos;s transcript, or refuses. Export approved notes to PDF, Word or Markdown. Bring your own key: Ollama, OpenAI, Anthropic, Groq, OpenRouter.</p>
                    </div>
                </div>
            </div>

            {/* In beta — the live copilot (BACKLOG EPIC I). */}
            <div className="space-y-3">
                <h2 className="text-title text-foreground">In beta</h2>
                <div className="rounded-lg border border-ai-border bg-ai-surface p-3.5">
                    <div className="mb-1 flex items-center justify-between gap-2">
                        <h3 className="flex items-center gap-1.5 text-title-sm text-ai-ink">
                            <Sparkles className="size-4" aria-hidden="true" />
                            Live copilot — off by default
                        </h3>
                    </div>
                    <p className="text-caption leading-relaxed text-muted-foreground">
                        Turn it on under Settings → Beta and a small always-on-top panel follows a recording you start. When you ask, it drafts a recap, a suggestion, a definition or follow-up questions from the last few minutes of the transcript; eight meeting modes decide what it offers. Every answer is labelled AI-generated and cites the transcript segment it came from, or is refused. Pin to notes keeps an answer as a draft in the meeting&apos;s summary when you save it. It ships with &ldquo;Allow cloud insights&rdquo; off, so it answers only with a model running on your device and refuses rather than sending anything away; turning that on under Settings &rarr; Beta sends the last few minutes of the conversation to the provider you configured, on every action you press. It never records on its own and never hides Mityu. It has not yet been checked against a real model.
                    </p>
                </div>
            </div>

            <div className="rounded-lg border border-border bg-muted p-3">
                <p className="text-caption leading-relaxed text-muted-foreground">
                    <span className="font-semibold text-foreground">What changed in this version:</span>{' '}
                    Mityu shows you after every update, and you can re-open it any time from Settings → General.
                </p>
            </div>

            <div className="rounded-lg border border-border bg-accent p-3">
                <p className="text-caption text-accent-foreground">
                    <span className="font-semibold">Coming soon:</span> A library of on-device AI agents — drafting follow-ups, tracking action items, and more. Draft-only: nothing is sent until you approve it.
                </p>
            </div>

            {/* CTA Section - Compact */}
            <div className="rounded-lg border border-border bg-card p-4 text-center space-y-2">
                <h3 className="text-title text-foreground">Ready to push your business further?</h3>
                <p className="text-caption text-muted-foreground">
                    If you&apos;re planning to build privacy-first custom AI agents or a fully tailored product for your <span className="font-semibold text-foreground">business</span>, we can help you build it.
                </p>
                <Button onClick={handleContactClick} variant="default" size="sm">
                    Chat with the bluedev team
                </Button>
            </div>

            {/* Acknowledgments - Compact */}
            <div className="border-t border-border pt-3">
                <h3 className="mb-1.5 text-eyebrow uppercase text-subtle-foreground">Acknowledgments</h3>
                <p className="text-caption leading-relaxed text-muted-foreground">
                    On-device transcription is powered by{' '}
                    <button onClick={() => openExternalLink('https://github.com/ggerganov/whisper.cpp')} className="underline hover:text-foreground">
                        whisper.cpp
                    </button>{' '}
                    (OpenAI Whisper, MIT license),{' '}
                    <button onClick={() => openExternalLink('https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3')} className="underline hover:text-foreground">
                        NVIDIA&apos;s Parakeet
                    </button>{' '}
                    model (CC BY 4.0) with an ONNX conversion by{' '}
                    <button onClick={() => openExternalLink('https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx')} className="underline hover:text-foreground">
                        istupakov
                    </button>, and{' '}
                    <button onClick={() => openExternalLink('https://huggingface.co/nvidia')} className="underline hover:text-foreground">
                        NVIDIA Nemotron 3.5 Transcribe
                    </button>. Built-in summarization models include{' '}
                    <button onClick={() => openExternalLink('https://huggingface.co/Qwen/Qwen3.5-2B')} className="underline hover:text-foreground">
                        Qwen 3.5
                    </button>{' '}
                    (Apache 2.0) and{' '}
                    <button onClick={() => openExternalLink('https://ai.google.dev/gemma/terms')} className="underline hover:text-foreground">
                        Gemma 3
                    </button>{' '}
                    (subject to Google&apos;s Gemma Terms and prohibited-use policy). Model notices are included with the app.
                    {' '}Audio conversion uses a separately bundled{' '}
                    <button onClick={() => openExternalLink('https://ffmpeg.org/')} className="underline hover:text-foreground">
                        FFmpeg
                    </button>{' '}
                    executable (this build: LGPL v3 or later); license and source details are included with the app.
                </p>
            </div>

            {/* Footer - Compact */}
            <div className="border-t border-border pt-2 text-center">
                <p className="text-caption text-subtle-foreground">
                    Built by bluedev
                </p>
            </div>
            <AnalyticsConsentSwitch />

            {/* Update Dialog */}
            <UpdateDialog
                open={showUpdateDialog}
                onOpenChange={setShowUpdateDialog}
                updateInfo={updateInfo}
            />
        </div>

    );
}
