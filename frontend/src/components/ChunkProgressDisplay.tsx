import React from 'react';

export interface ChunkStatus {
  chunk_id: number;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  start_time?: number;
  end_time?: number;
  duration_ms?: number;
  text_preview?: string;
  error_message?: string;
}

export interface ProcessingProgress {
  total_chunks: number;
  completed_chunks: number;
  processing_chunks: number;
  failed_chunks: number;
  estimated_remaining_ms?: number;
  chunks: ChunkStatus[];
}

interface ChunkProgressDisplayProps {
  progress: ProcessingProgress;
  onPause?: () => void;
  onResume?: () => void;
  onCancel?: () => void;
  isPaused?: boolean;
  className?: string;
}

export function ChunkProgressDisplay({
  progress,
  onPause,
  onResume,
  onCancel,
  isPaused = false,
  className = ''
}: ChunkProgressDisplayProps) {
  const completionPercentage = progress.total_chunks > 0
    ? Math.round((progress.completed_chunks / progress.total_chunks) * 100)
    : 0;

  const formatDuration = (ms: number) => {
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);

    if (hours > 0) {
      return `${hours}h ${minutes % 60}m ${seconds % 60}s`;
    } else if (minutes > 0) {
      return `${minutes}m ${seconds % 60}s`;
    } else {
      return `${seconds}s`;
    }
  };

  const formatTimeRemaining = (ms?: number) => {
    if (!ms || ms <= 0) return 'Calculating...';
    return formatDuration(ms);
  };

  const getChunkStatusIcon = (status: ChunkStatus['status']) => {
    switch (status) {
      case 'completed':
        return '✅';
      case 'processing':
        return '⚡';
      case 'failed':
        return '❌';
      case 'pending':
      default:
        return '⏳';
    }
  };

  const getChunkStatusColor = (status: ChunkStatus['status']) => {
    switch (status) {
      case 'completed':
        return 'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-500/10 border-green-200 dark:border-green-500/25';
      case 'processing':
        return 'text-primary bg-accent border-primary/20';
      case 'failed':
        return 'text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-500/10 border-red-200 dark:border-red-500/25';
      case 'pending':
      default:
        return 'text-muted-foreground bg-muted border-border';
    }
  };

  return (
    <div className={`bg-card border border-border rounded-lg p-4 ${className}`}>
      {/* Progress Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-3">
          <h3 className="text-lg font-semibold text-foreground">
            Processing Progress
          </h3>
          {isPaused && (
            <span className="bg-yellow-100 dark:bg-yellow-500/15 text-yellow-800 dark:text-yellow-200 px-2 py-1 rounded-full text-xs font-medium">
              Paused
            </span>
          )}
        </div>

        <div className="flex items-center space-x-2">
          {!isPaused ? (
            <button
              onClick={onPause}
              className="bg-yellow-500 hover:bg-yellow-600 text-white px-3 py-1 rounded text-sm transition-colors"
              disabled={progress.processing_chunks === 0 && progress.completed_chunks === progress.total_chunks}
            >
              Pause
            </button>
          ) : (
            <button
              onClick={onResume}
              className="bg-green-500 hover:bg-green-600 text-white px-3 py-1 rounded text-sm transition-colors"
            >
              Resume
            </button>
          )}

          <button
            onClick={onCancel}
            className="bg-red-500 hover:bg-red-600 text-white px-3 py-1 rounded text-sm transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>

      {/* Progress Bar */}
      <div className="mb-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-medium text-foreground">
            {progress.completed_chunks} of {progress.total_chunks} chunks completed
          </span>
          <span className="text-sm font-medium text-foreground">
            {completionPercentage}%
          </span>
        </div>

        <div className="w-full bg-secondary rounded-full h-2">
          <div
            className="bg-primary h-2 rounded-full transition-all duration-300 ease-out"
            style={{ width: `${completionPercentage}%` }}
          />
        </div>
      </div>

      {/* Processing Stats */}
      <div className="grid grid-cols-4 gap-4 mb-4 text-sm">
        <div className="text-center">
          <div className="text-lg font-semibold text-green-600 dark:text-green-400">
            {progress.completed_chunks}
          </div>
          <div className="text-muted-foreground">Completed</div>
        </div>

        <div className="text-center">
          <div className="text-lg font-semibold text-primary">
            {progress.processing_chunks}
          </div>
          <div className="text-muted-foreground">Processing</div>
        </div>

        <div className="text-center">
          <div className="text-lg font-semibold text-muted-foreground">
            {progress.total_chunks - progress.completed_chunks - progress.processing_chunks - progress.failed_chunks}
          </div>
          <div className="text-muted-foreground">Pending</div>
        </div>

        <div className="text-center">
          <div className="text-lg font-semibold text-red-600 dark:text-red-400">
            {progress.failed_chunks}
          </div>
          <div className="text-muted-foreground">Failed</div>
        </div>
      </div>

      {/* Time Estimate */}
      {progress.estimated_remaining_ms && progress.estimated_remaining_ms > 0 && (
        <div className="bg-accent border border-primary/20 rounded-lg p-3 mb-4">
          <div className="flex items-center space-x-2">
            <span className="text-primary">⏱️</span>
            <span className="text-sm text-primary">
              Estimated time remaining: {formatTimeRemaining(progress.estimated_remaining_ms)}
            </span>
          </div>
        </div>
      )}

      {/* Recent Chunks Grid */}
      <div className="space-y-2">
        <h4 className="text-sm font-medium text-foreground mb-2">
          Recent Chunks ({Math.min(progress.chunks.length, 10)} of {progress.total_chunks})
        </h4>

        <div className="max-h-48 overflow-y-auto space-y-1">
          {progress.chunks
            .slice(-10) // Show last 10 chunks
            .reverse() // Most recent first
            .map((chunk) => (
              <div
                key={chunk.chunk_id}
                className={`text-xs p-2 rounded border ${getChunkStatusColor(chunk.status)}`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center space-x-2">
                    <span>{getChunkStatusIcon(chunk.status)}</span>
                    <span className="font-medium">
                      Chunk {chunk.chunk_id}
                    </span>
                    {chunk.duration_ms && (
                      <span className="text-muted-foreground">
                        ({formatDuration(chunk.duration_ms)})
                      </span>
                    )}
                  </div>

                  {chunk.status === 'processing' && (
                    <div className="flex items-center space-x-1">
                      <div className="animate-spin w-3 h-3 border border-primary border-t-transparent rounded-full"></div>
                    </div>
                  )}
                </div>

                {chunk.text_preview && (
                  <div className="mt-1 text-foreground text-xs truncate">
                    "{chunk.text_preview}"
                  </div>
                )}

                {chunk.error_message && (
                  <div className="mt-1 text-red-700 dark:text-red-300 text-xs">
                    Error: {chunk.error_message}
                  </div>
                )}
              </div>
            ))}
        </div>
      </div>

      {/* Processing Complete */}
      {progress.completed_chunks === progress.total_chunks && progress.total_chunks > 0 && (
        <div className="mt-4 bg-green-50 dark:bg-green-500/10 border border-green-200 dark:border-green-500/25 rounded-lg p-3">
          <div className="flex items-center space-x-2">
            <span className="text-green-600 dark:text-green-400">🎉</span>
            <span className="text-sm font-medium text-green-800 dark:text-green-200">
              Processing completed! All {progress.total_chunks} chunks have been transcribed.
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

// Mini version for sidebar or compact display
export function ChunkProgressMini({ progress, className = '' }: { progress: ProcessingProgress; className?: string }) {
  const completionPercentage = progress.total_chunks > 0
    ? Math.round((progress.completed_chunks / progress.total_chunks) * 100)
    : 0;

  return (
    <div className={`bg-muted border border-border rounded-lg p-3 ${className}`}>
      <div className="flex items-center justify-between mb-2">
        <span className="text-sm font-medium text-foreground">
          Processing
        </span>
        <span className="text-sm font-medium text-foreground">
          {completionPercentage}%
        </span>
      </div>

      <div className="w-full bg-secondary rounded-full h-1.5 mb-2">
        <div
          className="bg-primary h-1.5 rounded-full transition-all duration-300"
          style={{ width: `${completionPercentage}%` }}
        />
      </div>

      <div className="text-xs text-muted-foreground">
        {progress.completed_chunks} / {progress.total_chunks} chunks
        {progress.processing_chunks > 0 && (
          <span className="ml-2 text-primary">
            ({progress.processing_chunks} processing)
          </span>
        )}
      </div>
    </div>
  );
}