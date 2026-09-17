'use client';

import React from 'react';

interface MainContentProps {
  children: React.ReactNode;
}

/**
 * The content pane.
 *
 * It no longer computes a margin from the sidebar's state. The shell is a flex row —
 * rail, pane, content — so the content pane is simply what is left, and it moves because
 * the pane's own width animates, not because two components animate the same number in
 * opposite directions and have to agree. That is what retires the 4rem / 16rem literals
 * duplicated here, in `app/page.tsx` and in `StatusOverlays`.
 *
 * `min-w-0` is load-bearing: without it one long transcript line makes this flex child
 * refuse to shrink and the whole window scrolls sideways.
 *
 * The height is `h-full`, NOT `h-screen`. The shell column is `h-screen` and holds two
 * children: this row and the 40px `SessionDock`. Asking for a full viewport here made the
 * pane 40px taller than the row that contains it, and the row's `overflow-hidden` ate the
 * difference — so while a recording was live the bottom 40px of every scrollable page was
 * silently clipped. `h-full` fills the row it is actually in.
 */
const MainContent: React.FC<MainContentProps> = ({ children }) => {
  return (
    <main className="flex h-full min-w-0 flex-1 flex-col overflow-hidden">
      {children}
    </main>
  );
};

export default MainContent;
