import { useEffect, useRef, useState } from "react";
import { filterSuggestionItems, insertOrUpdateBlockForSlashMenu } from "@blocknote/core";
import {
  getDefaultReactSlashMenuItems,
  SuggestionMenuController,
  useCreateBlockNote,
  type DefaultReactSuggestionItem,
} from "@blocknote/react";
import { BlockNoteView } from "@blocknote/mantine";

import { PageIcon, schema, type AppPartialBlock } from "../editorSchema";
import { useColorScheme } from "../hooks";
import { usePages } from "../pagesContext";
import { registerFlusher, trackSave } from "../pendingSaves";

const SAVE_DELAY_MS = 600;

export type SaveStatus = "saved" | "unsaved" | "saving" | "error";

interface EditorProps {
  documentId: string;
  onError: (e: unknown) => void;
  onStatus: (s: SaveStatus) => void;
}

/** Loads a page's blocks, then mounts the editor with them. */
export function Editor({ documentId, onError, onStatus }: EditorProps) {
  const { api } = usePages();
  const [initial, setInitial] = useState<AppPartialBlock[] | null>(null);

  useEffect(() => {
    let cancelled = false;
    setInitial(null);
    api
      .getDocumentBlocks(documentId)
      .then((blocks) => {
        if (!cancelled) setInitial(blocks.map((b) => b.content as AppPartialBlock));
      })
      .catch(onError);
    return () => {
      cancelled = true;
    };
  }, [api, documentId, onError]);

  if (initial === null) return <div className="editor-loading" />;
  return (
    <LoadedEditor key={documentId} documentId={documentId} initialContent={initial} onError={onError} onStatus={onStatus} />
  );
}

/** IDs of all pages linked from `blocks` (including nested blocks). */
function linkedPageIds(blocks: readonly { type: string; props?: unknown; children?: readonly unknown[] }[]): Set<string> {
  const ids = new Set<string>();
  const walk = (list: readonly unknown[]) => {
    for (const raw of list) {
      const b = raw as { type: string; props?: { pageId?: string }; children?: unknown[] };
      if (b.type === "pageLink" && b.props?.pageId) ids.add(b.props.pageId);
      if (b.children?.length) walk(b.children);
    }
  };
  walk(blocks);
  return ids;
}

interface LoadedEditorProps extends EditorProps {
  initialContent: AppPartialBlock[];
}

function LoadedEditor({ documentId, initialContent, onError, onStatus }: LoadedEditorProps) {
  const editor = useCreateBlockNote({
    schema,
    initialContent: initialContent.length ? initialContent : undefined,
  });
  const theme = useColorScheme();
  const pages = usePages();
  // Latest page list for event handlers (they may run before React re-renders).
  const pagesRef = useRef(pages);
  pagesRef.current = pages;

  const timer = useRef<number | undefined>(undefined);
  // Saves run one after another so an older save can never overwrite a newer one.
  const queue = useRef<Promise<void>>(Promise.resolve());
  // Page links currently in the document, to notice links being removed or restored.
  const linked = useRef<Set<string>>(linkedPageIds(editor.document));

  const save = () => {
    timer.current = undefined;
    const blocks = editor.document.map((b) => ({ id: b.id, blockType: b.type, content: b }));
    onStatus("saving");
    queue.current = trackSave(
      queue.current
        .then(() => pages.api.saveDocumentBlocks(documentId, blocks))
        .then(
          () => onStatus(timer.current === undefined ? "saved" : "unsaved"),
          (e) => {
            onStatus("error");
            onError(e);
          },
        ),
    );
  };

  // Start a pending save right away (leaving the page, or closing the window).
  const flushRef = useRef(() => {});
  flushRef.current = () => {
    if (timer.current !== undefined) {
      window.clearTimeout(timer.current);
      save();
    }
  };
  useEffect(() => {
    const unregister = registerFlusher(() => flushRef.current());
    return () => {
      unregister();
      flushRef.current();
    };
  }, []);

  // Deleting a link to a subpage moves that subpage to the trash; undo (or
  // pasting the link back) restores it.
  const syncPageLinks = () => {
    const now = linkedPageIds(editor.document);
    const { docsById, trashPage, restorePage } = pagesRef.current;
    for (const id of linked.current) {
      if (!now.has(id) && docsById.get(id)?.parentId === documentId) void trashPage(id);
    }
    for (const id of now) {
      if (!linked.current.has(id) && !docsById.has(id)) void restorePage(id, { quiet: true });
    }
    linked.current = now;
  };

  const onChange = () => {
    syncPageLinks();
    onStatus("unsaved");
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(save, SAVE_DELAY_MS);
  };

  // "/" menu: BlockNote's defaults plus "Page" (create a nested page and link to it).
  const getSlashMenuItems = async (query: string) => {
    const pageItem: DefaultReactSuggestionItem = {
      title: "Page",
      subtext: "Create a page inside this one",
      aliases: ["page", "subpage", "nested", "child"],
      group: "Basic blocks",
      icon: <PageIcon />,
      onItemClick: async () => {
        const doc = await pages.createSubpage(documentId);
        if (!doc) return;
        linked.current.add(doc.id); // a new link, not a restored one
        insertOrUpdateBlockForSlashMenu(editor, { type: "pageLink", props: { pageId: doc.id } });
        pages.openPage(doc.id);
      },
    };

    const items = getDefaultReactSlashMenuItems(editor);
    // Keep groups contiguous: put "Page" at the end of the "Basic blocks" group.
    let lastBasic = -1;
    items.forEach((item, i) => {
      if (item.group === "Basic blocks") lastBasic = i;
    });
    items.splice(lastBasic >= 0 ? lastBasic + 1 : items.length, 0, pageItem);
    return filterSuggestionItems(items, query);
  };

  return (
    <BlockNoteView editor={editor} theme={theme} onChange={onChange} slashMenu={false}>
      <SuggestionMenuController triggerCharacter="/" getItems={getSlashMenuItems} />
    </BlockNoteView>
  );
}
