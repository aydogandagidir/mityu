/**
 * The scroller for every `/design/*` fixture route — DESIGN_SYSTEM.md §11.1/§11.2.
 *
 * WHY THIS FILE EXISTS. `globals.css` sets `body { overflow: hidden; height: 100% }` with
 * `html` left at `overflow: visible`, so the body's hidden overflow propagates to the
 * VIEWPORT: nothing on any route can grow the page and scroll it. Every product route
 * compensates by pairing `h-screen` with an inner `flex-1 overflow-y-auto`
 * (`app/settings/page.tsx`, `app/actions/page.tsx`). The fixture routes did not, so the
 * token matrix and the primitives gallery — which are several viewports tall each — were
 * unreachable below the fold in a real browser and absent from the §11.2 screenshots,
 * while the `--expect` markers kept passing because they are checked against `--dump-dom`,
 * which sees the whole tree regardless of visibility.
 *
 * One scroller here rather than one per page: the defect is the route group's, not any
 * single fixture's, and a page that manages its own height (`/design/tour`, which is
 * `h-screen` with inner panes) is unaffected by a container it exactly fills.
 *
 * WHAT THIS MEANS FOR §11.2's SCREENSHOTS. With the scroller INSIDE the page, the document
 * is exactly one viewport tall, so `Page.captureScreenshot{captureBeyondViewport:true}`
 * would capture nothing extra — the lever is the WINDOW, i.e. `shoot.py --height`. A PNG of
 * a fixture route is evidence for §11.2 only when it was shot tall enough to contain the
 * panel (the committed proofs used 900×3600); a default 1280×900 shot shows the top of one
 * panel and is not a proof that the rest rendered.
 */
export default function DesignFixtureLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <div className="h-screen overflow-y-auto">{children}</div>;
}
