import React from "react";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";

interface LogoProps {
    isCollapsed: boolean;
}

const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(({ isCollapsed }, ref) => {
  return (
    <Dialog aria-describedby={undefined}>
      {isCollapsed ? (
        <DialogTrigger asChild>
          <button ref={ref} className="flex items-center justify-start mb-2 cursor-pointer bg-transparent border-none p-0 hover:opacity-80 transition-opacity">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src="/mityu-mark.svg" alt="Mityu" width={32} height={32} />
          </button>
        </DialogTrigger>
      ) : (
        <DialogTrigger asChild>
          {/* Premium wordmark: product mark + gradient lowercase wordmark (bluedev
              brand DNA), a quiet hover surface instead of the old washed pill. */}
          <button className="group -mx-1 flex items-center gap-2 rounded-lg px-1 py-0.5 transition-colors hover:bg-muted">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src="/mityu-mark.svg" alt="" width={26} height={26} className="shrink-0" />
            <span className="bg-gradient-to-r from-primary to-[#4B78FF] bg-clip-text text-[17px] font-bold leading-none tracking-tight text-transparent">
              mityu
            </span>
          </button>
        </DialogTrigger>
      )}
      <DialogContent>
        <VisuallyHidden>
          <DialogTitle>About Mityu</DialogTitle>
        </VisuallyHidden>
        <About />
      </DialogContent>
    </Dialog>
  );
});

Logo.displayName = "Logo";

export default Logo;