import { createContext, useContext } from "react";

import type { Doc, SpaceApi } from "./api";

/** Page list + navigation, for components rendered inside the editor. */
export interface PagesContextValue {
  /** Note commands for the current space. */
  api: SpaceApi;
  /** Non-archived pages of the current workspace, by ID. */
  docsById: Map<string, Doc>;
  openPage: (id: string) => void;
  /** Create a page nested under `parentId`; resolves to null on failure (error is reported). */
  createSubpage: (parentId: string) => Promise<Doc | null>;
  /** Move a page (and its nested pages) to the trash. */
  trashPage: (id: string) => Promise<void>;
  /** Bring a page (and its nested pages) back from the trash. `quiet` suppresses errors. */
  restorePage: (id: string, opts?: { quiet?: boolean }) => Promise<void>;
}

export const PagesContext = createContext<PagesContextValue | null>(null);

export function usePages(): PagesContextValue {
  const ctx = useContext(PagesContext);
  if (!ctx) throw new Error("usePages must be used inside <PagesContext.Provider>");
  return ctx;
}
