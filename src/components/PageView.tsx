import { useEffect, useRef, useState } from "react";

import { api, type Doc } from "../api";
import { Editor, type SaveStatus } from "./Editor";

const TITLE_SAVE_DELAY_MS = 400;

interface PageViewProps {
  doc: Doc;
  onTitleChange: (id: string, title: string) => void;
  onError: (e: unknown) => void;
}

/** One page: editable title + block editor. Remounted per page (keyed by id). */
export function PageView({ doc, onTitleChange, onError }: PageViewProps) {
  const [title, setTitle] = useState(doc.title);
  const [status, setStatus] = useState<SaveStatus>("saved");
  const pendingTitle = useRef<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const titleRef = useRef<HTMLTextAreaElement>(null);

  const flushTitle = () => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    const value = pendingTitle.current;
    if (value === null) return;
    pendingTitle.current = null;
    api.updateDocumentTitle(doc.id, value).catch(onError);
  };

  // Save a pending title when switching pages.
  useEffect(() => () => flushTitle(), []); // eslint-disable-line react-hooks/exhaustive-deps

  // New, untitled pages start with the cursor in the title.
  useEffect(() => {
    if (!doc.title) titleRef.current?.focus();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-grow the title textarea.
  useEffect(() => {
    const el = titleRef.current;
    if (el) {
      el.style.height = "auto";
      el.style.height = `${el.scrollHeight}px`;
    }
  }, [title]);

  const changeTitle = (value: string) => {
    const clean = value.replace(/\n/g, "");
    setTitle(clean);
    onTitleChange(doc.id, clean);
    pendingTitle.current = clean;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(flushTitle, TITLE_SAVE_DELAY_MS);
  };

  return (
    <article className="page">
      <div className="page-status" aria-live="polite">
        {status === "unsaved" || status === "saving" ? "Saving…" : status === "error" ? "Not saved" : "Saved"}
      </div>
      <textarea
        ref={titleRef}
        className="page-title"
        value={title}
        placeholder="Untitled"
        rows={1}
        maxLength={255}
        onChange={(e) => changeTitle(e.target.value)}
        onBlur={flushTitle}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.preventDefault();
        }}
        aria-label="Page title"
      />
      <Editor documentId={doc.id} onError={onError} onStatus={setStatus} />
    </article>
  );
}
