'use client';

/**
 * /design/dialogs — the app's blocking surfaces, Tauri-free (DESIGN_SYSTEM.md §11.1).
 *
 * These are the screens a user meets at their worst moment: about to record people, or
 * about to destroy something. Until now none of them could be seen without the desktop
 * app, and several of them were native `alert()` and `confirm()` boxes that cannot be
 * screenshotted at all because the operating system draws them.
 *
 * ONE AT A TIME, selected by a query parameter — the pattern `/design/hitl?reject=1` and
 * `/design/tour?tour=N` already use. A modal portals to the body and covers the page, so
 * rendering two of them open at once produces a screenshot of whichever won, stacked over
 * a dimmed copy of everything else. The alternative — rebuilding their contents inline —
 * would prove a lookalike.
 *
 *   /design/dialogs               the inline feedback that replaced twelve alert() boxes
 *   /design/dialogs?dialog=consent   the recording-consent gate
 *   /design/dialogs?dialog=delete    the destructive confirmation
 */

import React, { Suspense, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import { RecordingConsentDialog } from '@/components/consent/RecordingConsentDialog';
import { ConfirmationModal } from '@/components/ConfirmationModel/confirmation-modal';
import { Notice } from '@/components/ui/notice';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';

/** 🔒 ADR-0026 — the disclosure a person reads before a meeting is destroyed. */
const DELETE_DISCLOSURE =
  'This removes the meeting from Mityu-managed local database and search data, its Mityu-managed recording artifacts, and recovery cache. Unknown files you placed in the recording folder are retained. This cannot be undone. SSD wear-leveling, copy-on-write filesystems, snapshots, backups, exports, and WebView/browser storage may retain physical traces or separate copies that Mityu cannot erase.';

function Panel({ theme, dialog }: { theme: 'light' | 'dark'; dialog: string | null }) {
  const [consentOpen, setConsentOpen] = useState(dialog === 'consent');
  const [deleteOpen, setDeleteOpen] = useState(dialog === 'delete');

  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="space-y-6 bg-background p-6 text-foreground">
        <header className="space-y-1">
          <div className="text-eyebrow uppercase text-subtle-foreground">{theme} theme</div>
          <h1 className="text-display">Dialogs and feedback</h1>
          <p className="text-body text-muted-foreground">
            §6.8. The surfaces a user meets before recording people, and before deleting
            their own evidence.
          </p>
        </header>

        {dialog === 'consent' && (
          <section className="space-y-2">
            <h2 className="text-title">Recording consent</h2>
            <p className="text-caption text-subtle-foreground">
              Every dismissal path — Cancel, the close control, Escape, a click outside —
              aborts the start. The confirm is the one filled blue action: a human decided.
            </p>
            <RecordingConsentDialog
              open={consentOpen}
              onConfirm={() => setConsentOpen(false)}
              onCancel={() => setConsentOpen(false)}
            />
          </section>
        )}

        {dialog === 'delete' && (
          <section className="space-y-2">
            <h2 className="text-title">Destructive confirmation</h2>
            <p className="text-caption text-subtle-foreground">
              It names the meeting, and the full disclosure is one disclosure away rather
              than ninety words in the lead.
            </p>
            <ConfirmationModal
              isOpen={deleteOpen}
              title="Delete “Ankara site visit — retention policy”?"
              text={DELETE_DISCLOSURE}
              onConfirm={() => setDeleteOpen(false)}
              onCancel={() => setDeleteOpen(false)}
            />
          </section>
        )}

        <section className="space-y-2">
          <h2 className="text-title">Inline feedback, where an alert used to be</h2>
          <p className="text-caption text-subtle-foreground">
            Twelve native alert and confirm boxes are gone. Each one is now a message
            that names what happened and what to do next — themed, translatable, and
            unable to freeze the window.
          </p>
          <Card className="space-y-3 p-4">
            <Notice tone="destructive" title="Recording is unavailable">
              Mityu could not reach the audio engine. Restart the app; if it keeps
              happening, reinstall.
            </Notice>
            <Notice tone="warning" title="Could not open system settings">
              Open System Settings → Privacy &amp; Security → Microphone and allow Mityu,
              then choose Re-check.
            </Notice>
            <Notice tone="info" title="Live transcript is unavailable">
              Recording continues and audio is still saved, but text will not appear
              until you restart the app.
            </Notice>
            <Notice
              tone="success"
              title="Meeting deleted successfully"
              action={
                <Button variant="outline" size="sm">
                  Undo
                </Button>
              }
            >
              Mityu-managed database, search, recording, and recovery data was removed.
            </Notice>
          </Card>
        </section>
      </div>
    </div>
  );
}

function DialogsPanels() {
  const dialog = useSearchParams().get('dialog');
  // A modal covers the window, so a themed pair only makes sense when nothing is open.
  if (dialog) return <Panel theme="light" dialog={dialog} />;
  return (
    <div>
      <Panel theme="light" dialog={null} />
      <Panel theme="dark" dialog={null} />
    </div>
  );
}

export default function DesignDialogsPage() {
  return (
    <Suspense fallback={null}>
      <DialogsPanels />
    </Suspense>
  );
}
