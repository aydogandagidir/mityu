'use client';

/**
 * Settings → Modes (BACKLOG **I4b**, ADR-0038).
 *
 * A mode decides what kind of conversation the copilot thinks it is in: who the
 * user is, who the other side is, which actions are offered and how strictly an
 * answer must cite. It is the largest single influence on what comes back, so
 * this tab exists to make "which one is answering" a thing the user chose
 * rather than a thing they have to infer.
 *
 * ## Every rule here is enforced in Rust, and none of them is re-implemented
 *
 * The import flow asks the backend to check a file and shows **its** sentence.
 * Duplicating the size cap, the id and name collision checks or ADR-0038's
 * rejected category in TypeScript would give two answers that drift, and only
 * the Rust one actually guards the store. So this component validates nothing;
 * it renders a refusal it was handed.
 *
 * ## What can be edited, and what cannot
 *
 * Built-ins are read-only: a user who could rewrite `General` could make the
 * copilot's default behaviour unexplainable to the next person who opens the
 * app. For a custom mode the editor exposes name, purpose and voice — its
 * *wording*. What a mode **permits** (its id, its actions, its sources) stays
 * in the authored file, because widening a permission through a text box is an
 * install, not an edit.
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { AlertTriangle, Check, Loader2, Pencil, Plus, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { modesService } from '@/services/modesService';
import type { Mode, ModeRow, ModesView } from '@/types/modes';
import { ACTION_LABELS } from './CopilotInsights';

/** The wording fields the editor exposes. */
export interface ModeDraft {
  name: string;
  purpose: string;
  voice: string;
}

export function draftFrom(mode: Mode): ModeDraft {
  return { name: mode.name, purpose: mode.purpose, voice: mode.voice };
}

/** Nothing to save when every field is unchanged or blank. */
export function draftIsSaveable(draft: ModeDraft, original: ModeDraft): boolean {
  const filled = draft.name.trim() && draft.purpose.trim() && draft.voice.trim();
  if (!filled) return false;
  return (
    draft.name !== original.name ||
    draft.purpose !== original.purpose ||
    draft.voice !== original.voice
  );
}

function ActionChips({ mode }: { mode: ModeRow }) {
  return (
    <div className="mt-1 flex flex-wrap gap-1">
      {mode.live.allowedActions.map((action) => (
        <span
          key={action}
          className="rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground"
        >
          {ACTION_LABELS[action]}
        </span>
      ))}
      {mode.live.citeRequired && (
        <span className="rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground">
          Every point cites the conversation
        </span>
      )}
    </div>
  );
}

export default function ModesSettings() {
  const [view, setView] = useState<ModesView | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [draft, setDraft] = useState<ModeDraft>({ name: '', purpose: '', voice: '' });
  const [preview, setPreview] = useState<Mode | null>(null);
  const [pendingRaw, setPendingRaw] = useState<string | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let cancelled = false;
    modesService
      .list()
      .then((v) => {
        if (!cancelled) setView(v);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /**
   * Every mutation returns the whole list, so the UI re-renders from the
   * backend's answer instead of guessing what its own call did. A failure
   * leaves the previous list on screen — which is the truth, because a refused
   * write changed nothing.
   */
  const run = useCallback(async (work: () => Promise<ModesView>) => {
    setBusy(true);
    setError(null);
    try {
      setView(await work());
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusy(false);
    }
  }, []);

  const onPickFile = useCallback(async (file: File) => {
    setError(null);
    setPreview(null);
    const raw = await file.text();
    try {
      // Checked before anything is written, so the user reads the problem
      // rather than discovering it in the list afterwards.
      setPreview(await modesService.preview(raw));
      setPendingRaw(raw);
    } catch (e) {
      setPendingRaw(null);
      setError(String(e));
    }
  }, []);

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" aria-hidden /> Loading modes…
      </div>
    );
  }

  return (
    <section className="space-y-4 p-1" aria-label="Meeting modes">
      <div>
        <h3 className="text-sm font-medium text-foreground">Meeting modes</h3>
        <p className="mt-0.5 text-xs text-muted-foreground">
          The mode decides what kind of conversation the copilot thinks it is in — who you are in
          it, what it may offer, and how strictly it must cite. It never changes what is recorded.
        </p>
      </div>

      {error && (
        <p role="alert" className="flex items-start gap-2 text-xs text-destructive">
          <AlertTriangle className="mt-0.5 h-3 w-3 shrink-0" aria-hidden />
          {error}
        </p>
      )}

      <ul className="space-y-2">
        {(view?.modes ?? []).map((mode) => (
          <li
            key={mode.id}
            className={`rounded-lg border px-3 py-2 ${
              mode.active ? 'border-foreground/40 bg-muted/40' : 'border-border'
            }`}
          >
            <div className="flex items-start gap-2">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                  <span className="text-[13px] font-medium text-foreground">{mode.name}</span>
                  {mode.builtin && (
                    <span className="rounded bg-muted px-1 py-0.5 text-[10px] text-muted-foreground">
                      Built in
                    </span>
                  )}
                  {mode.active && (
                    <span className="inline-flex items-center gap-0.5 text-[10px] text-foreground">
                      <Check className="h-3 w-3" aria-hidden /> Active
                    </span>
                  )}
                </div>
                <p className="mt-0.5 text-xs text-muted-foreground">{mode.purpose}</p>
                <p className="mt-0.5 text-[11px] text-muted-foreground">
                  You are {mode.userRole}; the other side is {mode.counterpartRole}.
                </p>
                <ActionChips mode={mode} />
              </div>
              <div className="flex shrink-0 flex-col gap-1">
                {!mode.active && (
                  <Button
                    size="sm"
                    variant="outline"
                    disabled={busy}
                    onClick={() => run(() => modesService.setActive(mode.id))}
                  >
                    Use this
                  </Button>
                )}
                {!mode.builtin && (
                  <>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      aria-label={`Edit ${mode.name}`}
                      onClick={() => {
                        setEditing(mode.id);
                        setDraft(draftFrom(mode));
                      }}
                    >
                      <Pencil className="h-3 w-3" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={busy}
                      aria-label={`Remove ${mode.name}`}
                      onClick={() => run(() => modesService.remove(mode.id))}
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  </>
                )}
              </div>
            </div>

            {editing === mode.id && (
              <div className="mt-2 space-y-2 border-t border-border pt-2">
                <label className="block text-[11px] text-muted-foreground">
                  Name
                  <Input
                    className="mt-0.5"
                    value={draft.name}
                    onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                  />
                </label>
                <label className="block text-[11px] text-muted-foreground">
                  Purpose — written as guidance to an assistant, not as marketing
                  <Input
                    className="mt-0.5"
                    value={draft.purpose}
                    onChange={(e) => setDraft({ ...draft, purpose: e.target.value })}
                  />
                </label>
                <label className="block text-[11px] text-muted-foreground">
                  Voice — how an answer should read
                  <Input
                    className="mt-0.5"
                    value={draft.voice}
                    onChange={(e) => setDraft({ ...draft, voice: e.target.value })}
                  />
                </label>
                <p className="text-[10px] text-muted-foreground">
                  What this mode may do — its actions and the sources it reads — is set in the mode
                  file, not here. Import a new file to change it.
                </p>
                <div className="flex gap-2">
                  <Button
                    size="sm"
                    disabled={busy || !draftIsSaveable(draft, draftFrom(mode))}
                    onClick={async () => {
                      if (await run(() => modesService.update(mode.id, draft))) setEditing(null);
                    }}
                  >
                    Save
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setEditing(null)}>
                    Cancel
                  </Button>
                </div>
              </div>
            )}
          </li>
        ))}
      </ul>

      <div className="space-y-2 border-t border-border pt-3">
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="outline"
            disabled={busy}
            onClick={() => fileInput.current?.click()}
          >
            <Plus className="mr-1 h-3 w-3" /> Import a mode file
          </Button>
          <span className="text-[11px] text-muted-foreground">
            A small JSON file describing one kind of conversation.
          </span>
        </div>
        <input
          ref={fileInput}
          type="file"
          accept="application/json,.json"
          className="hidden"
          aria-label="Choose a mode file"
          onChange={(e) => {
            const file = e.target.files?.[0];
            // Cleared so choosing the same file twice still fires a change.
            e.target.value = '';
            if (file) void onPickFile(file);
          }}
        />

        {preview && (
          <div className="rounded-lg border border-border p-3">
            <p className="text-[11px] text-muted-foreground">This file would install:</p>
            <p className="mt-1 text-[13px] font-medium text-foreground">{preview.name}</p>
            <p className="text-xs text-muted-foreground">{preview.purpose}</p>
            <p className="mt-0.5 text-[11px] text-muted-foreground">
              You are {preview.userRole}; the other side is {preview.counterpartRole}.
            </p>
            <div className="mt-2 flex gap-2">
              <Button
                size="sm"
                disabled={busy || !pendingRaw}
                onClick={async () => {
                  if (pendingRaw && (await run(() => modesService.install(pendingRaw)))) {
                    setPreview(null);
                    setPendingRaw(null);
                  }
                }}
              >
                Install
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => {
                  setPreview(null);
                  setPendingRaw(null);
                }}
              >
                Discard
              </Button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}
