import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "./backend";
import type { Settings, Snapshot } from "./types";

export const opensExpanded = (s: Settings) => s.openExpanded === true;

const MIN_REFRESH_MS = 700;
const REFRESHED_NOTE_MS = 2500;

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export function singleFlight<T>(task: () => Promise<T>): () => Promise<T> {
  let running: Promise<T> | null = null;
  return () => {
    if (!running) running = task().finally(() => { running = null; });
    return running;
  };
}

export interface SettingsStore {
  read(): Snapshot | null;
  write(next: Snapshot): void;
  save(settings: Settings): Promise<void>;
}

export function createSettingsQueue(store: SettingsStore) {
  let tail: Promise<void> = Promise.resolve();
  const apply = async (patch: Partial<Settings>) => {
    const cur = store.read();
    if (!cur) return;
    const next = { ...cur.settings, ...patch };
    store.write({ ...cur, settings: next });
    try {
      await store.save(next);
    } catch {
      store.write(cur);
    }
  };
  return (patch: Partial<Settings>): Promise<void> => {
    const done = tail.then(() => apply(patch));
    tail = done.catch(() => {});
    return done;
  };
}

export function useAppearance(settings: Settings | undefined) {
  const theme = settings?.theme;
  const textScale = settings?.textScale;
  const palette = settings?.palette;
  useEffect(() => {
    if (!theme) return;
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
  }, [theme]);
  useEffect(() => {
    if (!palette) return;
    const root = document.documentElement;
    root.style.setProperty("zoom", String(textScale || 1));
    root.dataset.palette = palette;
  }, [textScale, palette]);
}

function useSnapshotFeed(setSnap: (s: Snapshot) => void) {
  const [loadFailed, setLoadFailed] = useState(false);
  const load = useCallback(() => {
    setLoadFailed(false);
    void api.snapshot().then(setSnap, () => setLoadFailed(true));
  }, [setSnap]);
  useEffect(() => {
    load();
    return api.onSnapshot(setSnap);
  }, [load, setSnap]);
  return { loadFailed, retry: load };
}

function useRefresh(setSnap: (s: Snapshot) => void) {
  const [busy, setBusy] = useState(false);
  const [refreshedAt, setRefreshedAt] = useState<number | null>(null);
  const [refreshFailed, setRefreshFailed] = useState(false);
  const [refresh] = useState(() => singleFlight(async () => {
    setBusy(true);
    setRefreshedAt(null);
    setRefreshFailed(false);
    const spin = wait(MIN_REFRESH_MS);
    try {
      const next = await api.refresh(true);
      await spin;
      setSnap(next);
      setRefreshedAt(Date.now());
    } catch {
      await spin;
      setRefreshFailed(true);
    } finally {
      setBusy(false);
    }
  }));
  useEffect(() => {
    if (refreshedAt == null) return;
    const t = setTimeout(() => setRefreshedAt(null), REFRESHED_NOTE_MS);
    return () => clearTimeout(t);
  }, [refreshedAt]);
  return { busy, refreshedAt, refreshFailed, refresh };
}

function useSettingsQueue(latest: { current: Snapshot | null }, setSnap: (s: Snapshot) => void) {
  const [update] = useState(() => createSettingsQueue({
    read: () => latest.current,
    write: setSnap,
    save: (settings) => api.saveSettings(settings),
  }));
  return update;
}

export function useSnapshot() {
  const [snap, setSnapState] = useState<Snapshot | null>(null);
  const latest = useRef<Snapshot | null>(null);
  const setSnap = useCallback((s: Snapshot) => {
    latest.current = s;
    setSnapState(s);
  }, []);
  const feed = useSnapshotFeed(setSnap);
  useAppearance(snap?.settings);
  const refresh = useRefresh(setSnap);
  const update = useSettingsQueue(latest, setSnap);
  return { snap, ...feed, ...refresh, update };
}
