import { ReactNode } from "react";

interface BottomNavigationProps {
  children: ReactNode;
}

export const BottomNavigation = ({ children }: BottomNavigationProps) => {
  return (
    <div className="absolute bottom-0 w-full h-56 bg-white/30 z-100">
      {children}
    </div>
  );
};
