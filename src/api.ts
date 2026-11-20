// Typed wrappers around the Rust commands (src-tauri/src/commands).
// Argument names are camelCase here; Tauri maps them to the Rust snake_case names.
import { invoke } from "@tauri-apps/api/core";

export interface User {
  id: string;
  email: string;
  displayName: string;
  isLocal: boolean;
}

export interface Workspace {
  id: string;
  name: string;
}

export interface Doc {
  id: string;
  title: string;
  workspaceId: string;
  parentId: string | null;
  isArchived: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface StoredBlock {
  id: string;
  blockType: string;
  /** The editor's JSON for this block (including nested children). */
  content: unknown;
}

export interface BlockInput {
  id?: string;
  blockType: string;
  content: unknown;
}

/** Shape of errors returned by commands (see src-tauri/src/error.rs). */
export interface ApiError {
  kind: "database" | "notFound" | "unauthenticated" | "invalidCredentials" | "invalid" | "internal";
  message: string;
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as ApiError).message);
  return String(e);
}

export const api = {
  getCurrentUser: () => invoke<User>("get_current_user"),

  listWorkspaces: () => invoke<Workspace[]>("list_workspaces"),
  createWorkspace: (name: string) => invoke<Workspace>("create_workspace", { name }),

  listDocuments: (workspaceId: string) => invoke<Doc[]>("list_documents", { workspaceId }),
  getDocument: (id: string) => invoke<Doc>("get_document", { id }),
  createDocument: (workspaceId: string, title: string, parentId: string | null) =>
    invoke<Doc>("create_document", { workspaceId, title, parentId }),
  updateDocumentTitle: (id: string, title: string) => invoke<Doc>("update_document_title", { id, title }),
  /** Archives the page and all nested pages; resolves to the archived IDs. */
  archiveDocument: (id: string) => invoke<string[]>("archive_document", { id }),
  moveDocument: (id: string, parentId: string | null) => invoke<Doc>("move_document", { id, parentId }),

  getDocumentBlocks: (documentId: string) => invoke<StoredBlock[]>("get_document_blocks", { documentId }),
  saveDocumentBlocks: (documentId: string, blocks: BlockInput[]) =>
    invoke<void>("save_document_blocks", { documentId, blocks }),
};
