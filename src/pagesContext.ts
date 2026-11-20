import { createContext, useContext } from "react";

import type { Doc } from "./api";

/** Page list + navigation, for components rendered inside the editor. */
export interface PagesContextValue {
  /** Non-archived pages of the current workspace, by ID. */
  docsById: Map<string, Doc>;
  openPage: (id: string) => void;
  /** Create a page nested under `parentId`; resolves to null on failure (error is reported). */
  createSubpage: (parentId: string) => Promise<Doc | null>;
}

export const PagesContext = createContext<PagesContextValue | null>(null);

export function usePages(): PagesContextValue {
  const ctx = useContext(PagesContext);
  if (!ctx) throw new Error("usePages must be used inside <PagesContext.Provider>");
  return ctx;
}
