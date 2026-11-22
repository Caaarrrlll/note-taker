import { useEffect, useMemo, useState } from "react";

import type { Doc, Space } from "../api";

/** What the space switcher at the top of the sidebar needs. */
export interface SpaceMenu {
  spaces: Space[];
  active: Space;
  onSwitch: (id: string) => void;
  onNew: () => void;
  onSettings: () => void;
}

interface SidebarProps {
  spaceMenu: SpaceMenu;
  docs: Doc[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: (parentId: string | null) => void;
  onArchive: (id: string) => void;
  onOpenTrash: () => void;
}

export function Sidebar({ spaceMenu, docs, selectedId, onSelect, onCreate, onArchive, onOpenTrash }: SidebarProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const { roots, children, byId } = useMemo(() => {
    const byId = new Map(docs.map((d) => [d.id, d]));
    const children = new Map<string, Doc[]>();
    const roots: Doc[] = [];
    for (const d of docs) {
      // A parent that isn't in the list (shouldn't happen) is treated as top level.
      if (d.parentId && byId.has(d.parentId)) {
        const list = children.get(d.parentId) ?? [];
        list.push(d);
        children.set(d.parentId, list);
      } else {
        roots.push(d);
      }
    }
    return { roots, children, byId };
  }, [docs]);

  // Reveal the selected page by expanding its ancestors.
  useEffect(() => {
    if (!selectedId) return;
    const ancestors: string[] = [];
    let parent = byId.get(selectedId)?.parentId;
    while (parent && byId.has(parent) && !ancestors.includes(parent)) {
      ancestors.push(parent);
      parent = byId.get(parent)?.parentId;
    }
    if (ancestors.length) setExpanded((prev) => new Set([...prev, ...ancestors]));
  }, [selectedId, byId]);

  const toggle = (id: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const renderItem = (doc: Doc, depth: number) => {
    const kids = children.get(doc.id) ?? [];
    const isOpen = expanded.has(doc.id);
    return (
      <li key={doc.id}>
        <div
          className={`tree-row${doc.id === selectedId ? " selected" : ""}`}
          style={{ paddingLeft: 8 + depth * 14 }}
          onClick={() => onSelect(doc.id)}
        >
          <button
            className={`icon-button chevron${isOpen ? " open" : ""}${kids.length ? "" : " empty"}`}
            onClick={(e) => {
              e.stopPropagation();
              toggle(doc.id);
            }}
            aria-label={isOpen ? "Collapse" : "Expand"}
          >
            <ChevronIcon />
          </button>
          <span className={`tree-title${doc.title ? "" : " placeholder"}`}>{doc.title || "Untitled"}</span>
          <span className="row-actions">
            <button
              className="icon-button"
              title="Move to trash"
              aria-label="Move to trash"
              onClick={(e) => {
                e.stopPropagation();
                onArchive(doc.id);
              }}
            >
              <TrashIcon />
            </button>
            <button
              className="icon-button"
              title="Add a page inside"
              aria-label="Add a page inside"
              onClick={(e) => {
                e.stopPropagation();
                setExpanded((prev) => new Set(prev).add(doc.id));
                onCreate(doc.id);
              }}
            >
              <PlusIcon />
            </button>
          </span>
        </div>
        {isOpen && kids.length > 0 && <ul className="tree">{kids.map((k) => renderItem(k, depth + 1))}</ul>}
      </li>
    );
  };

  return (
    <nav className="sidebar">
      <SpaceSwitcher menu={spaceMenu} />
      <ul className="tree">{roots.map((d) => renderItem(d, 0))}</ul>
      <button className="new-page-button" onClick={() => onCreate(null)}>
        <PlusIcon /> New page
      </button>
      <div className="sidebar-footer">
        <button className="new-page-button" onClick={onOpenTrash}>
          <TrashIcon /> Trash
        </button>
      </div>
    </nav>
  );
}

function SpaceSwitcher({ menu }: { menu: SpaceMenu }) {
  const [open, setOpen] = useState(false);

  // Close the menu on any outside click.
  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [open]);

  const choose = (action: () => void) => {
    setOpen(false);
    action();
  };

  return (
    <div className="space-switcher">
      <button
        className="space-current"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
      >
        <span className="space-dot" style={{ background: menu.active.color }} />
        <span className="space-name">{menu.active.name}</span>
        <ChevronIcon />
      </button>
      {open && (
        <div className="space-menu" role="menu" onClick={(e) => e.stopPropagation()}>
          {menu.spaces.map((space, i) => (
            <button
              key={space.id}
              role="menuitemradio"
              aria-checked={space.id === menu.active.id}
              className={`space-menu-item${space.id === menu.active.id ? " active" : ""}`}
              onClick={() => choose(() => menu.onSwitch(space.id))}
            >
              <span className="space-dot" style={{ background: space.color }} />
              <span className="space-name">{space.name}</span>
              {i < 9 && <kbd>Ctrl+{i + 1}</kbd>}
            </button>
          ))}
          <div className="space-menu-divider" />
          <button role="menuitem" className="space-menu-item" onClick={() => choose(menu.onNew)}>
            <PlusIcon /> New space…
          </button>
          <button role="menuitem" className="space-menu-item" onClick={() => choose(menu.onSettings)}>
            <GearIcon /> Space settings…
          </button>
        </div>
      )}
    </div>
  );
}

const iconProps ={ width: 14, height: 14, viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: 1.6 };

function ChevronIcon() {
  return (
    <svg {...iconProps} aria-hidden="true">
      <path d="M6 4l4 4-4 4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function PlusIcon() {
  return (
    <svg {...iconProps} aria-hidden="true">
      <path d="M8 3v10M3 8h10" strokeLinecap="round" />
    </svg>
  );
}

export function TrashIcon() {
  return (
    <svg {...iconProps} aria-hidden="true">
      <path d="M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 8.5h5.8l.6-8.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function GearIcon() {
  return (
    <svg {...iconProps} aria-hidden="true">
      <circle cx="8" cy="8" r="2.2" />
      <path
        d="M8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1"
        strokeLinecap="round"
      />
    </svg>
  );
}
