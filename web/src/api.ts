// Typed wrappers around the Rust commands (apps/desktop/src/commands).
// Note commands always run against one space (`spaceApi(spaceId)`).
// Argument names are camelCase here; Tauri maps them to the Rust snake_case names.
//
// Two transports, chosen at build time:
// - desktop: Tauri `invoke`
// - web (`vite --mode web`): `POST /api/<command>` on note-server, same names and JSON
import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export const IS_WEB = import.meta.env.MODE === "web";

/** Fired when the server says the session is gone; the app shows the sign-in screen. */
export const SIGNED_OUT_EVENT = "note-taker:signed-out";

async function post<T>(path: string, args: Record<string, unknown>): Promise<T> {
  const res = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", "X-Requested-With": "note-taker" },
    body: JSON.stringify(args),
  });
  const text = await res.text();
  const body = text ? JSON.parse(text) : undefined;
  if (!res.ok) {
    if (res.status === 401 && body?.kind === "unauthenticated") window.dispatchEvent(new Event(SIGNED_OUT_EVENT));
    throw body ?? { kind: "internal", message: `Server error ${res.status}` };
  }
  return body as T;
}

function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return IS_WEB ? post<T>(`/api/${command}`, args) : tauriInvoke<T>(command, args);
}

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

/** Shape of errors returned by commands (see crates/note-core/src/error.rs). */
export interface ApiError {
  kind:
    | "database"
    | "notFound"
    | "unauthenticated"
    | "invalidCredentials"
    | "invalid"
    | "internal"
    | "forbidden"
    | "rateLimited";
  message: string;
}

export function errorMessage(e: unknown): string {
  if (e && typeof e === "object" && "message" in e) return String((e as ApiError).message);
  return String(e);
}

export interface Space {
  id: string;
  name: string;
  /** `#rrggbb` */
  color: string;
  /** The space's SQLite file. */
  path: string;
}

/** On the web, the signed-in account is the one and only space. */
const WEB_SPACE: Space = { id: "server", name: "Notes", color: "#3b82f6", path: "" };

/** Spaces: separate sets of notes, each in its own database file (desktop only). */
export const spacesApi = {
  list: () => (IS_WEB ? Promise.resolve([WEB_SPACE]) : invoke<Space[]>("list_spaces")),
  /** `path`: an existing notes file to open, or a new file to create; omit for the default location. */
  create: (name: string, color: string, path: string | null) => invoke<Space>("create_space", { name, color, path }),
  update: (id: string, name: string, color: string) => invoke<Space>("update_space", { id, name, color }),
  /** Removes the space from the list; its file stays on disk. */
  remove: (id: string) => invoke<void>("remove_space", { id }),
  /** Copies the file to `path`, verifies it and switches to it; the old file is kept. */
  moveFile: (id: string, path: string) => invoke<Space>("move_space_file", { id, path }),
};

/** All note commands, bound to one space. */
export function spaceApi(spaceId: string) {
  const call = <T>(command: string, args: Record<string, unknown> = {}) => invoke<T>(command, { spaceId, ...args });
  return {
    getCurrentUser: () => call<User>("get_current_user"),

    listWorkspaces: () => call<Workspace[]>("list_workspaces"),
    createWorkspace: (name: string) => call<Workspace>("create_workspace", { name }),

    listDocuments: (workspaceId: string) => call<Doc[]>("list_documents", { workspaceId }),
    getDocument: (id: string) => call<Doc>("get_document", { id }),
    createDocument: (workspaceId: string, title: string, parentId: string | null) =>
      call<Doc>("create_document", { workspaceId, title, parentId }),
    updateDocumentTitle: (id: string, title: string) => call<Doc>("update_document_title", { id, title }),
    /** Archives the page and all nested pages; resolves to the archived IDs. */
    archiveDocument: (id: string) => call<string[]>("archive_document", { id }),
    moveDocument: (id: string, parentId: string | null) => call<Doc>("move_document", { id, parentId }),

    /** Trash: archived pages, most recently deleted first. */
    listArchivedDocuments: (workspaceId: string) => call<Doc[]>("list_archived_documents", { workspaceId }),
    /** Restores the page and its nested pages; resolves to the restored pages. */
    restoreDocument: (id: string) => call<Doc[]>("restore_document", { id }),
    /** Permanently deletes a trashed page and its nested pages; resolves to the deleted IDs. */
    deleteDocumentPermanently: (id: string) => call<string[]>("delete_document_permanently", { id }),

    getDocumentBlocks: (documentId: string) => call<StoredBlock[]>("get_document_blocks", { documentId }),
    saveDocumentBlocks: (documentId: string, blocks: BlockInput[]) =>
      call<void>("save_document_blocks", { documentId, blocks }),
  };
}

export type SpaceApi = ReturnType<typeof spaceApi>;

export interface AuthStatus {
  /** No accounts yet: the first visitor creates one. */
  needsSetup: boolean;
  registrationOpen: boolean;
  user: User | null;
}

/** Sign-in for the web app (note-server). */
export const authApi = {
  status: async (): Promise<AuthStatus> => {
    const res = await fetch("/api/auth/status", { credentials: "same-origin" });
    if (!res.ok) throw { kind: "internal", message: `Server error ${res.status}` };
    return res.json();
  },
  setup: (email: string, displayName: string, password: string) =>
    post<User>("/api/auth/setup", { email, displayName, password }),
  register: (email: string, displayName: string, password: string) =>
    post<User>("/api/auth/register", { email, displayName, password }),
  login: (email: string, password: string) => post<User>("/api/auth/login", { email, password }),
  logout: () => post<void>("/api/auth/logout", {}),
};
