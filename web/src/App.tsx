import { useCallback, useEffect, useState } from "react";

import { errorMessage, spacesApi, type Space } from "./api";
import { storage } from "./hooks";
import { SpaceDialog } from "./components/SpaceDialog";
import { SpaceView } from "./components/SpaceView";

const ACTIVE_SPACE_KEY = "activeSpaceId";

type Dialog = { kind: "new" } | { kind: "settings"; space: Space } | null;

/** Space list, switcher state and space dialogs; the active space renders below. */
export default function App() {
  const [spaces, setSpaces] = useState<Space[] | null>(null);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    spacesApi.list().then(
      (list) => {
        setSpaces(list);
        const last = storage.get(ACTIVE_SPACE_KEY);
        setActiveId(list.find((s) => s.id === last)?.id ?? list[0]?.id ?? null);
      },
      (e) => setError(errorMessage(e)),
    );
  }, []);

  const switchTo = useCallback((id: string) => {
    setActiveId(id);
    storage.set(ACTIVE_SPACE_KEY, id);
  }, []);

  // Ctrl+1 … Ctrl+9 switch spaces.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!e.ctrlKey || e.altKey || e.shiftKey || e.metaKey) return;
      const index = Number(e.key) - 1;
      const space = spaces?.[index];
      if (Number.isInteger(index) && space) {
        e.preventDefault();
        switchTo(space.id);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [spaces, switchTo]);

  const active = spaces?.find((s) => s.id === activeId) ?? null;

  if (error) {
    return (
      <div className="empty-state" role="alert">
        <p>Couldn't load your spaces: {error}</p>
      </div>
    );
  }
  if (!spaces || !active) return null;

  const saved = (space: Space) => {
    setSpaces((prev) => {
      const list = prev ?? [];
      return list.some((s) => s.id === space.id) ? list.map((s) => (s.id === space.id ? space : s)) : [...list, space];
    });
    if (dialog?.kind === "new") {
      switchTo(space.id);
      setDialog(null);
    } else if (dialog?.kind === "settings") {
      setDialog({ kind: "settings", space });
    }
  };

  const removed = (id: string) => {
    const remaining = spaces.filter((s) => s.id !== id);
    setSpaces(remaining);
    if (id === activeId && remaining[0]) switchTo(remaining[0].id);
    setDialog(null);
  };

  return (
    <>
      <SpaceView
        // Remount per space: pending saves flush, then the new space loads.
        // The path is part of the key so a moved file reconnects cleanly.
        key={`${active.id}:${active.path}`}
        space={active}
        spaceMenu={{
          spaces,
          active,
          onSwitch: switchTo,
          onNew: () => setDialog({ kind: "new" }),
          onSettings: () => setDialog({ kind: "settings", space: active }),
        }}
      />
      {dialog && (
        <SpaceDialog
          space={dialog.kind === "settings" ? dialog.space : undefined}
          canRemove={spaces.length > 1}
          onSaved={saved}
          onRemoved={removed}
          onClose={() => setDialog(null)}
        />
      )}
    </>
  );
}
