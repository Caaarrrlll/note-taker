import { useEffect, useRef, useState } from "react";
import { filterSuggestionItems, insertOrUpdateBlockForSlashMenu } from "@blocknote/core";
import {
  getDefaultReactSlashMenuItems,
  SuggestionMenuController,
  useCreateBlockNote,
  type DefaultReactSuggestionItem,
} from "@blocknote/react";
import { BlockNoteView } from "@blocknote/mantine";

import { api } from "../api";
import { PageIcon, schema, type AppPartialBlock } from "../editorSchema";
import { useColorScheme } from "../hooks";
import { usePages } from "../pagesContext";

const SAVE_DELAY_MS = 600;

export type SaveStatus = "saved" | "unsaved" | "saving" | "error";

interface EditorProps {
  documentId: string;
  onError: (e: unknown) => void;
  onStatus: (s: SaveStatus) => void;
}

/** Loads a page's blocks, then mounts the editor with them. */
export function Editor({ documentId, onError, onStatus }: EditorProps) {
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
  }, [documentId, onError]);

  if (initial === null) return <div className="editor-loading" />;
  return (
    <LoadedEditor key={documentId} documentId={documentId} initialContent={initial} onError={onError} onStatus={onStatus} />
  );
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
  const { createSubpage, openPage } = usePages();
  const timer = useRef<number | undefined>(undefined);
  // Saves run one after another so an older save can never overwrite a newer one.
  const queue = useRef<Promise<void>>(Promise.resolve());

  const save = () => {
    timer.current = undefined;
    const blocks = editor.document.map((b) => ({ id: b.id, blockType: b.type, content: b }));
    onStatus("saving");
    queue.current = queue.current
      .then(() => api.saveDocumentBlocks(documentId, blocks))
      .then(
        () => onStatus(timer.current === undefined ? "saved" : "unsaved"),
        (e) => {
          onStatus("error");
          onError(e);
        },
      );
  };

  // Flush a pending save when leaving the page (also covers opening a new subpage).
  const saveRef = useRef(save);
  saveRef.current = save;
  useEffect(
    () => () => {
      if (timer.current !== undefined) {
        window.clearTimeout(timer.current);
        saveRef.current();
      }
    },
    [],
  );

  const onChange = () => {
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
        const doc = await createSubpage(documentId);
        if (!doc) return;
        insertOrUpdateBlockForSlashMenu(editor, { type: "pageLink", props: { pageId: doc.id } });
        openPage(doc.id);
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
