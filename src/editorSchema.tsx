import { BlockNoteSchema, defaultBlockSpecs } from "@blocknote/core";
import { createReactBlockSpec } from "@blocknote/react";

import { usePages } from "./pagesContext";

/**
 * "Page" block: a link to a nested page (like Notion's sub-page block).
 * Stores only the page ID; the title is looked up live so renames show up.
 */
export const createPageLinkBlock = createReactBlockSpec(
  {
    type: "pageLink",
    propSchema: { pageId: { default: "" } },
    content: "none",
  },
  {
    render: ({ block }) => <PageLink pageId={block.props.pageId} />,
  },
);

function PageLink({ pageId }: { pageId: string }) {
  const { docsById, openPage } = usePages();
  const doc = docsById.get(pageId);

  if (!doc) {
    return (
      <div className="page-link missing" contentEditable={false}>
        <PageIcon />
        <span>Deleted page</span>
      </div>
    );
  }

  return (
    <div
      className="page-link"
      contentEditable={false}
      role="link"
      tabIndex={0}
      onClick={() => openPage(doc.id)}
      onKeyDown={(e) => {
        if (e.key === "Enter") openPage(doc.id);
      }}
    >
      <PageIcon />
      <span className={doc.title ? "page-link-title" : "page-link-title placeholder"}>{doc.title || "Untitled"}</span>
    </div>
  );
}

export function PageIcon() {
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.3" aria-hidden="true">
      <path d="M4 1.75h5.5L13 5.25v9H4z" strokeLinejoin="round" />
      <path d="M9.5 1.75v3.5H13M6 8.5h5M6 11h5" strokeLinecap="round" />
    </svg>
  );
}

export const schema = BlockNoteSchema.create({
  blockSpecs: {
    ...defaultBlockSpecs,
    pageLink: createPageLinkBlock(),
  },
});

export type AppPartialBlock = typeof schema.PartialBlock;
