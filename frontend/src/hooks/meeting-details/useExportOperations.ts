/**
 * useExportOperations (BACKLOG C4.1)
 *
 * The export counterpart to `useCopyOperations`, built on the SAME primitives:
 *  - the shared `fetchAllTranscripts` (all rows, not the paginated slice) so
 *    every block/action-item `source_chunk_id` can resolve its `[MM:SS]`;
 *  - `buildTimestampMap` + the pure `buildExportDoc` (approved-only filtering);
 *  - a format renderer (`renderExportMarkdown` for C4.1);
 *  - the EXISTING blob-download pattern (URL.createObjectURL + a.download +
 *    revokeObjectURL) — identical to `AISummary/index.tsx` `handleExport`. No
 *    Tauri dialog plugin, no new capability; blob download works offline in the
 *    webview.
 *
 * Everything here runs on LOCAL data (SQLite via the core) and produces the file
 * in-process, so it works with the network OFF by construction.
 *
 * The component calls `exportMarkdown()`; the hook owns the fetch, the model, the
 * render, the download, and the content-free `Analytics.trackExport`. No raw
 * `invoke` lives in the component.
 */

import { useCallback, useState } from 'react';
import { toast } from 'sonner';
import Analytics from '@/lib/analytics';
import {
  fetchAllTranscripts,
  buildTimestampMap,
} from '@/lib/transcriptTimestamps';
import { buildExportDoc, type ExportMeta } from '@/lib/exportModel';
import { renderExportMarkdown } from '@/lib/exportMarkdown';
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
 * Trigger a client-side download of `content` as a file, using the existing
 * blob-download pattern (offline-safe; no capability required).
 */
function downloadTextFile(content: string, filename: string, mimeType: string): void {
  const blob = new Blob([content], { type: mimeType });
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

  const exportMarkdown = useCallback(async (): Promise<void> => {
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
        // export (blocks/items just render without a [MM:SS]) rather than block
        // the user. Log the technical detail; keep the UI message friendly.
        console.error('[useExportOperations] Failed to fetch transcripts for timestamps:', err);
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

      // 3) Render Markdown and download it via the blob pattern.
      const markdown = renderExportMarkdown(doc);
      const filename = `${sanitizeFilename(meetingTitle)}-summary.md`;
      downloadTextFile(markdown, filename, 'text/markdown');

      toast.success('Summary exported to Markdown');

      // 4) Content-free analytics (opt-in gate lives inside Analytics).
      await Analytics.trackExport({ format: 'markdown', meetingId });
    } catch (err) {
      console.error('[useExportOperations] Markdown export failed:', err);
      toast.error("Couldn't export the summary", {
        description: 'Please try again in a moment.',
      });
    } finally {
      setIsExporting(false);
    }
  }, [meetingId, draftResponse, meetingTitle, meetingDate]);

  return { exportMarkdown, isExporting };
}
