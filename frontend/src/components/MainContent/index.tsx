'use client';

import React from 'react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';

interface MainContentProps {
  children: React.ReactNode;
}

const MainContent: React.FC<MainContentProps> = ({ children }) => {
  const { isCollapsed } = useSidebar();

  return (
    // No inner pl-8 wrapper: it painted a page-background strip along the
    // sidebar's right edge that read as a broken white band (screenshot bug).
    // Pages own their padding.
    <main
      className={`flex-1 transition-[margin] duration-300 ${
        isCollapsed ? 'ml-16' : 'ml-64'
      }`}
    >
      {children}
    </main>
  );
};

export default MainContent;
