import { useCallback, useEffect, useMemo, useState } from "react";

import type { Doc } from "../api";
import { usePages } from "../pagesContext";
import { TrashIcon } from "./Sidebar";

interface TrashProps {
  workspaceId: string;
  /** Called after a restore with the restored pages and the ID that was restored. */
  onRestored: (restored: Doc[], id: string) => void;
  onClose: () => void;
  onError: (e: unknown) => void;
}

const relativeTime = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

function deletedAgo(iso: string): string {
  const seconds = (Date.parse(iso) - Date.now()) / 1000;
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
  ];
  for (const [unit, size] of units) {
    if (Math.abs(seconds) >= size) return relativeTime.format(Math.round(seconds / size), unit);
  }
  return "just now";
}

/** Trashed pages: restore them, or delete them for good. */
export function Trash({ workspaceId, onRestored, onClose, onError }: TrashProps) {
  const { api } = usePages();
  const [archived, setArchived] = useState<Doc[] | null>(null);
  const [confirmingId, setConfirmingId] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(() => {
    api.listArchivedDocuments(workspaceId).then(setArchived, onError);
  }, [api, workspaceId, onError]);

  useEffect(load, [load]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Show each deleted page once: nested pages travel with their trashed parent.
  const { roots, nestedCount } = useMemo(() => {
    const list = archived ?? [];
    const ids = new Set(list.map((d) => d.id));
    const children = new Map<string, string[]>();
    for (const d of list) {
      if (d.parentId && ids.has(d.parentId)) children.set(d.parentId, [...(children.get(d.parentId) ?? []), d.id]);
    }
    const count = (id: string): number =>
      (children.get(id) ?? []).reduce((n, child) => n + 1 + count(child), 0);
    return {
      roots: list.filter((d) => !d.parentId || !ids.has(d.parentId)),
      nestedCount: count,
    };
  }, [archived]);

  const restore = async (id: string) => {
    setBusyId(id);
    try {
      onRestored(await api.restoreDocument(id), id);
    } catch (e) {
      onError(e);
      setBusyId(null);
    }
  };

  const deleteForever = async (id: string) => {
    setBusyId(id);
    try {
      await api.deleteDocumentPermanently(id);
      load();
    } catch (e) {
      onError(e);
    } finally {
      setBusyId(null);
      setConfirmingId(null);
    }
  };

  return (
    <div className="overlay" onMouseDown={onClose}>
      <section
        className="trash-panel"
        role="dialog"
        aria-modal="true"
        aria-label="Trash"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="trash-header">
          <TrashIcon />
          <h2>Trash</h2>
          <button className="icon-button" onClick={onClose} aria-label="Close trash">
            ×
          </button>
        </header>

        {archived === null ? (
          <p className="trash-empty">Loading…</p>
        ) : roots.length === 0 ? (
          <p className="trash-empty">The trash is empty.</p>
        ) : (
          <ul className="trash-list">
            {roots.map((doc) => {
              const nested = nestedCount(doc.id);
              const busy = busyId === doc.id;
              return (
                <li key={doc.id} className="trash-row">
                  <div className="trash-info">
                    <span className={doc.title ? "trash-title" : "trash-title placeholder"}>
                      {doc.title || "Untitled"}
                    </span>
                    <span className="trash-meta">
                      Deleted {deletedAgo(doc.updatedAt)}
                      {nested > 0 && ` · ${nested} nested page${nested === 1 ? "" : "s"}`}
                    </span>
                  </div>
                  {confirmingId === doc.id ? (
                    <>
                      <button className="danger-button" disabled={busy} onClick={() => deleteForever(doc.id)}>
                        Delete forever
                      </button>
                      <button className="text-button" disabled={busy} onClick={() => setConfirmingId(null)}>
                        Cancel
                      </button>
                    </>
                  ) : (
                    <>
                      <button className="text-button" disabled={busy} onClick={() => restore(doc.id)}>
                        Restore
                      </button>
                      <button
                        className="icon-button"
                        disabled={busy}
                        title="Delete forever"
                        aria-label="Delete forever"
                        onClick={() => setConfirmingId(doc.id)}
                      >
                        <TrashIcon />
                      </button>
                    </>
                  )}
                </li>
              );
            })}
          </ul>
        )}
        <footer className="trash-footer">Restoring a page also restores the pages inside it.</footer>
      </section>
    </div>
  );
}
