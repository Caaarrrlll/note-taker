import { useCallback, useEffect, useMemo, useState } from "react";

import { api, errorMessage, type Doc, type Workspace } from "./api";
import { storage } from "./hooks";
import { PagesContext, type PagesContextValue } from "./pagesContext";
import { Sidebar } from "./components/Sidebar";
import { PageView } from "./components/PageView";

const LAST_PAGE_KEY = "lastPageId";

export default function App() {
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [docs, setDocs] = useState<Doc[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const reportError = useCallback((e: unknown) => setError(errorMessage(e)), []);

  // Initial load: the local user's first workspace and its pages.
  useEffect(() => {
    (async () => {
      try {
        const [ws] = await api.listWorkspaces();
        if (!ws) throw new Error("No workspace found");
        const list = await api.listDocuments(ws.id);
        setWorkspace(ws);
        setDocs(list);
        const last = storage.get(LAST_PAGE_KEY);
        setSelectedId(list.find((d) => d.id === last)?.id ?? list[0]?.id ?? null);
      } catch (e) {
        reportError(e);
      } finally {
        setLoading(false);
      }
    })();
  }, [reportError]);

  useEffect(() => {
    if (selectedId) storage.set(LAST_PAGE_KEY, selectedId);
  }, [selectedId]);

  const createDocument = useCallback(
    async (parentId: string | null): Promise<Doc | null> => {
      if (!workspace) return null;
      try {
        const doc = await api.createDocument(workspace.id, "", parentId);
        setDocs((prev) => [...prev, doc]);
        return doc;
      } catch (e) {
        reportError(e);
        return null;
      }
    },
    [workspace, reportError],
  );

  const createPage = useCallback(
    async (parentId: string | null) => {
      const doc = await createDocument(parentId);
      if (doc) setSelectedId(doc.id);
    },
    [createDocument],
  );

  const archivePage = useCallback(
    async (id: string) => {
      try {
        const archived = new Set(await api.archiveDocument(id));
        setDocs((prev) => {
          const remaining = prev.filter((d) => !archived.has(d.id));
          setSelectedId((sel) => (sel && archived.has(sel) ? (remaining[0]?.id ?? null) : sel));
          return remaining;
        });
      } catch (e) {
        reportError(e);
      }
    },
    [reportError],
  );

  // Local title update (PageView persists it); keeps the sidebar in sync while typing.
  const setTitle = useCallback((id: string, title: string) => {
    setDocs((prev) => prev.map((d) => (d.id === id ? { ...d, title } : d)));
  }, []);

  const selected = docs.find((d) => d.id === selectedId) ?? null;

  const pagesContext = useMemo<PagesContextValue>(
    () => ({
      docsById: new Map(docs.map((d) => [d.id, d])),
      openPage: setSelectedId,
      createSubpage: (parentId) => createDocument(parentId),
    }),
    [docs, createDocument],
  );

  return (
    <PagesContext.Provider value={pagesContext}>
      <div className="app">
        <Sidebar
          workspaceName={workspace?.name ?? ""}
          docs={docs}
          selectedId={selectedId}
          onSelect={setSelectedId}
          onCreate={createPage}
          onArchive={archivePage}
        />
        <main className="main">
          {error && (
            <div className="error-banner" role="alert">
              <span>{error}</span>
              <button className="icon-button" onClick={() => setError(null)} aria-label="Dismiss error">
                ×
              </button>
            </div>
          )}
          {loading ? null : selected ? (
            <PageView key={selected.id} doc={selected} onTitleChange={setTitle} onError={reportError} />
          ) : (
            <div className="empty-state">
              <p>No pages yet.</p>
              <button className="primary-button" onClick={() => createPage(null)} disabled={!workspace}>
                Create your first page
              </button>
            </div>
          )}
        </main>
      </div>
    </PagesContext.Provider>
  );
}
