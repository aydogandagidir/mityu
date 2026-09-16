interface StatusOverlaysProps {
  // Status flags
  isProcessing: boolean;      // Processing transcription after recording stops
  isSaving: boolean;          // Saving transcript to database

  /**
   * @deprecated Kept so existing call sites compile unchanged; it is ignored. The shell
   * is a flex row, so the overlay is positioned inside the content pane and no longer
   * mirrors the sidebar's width.
   */
  sidebarCollapsed?: boolean;
}

// Internal reusable component for individual status overlays
interface StatusOverlayProps {
  show: boolean;
  message: string;
}

/** A spinner that still says something when motion is reduced (§4.9). */
function Spinner() {
  return (
    <span
      aria-hidden="true"
      className="size-4 shrink-0 rounded-full border-2 border-border border-t-primary motion-safe:animate-spin"
    />
  );
}

function StatusOverlay({ show, message }: StatusOverlayProps) {
  if (!show) return null;

  // Absolute inside the content pane, not fixed to the window: the shell is a flex row
  // now, so this no longer has to mirror the sidebar's width to avoid sitting under it.
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center px-gutter">
      <div
        role="status"
        aria-live="polite"
        className="pointer-events-auto flex items-center gap-2 rounded-md border border-border bg-card px-4 py-2 shadow-elev-2"
      >
        <Spinner />
        <span className="text-body text-foreground">{message}</span>
      </div>
    </div>
  );
}

// Main exported component - renders multiple status overlays
export function StatusOverlays({
  isProcessing,
  isSaving,
}: StatusOverlaysProps) {
  return (
    <>
      {/* Processing status overlay - shown after recording stops while finalizing transcription */}
      <StatusOverlay show={isProcessing} message="Finalizing transcription..." />

      {/* Saving status overlay - shown while saving transcript to database */}
      <StatusOverlay show={isSaving} message="Saving transcript..." />
    </>
  );
}
