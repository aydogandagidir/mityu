import { useCallback } from 'react';
import { Transcript } from '@/types';
import { toast } from 'sonner';
import Analytics from '@/lib/analytics';
import {
  fetchAllTranscripts as fetchAllTranscriptsShared,
  formatTime,
} from '@/lib/transcriptTimestamps';

interface UseCopyOperationsProps {
  meeting: any;
  meetingTitle: string;
}

export function useCopyOperations({
  meeting,
  meetingTitle,
}: UseCopyOperationsProps) {

  // Helper function to fetch ALL transcripts for copying (not just paginated data).
  // Delegates to the shared `@/lib/transcriptTimestamps` fetch-all so the copy and
  // export flows read transcripts identically; keeps the copy-flow logging + toast.
  const fetchAllTranscripts = useCallback(async (meetingId: string): Promise<Transcript[]> => {
    try {
      console.log('📊 Fetching all transcripts for copying:', meetingId);
      const transcripts = await fetchAllTranscriptsShared(meetingId);
      console.log(`✅ Fetched ${transcripts.length} transcripts from database for copying`);
      return transcripts;
    } catch (error) {
      console.error('❌ Error fetching all transcripts:', error);
      toast.error('Failed to fetch transcripts for copying');
      return [];
    }
  }, []);

  // Copy transcript to clipboard
  const handleCopyTranscript = useCallback(async () => {
    // CHANGE: Fetch ALL transcripts from database, not from pagination state
    console.log('📊 Fetching all transcripts for copying...');
    const allTranscripts = await fetchAllTranscripts(meeting.id);

    if (!allTranscripts.length) {
      const error_msg = 'No transcripts available to copy';
      console.log(error_msg);
      toast.error(error_msg);
      return;
    }

    console.log(`✅ Copying ${allTranscripts.length} transcripts to clipboard`);

    // Timestamps are recording-relative [MM:SS] (wall-clock fallback for legacy
    // rows) via the shared `formatTime` — same helper the export flow uses.
    const header = `# Transcript of the Meeting: ${meeting.id} - ${meetingTitle ?? meeting.title}\n\n`;
    const date = `## Date: ${new Date(meeting.created_at).toLocaleDateString()}\n\n`;
    const fullTranscript = allTranscripts
      .map(t => `${formatTime(t.audio_start_time, t.timestamp)} ${t.text}  `)
      .join('\n');

    await navigator.clipboard.writeText(header + date + fullTranscript);
    toast.success("Transcript copied to clipboard");

    // Track copy analytics
    const wordCount = allTranscripts
      .map(t => t.text.split(/\s+/).length)
      .reduce((a, b) => a + b, 0);

    await Analytics.trackCopy('transcript', {
      transcript_length: allTranscripts.length.toString(),
      word_count: wordCount.toString()
    });
  }, [meeting, meetingTitle, fetchAllTranscripts]);

  return {
    handleCopyTranscript,
  };
}
