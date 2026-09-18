import React, { createContext, useContext } from "react";

type LayoutContextType = {
  width: number;
  height: number;
  isPortrait: boolean;
};

const LayoutContext = createContext<LayoutContextType | null>(null);

export const LayoutProvider: React.FC<{
  children: React.ReactNode;
  width: number;
  height: number;
}> = ({ children, width, height }) => {
  const isPortrait = height > width;
  return (
    <LayoutContext.Provider value={{ width, height, isPortrait }}>
      {children}
    </LayoutContext.Provider>
  );
};

export const useLayout = () => {
  const ctx = useContext(LayoutContext);
  if (!ctx) {
    throw new Error("useLayout must be used within a LayoutProvider");
  }
  return ctx;
};