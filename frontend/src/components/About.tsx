import React, { useState, useEffect } from "react";
import { getVersion } from '@tauri-apps/api/app';
import { openExternalUrl } from '@/services/systemService';
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch";
import { UpdateDialog } from "./UpdateDialog";
import { updateService, UpdateInfo } from '@/services/updateService';
import { Button } from './ui/button';
import { Loader2, CheckCircle2 } from 'lucide-react';
import { toast } from 'sonner';


export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>('0.4.0');
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
                <p className="text-medium text-muted-foreground mt-1">
                    Real-time notes and summaries that never leave your machine.
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

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-foreground">What makes Mityu different</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">Privacy-first</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Your data & AI processing workflow can now stay within your premise. No cloud, no leaks.</p>
                    </div>
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">Use Any Model</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Prefer local open-source model? Great. Want to plug in an external API? Also fine. No lock-in.</p>
                    </div>
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">Cost-Smart</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Avoid pay-per-minute bills by running models locally (or pay only for the calls you choose).</p>
                    </div>
                    <div className="bg-muted rounded p-3 hover:bg-muted transition-colors">
                        <h3 className="font-bold text-sm text-foreground mb-1">Any meeting app</h3>
                        <p className="text-xs text-muted-foreground leading-relaxed">Google Meet, Zoom, Teams — or a face-to-face conversation. Mityu captures system audio, so it never joins your call. macOS &amp; Windows (Linux experimental).</p>
                    </div>
                </div>
            </div>

            {/* Coming Soon - Compact */}
            <div className="bg-accent rounded p-3">
                <p className="text-s text-primary">
                    <span className="font-bold">Coming soon:</span> A library of on-device AI agents — drafting follow-ups, tracking action items, and more. Draft-only: nothing is sent until you approve it.
                </p>
            </div>

            {/* CTA Section - Compact */}
            <div className="text-center space-y-2">
                <h3 className="text-medium font-semibold text-foreground">Ready to push your business further?</h3>
                <p className="text-s text-muted-foreground">
                    If you're planning to build privacy-first custom AI agents or a fully tailored product for your <span className="font-bold">business</span>, we can help you build it.
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
                    Mityu is built on the open-source{' '}
                    <button onClick={() => openExternalLink('https://github.com/Zackriya-Solutions/meeting-minutes')} className="underline hover:text-foreground">
                        Meetily
                    </button>{' '}
                    by Zackriya Solutions (MIT license). Mityu is a separate product by bluedev and is not affiliated with, nor endorsed by, Meetily or Zackriya Solutions.
                </p>
                <p className="text-xs text-muted-foreground leading-relaxed mt-1.5">
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