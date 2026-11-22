import { useEffect, useState } from "react";
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog";

import { errorMessage, spacesApi, type Space } from "../api";

export const SPACE_COLORS = ["#3b82f6", "#ef4444", "#22c55e", "#f59e0b", "#a855f7", "#ec4899", "#14b8a6", "#64748b"];

const DB_FILTERS = [{ name: "Notes database", extensions: ["db", "sqlite"] }];

/** Folders that sync files between machines. Opening one SQLite file from two machines can corrupt it. */
const CLOUD_FOLDER = /onedrive|dropbox|google ?drive|icloud/i;

type Location = { kind: "default" } | { kind: "new" | "existing"; path: string };

interface SpaceDialogProps {
  /** The space being edited; omit to create a new one. */
  space?: Space;
  canRemove: boolean;
  onSaved: (space: Space) => void;
  onRemoved: (id: string) => void;
  onClose: () => void;
}

/** Create a space, or edit one: name, colour, file location, remove. */
export function SpaceDialog({ space, canRemove, onSaved, onRemoved, onClose }: SpaceDialogProps) {
  const [name, setName] = useState(space?.name ?? "");
  const [color, setColor] = useState(space?.color ?? SPACE_COLORS[1]);
  const [location, setLocation] = useState<Location>({ kind: "default" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [busy, onClose]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const fileName = `${(name.trim() || "notes").replace(/[^\w -]+/g, "").trim() || "notes"}.db`;

  const pickNewFile = async () => {
    const path = await saveFileDialog({ defaultPath: fileName, filters: DB_FILTERS });
    if (path) setLocation({ kind: "new", path });
  };

  const pickExistingFile = async () => {
    const path = await openFileDialog({ multiple: false, directory: false, filters: DB_FILTERS });
    if (typeof path === "string") setLocation({ kind: "existing", path });
  };

  const moveFile = () =>
    run(async () => {
      if (!space) return;
      const path = await saveFileDialog({ defaultPath: fileName, filters: DB_FILTERS });
      if (path) onSaved(await spacesApi.moveFile(space.id, path));
    });

  const submit = () =>
    run(async () => {
      if (space) {
        onSaved(await spacesApi.update(space.id, name, color));
        onClose();
      } else {
        onSaved(await spacesApi.create(name, color, location.kind === "default" ? null : location.path));
      }
    });

  const shownPath = space ? space.path : location.kind === "default" ? null : location.path;

  return (
    <div className="overlay" onMouseDown={() => !busy && onClose()}>
      <section
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={space ? "Space settings" : "New space"}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="dialog-header">
          <h2>{space ? "Space settings" : "New space"}</h2>
          <button className="icon-button" onClick={onClose} disabled={busy} aria-label="Close">
            ×
          </button>
        </header>

        <form
          className="dialog-body"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          <label className="field">
            <span>Name</span>
            <input
              autoFocus
              value={name}
              maxLength={100}
              placeholder="Work, Personal, School…"
              onChange={(e) => setName(e.target.value)}
            />
          </label>

          <fieldset className="field">
            <legend>Colour</legend>
            <div className="swatches">
              {SPACE_COLORS.map((c) => (
                <button
                  key={c}
                  type="button"
                  className={`swatch${c === color ? " selected" : ""}`}
                  style={{ background: c }}
                  aria-label={`Colour ${c}`}
                  aria-pressed={c === color}
                  onClick={() => setColor(c)}
                />
              ))}
            </div>
          </fieldset>

          <fieldset className="field">
            <legend>Stored in (local file)</legend>
            {space ? (
              <div className="row">
                <button type="button" className="secondary-button" disabled={busy} onClick={moveFile}>
                  Move file…
                </button>
              </div>
            ) : (
              <div className="row">
                <button
                  type="button"
                  className={`secondary-button${location.kind === "default" ? " selected" : ""}`}
                  onClick={() => setLocation({ kind: "default" })}
                >
                  App folder
                </button>
                <button
                  type="button"
                  className={`secondary-button${location.kind === "new" ? " selected" : ""}`}
                  onClick={pickNewFile}
                >
                  New file…
                </button>
                <button
                  type="button"
                  className={`secondary-button${location.kind === "existing" ? " selected" : ""}`}
                  onClick={pickExistingFile}
                >
                  Open existing…
                </button>
              </div>
            )}
            <p className="path">{shownPath ?? "The app's data folder (default)"}</p>
            {shownPath && CLOUD_FOLDER.test(shownPath) && (
              <p className="warning">
                This looks like a cloud-synced folder. Don't open this space from two computers at the same
                time: the file can get corrupted.
              </p>
            )}
            {space && (
              <p className="hint">Moving copies the file, checks the copy, then switches. The old file is kept.</p>
            )}
          </fieldset>

          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}

          <footer className="dialog-footer">
            {space && canRemove && (
              <span className="dialog-footer-start">
                {confirmRemove ? (
                  <>
                    <button
                      type="button"
                      className="danger-button"
                      disabled={busy}
                      onClick={() =>
                        run(async () => {
                          await spacesApi.remove(space.id);
                          onRemoved(space.id);
                        })
                      }
                    >
                      Remove space
                    </button>
                    <button type="button" className="text-button" onClick={() => setConfirmRemove(false)}>
                      Cancel
                    </button>
                  </>
                ) : (
                  <button type="button" className="text-button danger" onClick={() => setConfirmRemove(true)}>
                    Remove…
                  </button>
                )}
              </span>
            )}
            <button type="button" className="text-button" onClick={onClose} disabled={busy}>
              Cancel
            </button>
            <button type="submit" className="primary-button" disabled={busy || !name.trim()}>
              {space ? "Save" : "Create space"}
            </button>
          </footer>
          {space && confirmRemove && (
            <p className="hint">The space disappears from the list. Its file stays on disk and can be opened again.</p>
          )}
        </form>
      </section>
    </div>
  );
}
