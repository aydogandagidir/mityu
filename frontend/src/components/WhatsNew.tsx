'use client';

/**
 * What changed in this version (BACKLOG G2).
 *
 * The user updated to 1.2.1 and had no way to learn what they had received.
 * Everything a release changed was in a GitHub release body they never see.
 *
 * Three decisions worth stating, because each is a way this could have been
 * built wrong:
 *
 * 1. **The notes are bundled, never fetched.** Local-first is the product
 *    (CLAUDE.md §0.1). A user with no network still learns what changed, and
 *    the text cannot be edited after the build is on their machine.
 * 2. **A fresh install sees nothing.** Nothing is "new" to someone who has
 *    never run the app; the product tour is what greets them. The dialog opens
 *    only when the version actually moved under a user who was already here.
 * 3. **Caveats sit in the same dialog as the features.** This app ships things
 *    that are off by default or unverified. A "what's new" that lists only the
 *    good half would be the same broken promise the rest of the product is
 *    built to avoid.
 */

import { useCallback, useEffect, useState } from 'react';
import { Sparkles, AlertTriangle } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { APP_VERSION } from '@/lib/appVersion';
import { RELEASE_NOTES, type ReleaseNote, whatsNewFor } from '@/lib/releaseNotes';
import { isTauri } from '@/lib/isTauri';

const STORE_FILE = 'whats-new.json';
const LAST_SEEN_KEY = 'lastSeenVersion';

/**
 * Shown at most once per app launch when the store cannot be written.
 *
 * A store that fails to save means "seen" cannot be recorded, so an
 * unconditional open would greet the user on every launch forever. Failing
 * open ONCE tells them what changed without becoming a nuisance they cannot
 * dismiss.
 */
let shownThisSession = false;

async function readLastSeen(): Promise<string | null> {
  if (!isTauri()) return null;
  const { Store } = await import('@tauri-apps/plugin-store');
  const store = await Store.load(STORE_FILE);
  return (await store.get<string>(LAST_SEEN_KEY)) ?? null;
}

async function writeLastSeen(version: string): Promise<void> {
  const { Store } = await import('@tauri-apps/plugin-store');
  const store = await Store.load(STORE_FILE);
  await store.set(LAST_SEEN_KEY, version);
  await store.save();
}

function NoteBody({ note }: { note: ReleaseNote }) {
  return (
    <section className="space-y-4">
      <header className="space-y-1">
        <div className="flex items-baseline gap-2">
          <h3 className="text-base font-semibold text-foreground">Version {note.version}</h3>
          <span className="text-xs text-muted-foreground">{note.date}</span>
        </div>
        <p className="text-sm text-muted-foreground">{note.headline}</p>
      </header>

      <ul className="space-y-3">
        {note.changes.map((change) => (
          <li key={change.title} className="space-y-0.5">
            <p className="text-sm font-medium text-foreground">{change.title}</p>
            <p className="text-sm leading-relaxed text-muted-foreground">{change.detail}</p>
          </li>
        ))}
      </ul>

      {/* Not a footnote and not behind a link: what is still not true belongs
          next to what is. */}
      {note.caveats.length > 0 && (
        <div className="rounded-lg border border-border bg-muted/40 p-3">
          <p className="mb-2 flex items-center gap-1.5 text-xs font-medium text-foreground">
            <AlertTriangle className="h-3.5 w-3.5 shrink-0" aria-hidden />
            Worth knowing
          </p>
          <ul className="space-y-1.5">
            {note.caveats.map((caveat) => (
              <li key={caveat} className="text-xs leading-relaxed text-muted-foreground">
                {caveat}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

export interface WhatsNewProps {
  /** Force the dialog open (the "What's new" entry in Settings). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function WhatsNew({ open: controlledOpen, onOpenChange }: WhatsNewProps = {}) {
  const [autoOpen, setAutoOpen] = useState(false);
  const [notes, setNotes] = useState<ReleaseNote[]>([]);

  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : autoOpen;

  useEffect(() => {
    if (isControlled) {
      // Opened on demand: show this version's note, or the newest we have.
      setNotes(RELEASE_NOTES.filter((n) => n.version === APP_VERSION).concat(
        RELEASE_NOTES.some((n) => n.version === APP_VERSION) ? [] : RELEASE_NOTES.slice(0, 1)
      ));
      return;
    }

    let cancelled = false;
    const decide = async () => {
      if (!isTauri() || shownThisSession) return;
      try {
        const lastSeen = await readLastSeen();
        const due = whatsNewFor(APP_VERSION, lastSeen);
        // Record the version even when there is nothing to show, so the FIRST
        // run after a fresh install establishes the baseline and the next
        // update is the first thing this dialog ever announces.
        await writeLastSeen(APP_VERSION);
        if (cancelled || due.length === 0) return;
        shownThisSession = true;
        setNotes(due);
        setAutoOpen(true);
      } catch (error) {
        console.warn("What's new: could not read or record the last seen version", error);
      }
    };
    decide();
    return () => {
      cancelled = true;
    };
  }, [isControlled]);

  const handleOpenChange = useCallback(
    (next: boolean) => {
      if (isControlled) onOpenChange?.(next);
      else setAutoOpen(next);
    },
    [isControlled, onOpenChange]
  );

  if (notes.length === 0) return null;

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-h-[80vh] max-w-lg overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-4 w-4 shrink-0 text-primary" aria-hidden />
            What&apos;s new in Mityu
          </DialogTitle>
          <DialogDescription>
            {notes.length === 1
              ? 'What changed in this update.'
              : `What changed across ${notes.length} updates.`}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-6">
          {notes.map((note) => (
            <NoteBody key={note.version} note={note} />
          ))}
        </div>

        <DialogFooter>
          <Button onClick={() => handleOpenChange(false)}>Got it</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export default WhatsNew;
