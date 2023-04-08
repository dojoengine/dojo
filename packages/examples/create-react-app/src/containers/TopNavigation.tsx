import { ReactNode } from "react";

interface TopNavigationProps {
  children: ReactNode;
}

export const TopNavigation = ({ children }: TopNavigationProps) => {
  return (
    <div className="absolute bottom-0 w-full h-56 bg-white/30 z-100">
      {children}
    </div>
  );
};
