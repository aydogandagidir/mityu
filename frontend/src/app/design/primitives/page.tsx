'use client';

/**
 * /design/primitives — the product primitives of DESIGN_SYSTEM.md §5.6–§5.19, rendered.
 *
 * This is the verification surface for WP3 (§11.1): every shared component, in every state
 * that has one, in light AND `.dark` side by side, so one screenshot proves both themes.
 * It renders the REAL components — no private copies — which is the whole point: the four
 * hand-rolled `AiLabel` / `SourceChip` / `SectionCard` / `StatTile` clones that used to live
 * in `/design/report` are exactly how a fixture drifts from the product it certifies.
 *
 * Tauri-free and with NO mount-time `invoke`/`listen`, so it renders in a plain browser —
 * that is what `tools/ui/shoot.py` (and CI) actually check. Not linked from product
 * navigation.
 *
 * `--expect` markers this route must keep printing (§11.2):
 *   `AI-generated · review required` · `Approve` · `Source ·` ·
 *   `Jump to source transcript segment`
 */

import {
  Calendar,
  Check,
  Download,
  FileText,
  Inbox,
  ListChecks,
  Mic,
  Pencil,
  Search,
  Timer,
  Users,
  X,
} from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardFooter, CardHeader, CardTitle, Section, Well } from '@/components/ui/card';
import { EmptyState } from '@/components/ui/empty-state';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { Notice } from '@/components/ui/notice';
import { Progress } from '@/components/ui/progress';
import { MeetingListSkeleton, Skeleton, TranscriptSkeleton } from '@/components/ui/skeleton';
import { StatTile, StatTileGroup } from '@/components/ui/stat-tile';
import { AiLabel, SectionCard, SourceChip } from '@/components/report/primitives';
import { StatusPill } from '@/components/report/StatusPill';

/** A labelled block inside a panel, so a reviewer can name what they are looking at. */
function Group({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="text-eyebrow uppercase text-muted-foreground">{title}</h2>
        {hint ? (
          <span className="font-mono text-micro text-subtle-foreground">{hint}</span>
        ) : null}
      </div>
      {children}
    </section>
  );
}

/**
 * One HITL block row (§5.10 anatomy), built from the primitives: the 3px status rule, the
 * block text, the meta row (StatusPill · SourceChip · AiLabel) and the action cluster whose
 * `Approve` is ALWAYS visible and ALWAYS labelled. The full control set with its eleven
 * frozen accessible names is WP10's `ReviewControls`; this row exists to prove the pieces
 * compose.
 */
function BlockRow({
  status,
  text,
  timestamp,
  resolved = true,
  active = false,
}: {
  status: 'draft' | 'approved' | 'edited' | 'rejected';
  text: string;
  timestamp?: string;
  resolved?: boolean;
  active?: boolean;
}) {
  const rule =
    status === 'draft'
      ? 'border-l-2 border-dashed border-l-ai'
      : status === 'rejected'
        ? 'border-l-2 border-l-destructive'
        : 'border-l-2 border-l-verified';

  return (
    <div className={`flex items-start gap-3 py-3 pl-3 ${rule}`}>
      <div className="min-w-0 flex-1 space-y-2">
        <p
          className={`text-read text-foreground ${status === 'rejected' ? 'line-through' : ''}`}
        >
          {text}
        </p>
        <div className="flex flex-wrap items-center gap-2">
          <StatusPill status={status} />
          <SourceChip timestamp={timestamp} resolved={resolved} active={active} />
          {status === 'draft' ? <AiLabel /> : null}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        {status === 'approved' ? null : (
          <Button variant="outline" size="row">
            <Check />
            Approve
          </Button>
        )}
        <Button variant="ghost" size="icon" aria-label="Edit block">
          <Pencil />
        </Button>
        <Button variant="ghost" size="icon" aria-label="Reject block">
          <X />
        </Button>
      </div>
    </div>
  );
}

function Panel({ theme }: { theme: 'light' | 'dark' }) {
  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="min-h-screen space-y-8 bg-background p-6 text-foreground">
        <header className="space-y-1">
          <div className="text-eyebrow uppercase text-subtle-foreground">{theme} theme</div>
          <h1 className="text-title-lg">Product primitives</h1>
          <p className="text-body text-muted-foreground">
            §5.6–§5.19, rendered from the real components.
          </p>
        </header>

        <Group title="Surfaces" hint="§5.6 card · section · well">
          <Card>
            <CardHeader>
              <CardTitle>Card</CardTitle>
              <Badge tone="neutral">3</Badge>
              <div className="ml-auto">
                <Button variant="ghost" size="icon" aria-label="Download this card">
                  <Download />
                </Button>
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              <p className="text-body text-muted-foreground">
                Body at `p-4`. No shadow — §4.8 is hairline-first.
              </p>
              <Well>
                <span className="font-mono text-micro">
                  ~/Library/Application Support/com.bluedev.mityu
                </span>
              </Well>
            </CardContent>
            <CardFooter>
              <Button variant="outline" size="sm">
                Secondary
              </Button>
              <Button size="sm">Primary</Button>
            </CardFooter>
          </Card>

          <Section label="Section label">
            <p className="text-body text-muted-foreground">
              A headerless grouping: an eyebrow and `space-y-3`.
            </p>
          </Section>
        </Group>

        <Group title="Status pills" hint="§5.8 word + icon, never colour">
          <div className="flex flex-wrap items-center gap-2">
            <StatusPill status="draft" />
            <StatusPill status="approved" />
            <StatusPill status="edited" />
            <StatusPill status="rejected" />
            <StatusPill status="legacy" />
            <StatusPill status="approved" label="5 of 7 approved" />
            <Badge tone="info">Beta</Badge>
            <Badge tone="warning">Needs attention</Badge>
            <Badge tone="neutral" dot>
              12
            </Badge>
          </div>
          <AiLabel />
        </Group>

        <Group title="Source chips" hint="§5.9 idle · active · unresolved">
          <div className="flex flex-wrap items-center gap-3">
            <SourceChip timestamp="12:04" />
            <SourceChip timestamp="50:03" active />
            <SourceChip timestamp="1:02:17" label="Source" />
          </div>
          <SourceChip timestamp={null} resolved={false} />
          <p className="text-caption text-subtle-foreground">
            A UTC worker fallback is dropped, never printed as a clock time.
          </p>
        </Group>

        <Group title="Review rows" hint="§5.10 draft · approved · rejected">
          <SectionCard icon={ListChecks} title="Key points" count={3} aiLabel>
            <div className="divide-y divide-border">
              <BlockRow
                status="draft"
                text="The vendor contract renews in March and needs a decision by the 14th."
                timestamp="12:04"
              />
              <BlockRow
                status="approved"
                text="Security review moves to the following sprint."
                timestamp="36:12"
                active
              />
              <BlockRow
                status="rejected"
                text="Pricing was agreed at the meeting."
                timestamp="58:41"
              />
            </div>
          </SectionCard>
        </Group>

        <Group title="Notices" hint="§5.17 six tones + the Art. 50 note">
          <div className="space-y-2">
            <Notice
              tone="ai"
              as="note"
              aria-label="AI-generated content, human review required"
              title="AI-generated · review required"
            >
              This summary and its action items were generated by AI. Each item is linked to
              its source transcript segment and must be reviewed and approved by a person
              before it is finalized.
            </Notice>
            <Notice tone="consent" title="You are responsible for participant consent">
              Recording laws differ by jurisdiction. Inform everyone in the room before you
              start.
            </Notice>
            <Notice
              tone="warning"
              as="status"
              title="Local database is not encrypted"
              action={
                <Button variant="outline" size="sm">
                  Fix
                </Button>
              }
            >
              Turn on encryption at rest in Settings → Privacy.
            </Notice>
            <Notice tone="info" title="Transcription runs on this device">
              Nothing leaves your machine unless you choose a cloud model.
            </Notice>
            <Notice tone="success" title="Summary approved">
              Exported to Markdown 2 minutes ago.
            </Notice>
            <Notice tone="destructive" title="Couldn’t load the summary draft">
              The local database is locked by another window.
            </Notice>
          </div>
        </Group>

        <Group title="Stat tiles" hint="§5.11 value · sub · empty · loading">
          <StatTileGroup className="grid-cols-2 sm:grid-cols-2 lg:grid-cols-3">
            <StatTile icon={Timer} label="Duration" value="1h 24m" />
            <StatTile icon={FileText} label="Words" value="12 480" sub="142 wpm" />
            <StatTile icon={ListChecks} label="Approved" value="3 / 7" />
            <StatTile icon={Users} label="Speakers" />
            <StatTile icon={Calendar} label="Segments" loading />
            <StatTile icon={Mic} label="Sample rate" value="48 kHz" />
          </StatTileGroup>
        </Group>

        <Group title="Progress" hint="§5.16 determinate · error · indeterminate">
          <div className="space-y-3 rounded-lg border border-border bg-card p-4">
            <div className="space-y-1.5">
              <div className="flex items-baseline justify-between text-caption tabular-nums text-muted-foreground">
                <span>large-v3</span>
                <span>142 MB / 670 MB · 4.2 MB/s · ~2 min left</span>
              </div>
              <Progress value={42} aria-label="Downloading large-v3" />
            </div>
            <div className="space-y-1.5">
              <p className="text-caption text-destructive-ink">
                Download failed — check your connection and retry.
              </p>
              <Progress value={68} tone="destructive" aria-label="Download failed at 68%" />
            </div>
            <div className="space-y-1.5">
              <p className="text-caption text-muted-foreground">Preparing the model…</p>
              <Progress indeterminate aria-label="Preparing the model" />
            </div>
          </div>
        </Group>

        <Group title="Empty states" hint="§5.12 first-run · blocked">
          <div className="rounded-lg border border-border bg-card">
            <EmptyState
              icon={Inbox}
              title="No action items yet"
              description="Approved action items from your meetings collect here."
              action={<Button size="sm">Record a meeting</Button>}
              link={
                <Button variant="link" size="sm">
                  How review works
                </Button>
              }
            />
          </div>
          <div className="rounded-lg border border-border bg-card">
            <EmptyState
              variant="blocked"
              icon={Search}
              title="No model selected"
              description="Choose a transcription model before you record."
              action={
                <Button size="sm" variant="outline">
                  Choose a model in Settings
                </Button>
              }
            />
          </div>
        </Group>

        <Group title="Skeletons" hint="§5.19 role=status + sr-only sentence">
          <div className="grid gap-3 sm:grid-cols-2">
            <div className="rounded-lg border border-border bg-card p-3">
              <MeetingListSkeleton rows={3} />
            </div>
            <div className="rounded-lg border border-border bg-card p-3">
              <TranscriptSkeleton rows={4} />
            </div>
          </div>
          <Skeleton className="h-8 w-40" />
        </Group>

        <Group title="Keys" hint="§3.4 keyboard map">
          <div className="flex flex-wrap items-center gap-3 text-body text-muted-foreground">
            <span className="inline-flex items-center gap-2">
              Command palette
              <KbdGroup>
                <Kbd>⌘</Kbd>
                <Kbd>K</Kbd>
              </KbdGroup>
            </span>
            <span className="inline-flex items-center gap-2">
              Shortcuts
              <Kbd>?</Kbd>
            </span>
          </div>
        </Group>
      </div>
    </div>
  );
}

export default function PrimitivesFixturePage() {
  return (
    <div className="grid w-full grid-cols-1 lg:grid-cols-2">
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
