import React, { useState, useEffect } from "react";
import { getVersion } from '@tauri-apps/api/app';
import { openExternalUrl } from '@/services/systemService';
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch";
import { UpdateDialog } from "./UpdateDialog";
import { updateService, UpdateInfo } from '@/services/updateService';
import { Button } from './ui/button';
import { Loader2, CheckCircle2 } from 'lucide-react';
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
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-3">
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img
                        src="/mityu-mark.svg"
                        alt="Mityu logo"
                        width={64}
                        height={64}
                        className="mx-auto"
                    />
                </div>
                <span className="text-sm text-muted-foreground"> v{currentVersion}</span>
                <p className="text-body text-muted-foreground mt-1">
                    Meetings captured and reviewed — entirely on your machine. No bots, no uploads, no account.
                </p>
                <div className="mt-3">
                    <Button
                        onClick={handleCheckForUpdates}
                        disabled={isChecking}
                        variant="outline"
                        size="sm"
                        className="text-xs"
                    >
                        {isChecking ? (
                            <>
                                <Loader2 className="h-3 w-3 mr-2 animate-spin" />
                                Checking...
                            </>
                        ) : (
                            <>
                                <CheckCircle2 className="h-3 w-3 mr-2" />
                                Check for Updates
                            </>
                        )}
                    </Button>
                    {updateInfo?.available && (
                        <div className="mt-2 text-xs text-primary">
                            Update available: v{updateInfo.version}
                        </div>
                    )}
                </div>
            </div>

            <div className="rounded border border-amber-300 bg-amber-50 p-3 text-xs leading-relaxed text-amber-900">
                <span className="font-semibold">Validation and storage notice:</span>{' '}
                transcription quality varies by language, microphone, overlap, and noise; the target-environment benchmark and human pilot have not been performed. Review important text against its source audio. Speaker separation is a best-effort estimate that runs after the recording ends. The live copilot is a beta: off by default, and not yet checked against a real model. Raw meeting audio remains local on this device until you delete the meeting.
            </div>

            {/* Why Mityu - three pillars, aligned with the landing page copy */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-foreground">Why Mityu</h2>
                <div className="space-y-2">
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">On-device</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Recording and transcription run locally with Whisper large-v3 or Parakeet — no network needed. Speaker separation and talk time run on this device after the recording ends, with anonymous labels. Encrypted at rest with SQLCipher when secure OS key storage is available, with opt-in PII redaction.</p>
                    </div>
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">Any meeting app</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Mityu captures your microphone and system audio on the machine itself, so it works with Zoom, Google Meet, or Microsoft Teams — no bot joins your call. Windows 10/11 (64-bit); macOS in development.</p>
                    </div>
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">Human-approved</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Every AI-generated line is a labeled draft linked to its source in the transcript — nothing is final until you approve it. Ask this meeting answers only from that meeting&apos;s transcript, or refuses. Export approved notes to PDF, Word or Markdown. Bring your own key: Ollama, OpenAI, Anthropic, Groq, OpenRouter.</p>
                    </div>
                </div>
            </div>

            {/* In beta — the live copilot (BACKLOG EPIC I). Same wording as the
                landing page's FAQ so the app and the site describe it identically:
                off by default, Settings → Beta, not validated against a real model.

                This is the STANDING description of what the app is, which is why
                it names no version. Per-release changes belong to the What's new
                dialog (`lib/releaseNotes.ts`, ADR-0047) — two surfaces, two jobs,
                so neither has to be rewritten when the other changes. */}
            <div className="space-y-3">
                <h2 className="text-title font-semibold text-foreground">In beta</h2>
                <div className="bg-muted rounded p-3">
                    <h3 className="font-bold text-body text-foreground mb-1">Live copilot — off by default</h3>
                    <p className="text-caption text-muted-foreground">
                        Turn it on under Settings → Beta and a small always-on-top panel follows a recording you start. When you ask, it drafts a recap, a suggestion, a definition or follow-up questions from the last few minutes of the transcript; eight meeting modes decide what it offers. Every answer is labelled AI-generated and cites the transcript segment it came from, or is refused. Pin to notes keeps an answer as a draft in the meeting&apos;s summary when you save it. It never records on its own, never hides Mityu, and may use a cloud provider only if you separately allow it. It has not yet been checked against a real model.
                    </p>
                </div>
            </div>

            {/* In development — mirrors the landing page's "In development" card.
                No dates, no promises: each item ships only when it can be described
                honestly (the on-device agents line is the one docs/ROADMAP.md cites). */}
            <div className="bg-accent rounded p-3 space-y-1.5">
                <p className="text-caption text-primary">
                    <span className="font-bold">In development, not in this build:</span>
                </p>
                <ul className="text-caption text-primary list-disc pl-4 space-y-1">
                    <li>macOS build — the code targets it; nothing is published yet.</li>
                    <li>Two validation gates: transcription measured on real recordings, and the copilot&apos;s live check against a real model. These decide what Mityu is allowed to claim.</li>
                    <li>Copilot: check claims against your own documents, and attach one screenshot you preview and confirm — never continuous.</li>
                    <li>Post-call recipes from approved notes only; manual speaker names.</li>
                    <li>A library of on-device AI agents — drafting follow-ups, tracking action items. Draft-only: nothing is sent until you approve it.</li>
                    <li>Team workspaces through an optional server you can host yourself; the app keeps working without it.</li>
                </ul>
            </div>

            {/* CTA Section - Compact */}
            <div className="text-center space-y-2">
                <h3 className="text-title font-semibold text-foreground">Ready to push your business further?</h3>
                <p className="text-caption text-muted-foreground">
                    If you&apos;re planning to build privacy-first custom AI agents or a fully tailored product for your <span className="font-bold">business</span>, we can help you build it.
                </p>
                <button
                    onClick={handleContactClick}
                    className="inline-flex items-center px-4 py-2 bg-primary hover:bg-primary/90 text-white text-sm font-medium rounded transition-colors duration-200 shadow-sm hover:shadow-md"
                >
                    Chat with the bluedev team
                </button>
            </div>

            {/* Acknowledgments - Compact */}
            <div className="pt-2 border-t border-border">
                <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide mb-1.5">Acknowledgments</h3>
                <p className="text-xs text-muted-foreground leading-relaxed">
                    On-device transcription is powered by{' '}
                    <button onClick={() => openExternalLink('https://github.com/ggerganov/whisper.cpp')} className="underline hover:text-foreground">
                        whisper.cpp
                    </button>{' '}
                    (OpenAI Whisper, MIT license) and{' '}
                    <button onClick={() => openExternalLink('https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3')} className="underline hover:text-foreground">
                        NVIDIA&apos;s Parakeet
                    </button>{' '}
                    model (CC BY 4.0), with an ONNX conversion by{' '}
                    <button onClick={() => openExternalLink('https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx')} className="underline hover:text-foreground">
                        istupakov
                    </button>.
                    {' '}Built-in summarization models include{' '}
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
            <div className="pt-2 border-t border-border text-center">
                <p className="text-xs text-muted-foreground">
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

    )
}
