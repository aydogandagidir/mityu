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
 */
const MainContent: React.FC<MainContentProps> = ({ children }) => {
  return (
    <main className="flex h-screen min-w-0 flex-1 flex-col overflow-hidden">
      {children}
    </main>
  );
};

export default MainContent;
