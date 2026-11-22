// Tracks unsaved edits so nothing is lost when the window closes.
//
// - Components with a debounced save register a *flusher* that starts their
//   pending save immediately.
// - Every save promise goes through `trackSave`.
// - On close, `flushPendingSaves` runs all flushers and waits for every
//   in-flight save (with a timeout, so a broken DB can't trap the window).

type Flusher = () => void;

const flushers = new Set<Flusher>();
const inFlight = new Set<Promise<unknown>>();

export function registerFlusher(flush: Flusher): () => void {
  flushers.add(flush);
  return () => {
    flushers.delete(flush);
  };
}

export function trackSave<T>(promise: Promise<T>): Promise<T> {
  inFlight.add(promise);
  promise.then(
    () => inFlight.delete(promise),
    () => inFlight.delete(promise),
  );
  return promise;
}

/** Saves still on their way to the backend. */
export function hasSavesInFlight(): boolean {
  return inFlight.size > 0;
}

export async function flushPendingSaves(timeoutMs = 3000): Promise<void> {
  for (const flush of flushers) flush();
  const all = Promise.allSettled([...inFlight]);
  await Promise.race([all, new Promise((resolve) => setTimeout(resolve, timeoutMs))]);
}
