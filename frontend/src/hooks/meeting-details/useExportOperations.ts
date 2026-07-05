/**
 * useExportOperations (BACKLOG C4.1 Markdown; C4.2 DOCX + PDF)
 *
 * The export counterpart to `useCopyOperations`, built on the SAME primitives:
 *  - the shared `fetchAllTranscripts` (all rows, not the paginated slice) so
 *    every block/action-item `source_chunk_id` can resolve its `[MM:SS]`;
 *  - `buildTimestampMap` + the pure `buildExportDoc` (approved-only filtering);
 *  - a format renderer (`renderExportMarkdown`, `renderExportDocx`,
 *    `renderExportPdf`) — the ONLY thing that differs per format;
 *  - the EXISTING blob-download pattern (URL.createObjectURL + a.download +
 *    revokeObjectURL) — identical to `AISummary/index.tsx` `handleExport`. No
 *    Tauri dialog plugin, no new capability; blob download works offline in the
 *    webview.
 *
 * All three formats share ONE code path (`runExport`) that resolves timestamps,
 * builds the format-agnostic `ExportDoc`, guards the empty case, renders, and
 * downloads — so Markdown, Word, and PDF exports carry byte-for-byte the same
 * content, only the renderer + file extension change. The DOCX/PDF libraries are
 * pure-JS and loaded lazily inside their renderers, so nothing here breaks the
 * offline, `output: 'export'` static build.
 *
 * Everything runs on LOCAL data (SQLite via the core) and produces the file
 * in-process, so it works with the network OFF by construction.
 *
 * The component calls `exportMarkdown()` / `exportDocx()` / `exportPdf()`; the
 * hook owns the fetch, the model, the render, the download, and the content-free
 * `Analytics.trackExport`. No raw `invoke` lives in the component.
 */

import { useCallback, useState } from 'react';
import { toast } from 'sonner';
import Analytics from '@/lib/analytics';
import {
  fetchAllTranscripts,
  buildTimestampMap,
} from '@/lib/transcriptTimestamps';
import { buildExportDoc, type ExportDoc, type ExportMeta } from '@/lib/exportModel';
import { renderExportMarkdown } from '@/lib/exportMarkdown';
import { renderExportDocx } from '@/lib/exportDocx';
import { renderExportPdf } from '@/lib/exportPdf';
import type { SummaryDraftResponse } from '@/services/summaryDraftService';

/** Everything the export needs from the host component. */
interface UseExportOperationsProps {
  /** The meeting whose approved summary is being exported. */
  meetingId: string;
  /** The current draft payload (the SAME object the review surface renders). */
  draftResponse: SummaryDraftResponse | null;
  /** Human-readable meeting title (drives the header + filename). */
  meetingTitle?: string;
  /** Human-readable meeting date for the header, if known. */
  meetingDate?: string;
}

/** The formats this hook can emit; drives the renderer, extension, and analytics. */
type ExportFormat = 'markdown' | 'docx' | 'pdf';

/** Per-format wiring: file extension, MIME type, and success toast copy. */
const FORMAT_META: Record<
  ExportFormat,
  { extension: string; mimeType: string; successLabel: string }
> = {
  markdown: { extension: 'md', mimeType: 'text/markdown', successLabel: 'Markdown' },
  docx: {
    extension: 'docx',
    mimeType:
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    successLabel: 'Word (.docx)',
  },
  pdf: { extension: 'pdf', mimeType: 'application/pdf', successLabel: 'PDF' },
};

/**
 * Turn a meeting title into a safe, lowercase file stem. Falls back to
 * `meeting-summary` when the title is empty or sanitizes to nothing.
 */
function sanitizeFilename(title: string | undefined): string {
  const base = (title ?? '').trim();
  const cleaned = base
    // Drop characters illegal on Windows/macOS filesystems.
    .replace(/[\\/:*?"<>|]+/g, ' ')
    // Collapse whitespace to single dashes.
    .replace(/\s+/g, '-')
    // Trim stray dashes.
    .replace(/^-+|-+$/g, '')
    .toLowerCase();
  return cleaned.length > 0 ? cleaned : 'meeting-summary';
}

/**
 * Trigger a client-side download of a `Blob` as a file, using the existing
 * blob-download pattern (offline-safe; no capability required). A `string`
 * payload is wrapped in a `Blob` first, so text and binary formats share it.
 */
function downloadBlob(
  payload: Blob | string,
  filename: string,
  mimeType: string,
): void {
  const blob =
    typeof payload === 'string' ? new Blob([payload], { type: mimeType }) : payload;
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export function useExportOperations({
  meetingId,
  draftResponse,
  meetingTitle,
  meetingDate,
}: UseExportOperationsProps) {
  const [isExporting, setIsExporting] = useState(false);

  /**
   * The single, format-agnostic export path shared by all three formats: resolve
   * timestamps (best-effort) → build the approved-only doc → guard the empty
   * case → render with the format's renderer → blob-download → content-free
   * analytics. `render` is the ONLY per-format input (may be sync or async).
   */
  const runExport = useCallback(
    async (
      format: ExportFormat,
      render: (doc: ExportDoc) => Blob | string | Promise<Blob | string>,
    ): Promise<void> => {
      setIsExporting(true);
      try {
        // 1) Resolve source timestamps from ALL rows (fetch-all, not paginated
        //    state) so every source_chunk_id can map to its [MM:SS].
        let segmentTimestamps: Map<string, string>;
        try {
          const transcripts = await fetchAllTranscripts(meetingId);
          segmentTimestamps = buildTimestampMap(transcripts);
        } catch (err) {
          // Timestamps are best-effort: if the transcript read fails we still
          // export (blocks/items just render without a [MM:SS]) rather than
          // block the user. Log the technical detail; keep the UI message friendly.
          console.error(
            '[useExportOperations] Failed to fetch transcripts for timestamps:',
            err,
          );
          segmentTimestamps = new Map();
        }

        // 2) Build the format-agnostic doc (approved-only filtering happens here).
        const meta: ExportMeta = {
          meetingId,
          title: meetingTitle?.trim() || 'Meeting Summary',
          meetingDate,
          exportedAt: new Date().toLocaleString(),
          approvedAt: draftResponse?.approved_at ?? undefined,
          approvedBy: draftResponse?.approved_by ?? undefined,
          model: draftResponse?.model ?? undefined,
        };
        const doc = buildExportDoc(draftResponse, segmentTimestamps, meta);

        // Guard: nothing approved to emit (should be unreachable because the
        // control is gated on an approved summary, but keep it friendly).
        if (doc.sections.length === 0 && doc.actionItems.length === 0) {
          toast.error('Nothing approved to export yet');
          return;
        }

        // 3) Render the chosen format and download it via the blob pattern.
        const { extension, mimeType, successLabel } = FORMAT_META[format];
        const payload = await render(doc);
        const filename = `${sanitizeFilename(meetingTitle)}-summary.${extension}`;
        downloadBlob(payload, filename, mimeType);

        toast.success(`Summary exported to ${successLabel}`);

        // 4) Content-free analytics (opt-in gate lives inside Analytics).
        await Analytics.trackExport({ format, meetingId });
      } catch (err) {
        console.error(`[useExportOperations] ${format} export failed:`, err);
        toast.error("Couldn't export the summary", {
          description: 'Please try again in a moment.',
        });
      } finally {
        setIsExporting(false);
      }
    },
    [meetingId, draftResponse, meetingTitle, meetingDate],
  );

  const exportMarkdown = useCallback(
    (): Promise<void> => runExport('markdown', (doc) => renderExportMarkdown(doc)),
    [runExport],
  );

  const exportDocx = useCallback(
    (): Promise<void> => runExport('docx', (doc) => renderExportDocx(doc)),
    [runExport],
  );

  const exportPdf = useCallback(
    (): Promise<void> => runExport('pdf', (doc) => renderExportPdf(doc)),
    [runExport],
  );

  return { exportMarkdown, exportDocx, exportPdf, isExporting };
}
