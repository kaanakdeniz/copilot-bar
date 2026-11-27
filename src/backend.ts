import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Settings, Snapshot, Tab, UpdateState } from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export interface EventMap {
  snapshot: Snapshot;
  update: UpdateState;
  "popover-shown": void;
  "open-tab": Tab;
}
export type EventName = keyof EventMap;

export interface BrowserBackend {
  defaultSettings(): Promise<Settings>;
  snapshot(): Promise<Snapshot>;
  refresh(): Promise<Snapshot>;
  saveSettings(settings: Settings): Promise<void>;
  updateState(): Promise<UpdateState>;
  checkForUpdates(): Promise<UpdateState>;
  onSnapshot(cb: (s: Snapshot) => void): () => void;
  onUpdate(cb: (u: UpdateState) => void): () => void;
}

let browser: BrowserBackend | null = null;
export function installBrowserBackend(impl: BrowserBackend) {
  browser = impl;
}
function browserBackend(): BrowserBackend {
  if (!browser) throw new Error("Outside the CopilotBar app, data comes only from `npm run dev`.");
  return browser;
}

export const api = {
  async snapshot(): Promise<Snapshot> {
    return inTauri ? invoke<Snapshot>("get_snapshot") : browserBackend().snapshot();
  },
  async refresh(full = true): Promise<Snapshot> {
    return inTauri ? invoke<Snapshot>("refresh", { full }) : browserBackend().refresh();
  },
  async saveSettings(settings: Settings): Promise<void> {
    if (inTauri) await invoke("save_settings", { settings });
    else await browserBackend().saveSettings(settings);
  },
  async defaultSettings(): Promise<Settings> {
    return inTauri ? invoke<Settings>("default_settings") : browserBackend().defaultSettings();
  },
  resizePopover(width: number, height: number): Promise<number> {
    return inTauri ? invoke<number>("resize_popover", { width, height }).catch(() => height) : Promise.resolve(height);
  },
  hidePopover() {
    if (inTauri) void invoke("hide_popover");
  },
  openDashboard(tab?: Tab) {
    if (inTauri) void invoke("open_dashboard", { tab: tab ?? null });
    else {
      const q = new URLSearchParams(location.search);
      q.set("view", "dashboard");
      q.set("tab", tab ?? "overview");
      window.open(`?${q.toString()}`, "_blank");
    }
  },
  async exportCsv(name: string, contents: string): Promise<string> {
    if (!inTauri) {
      const a = document.createElement("a");
      a.href = URL.createObjectURL(new Blob([contents], { type: "text/csv" }));
      a.download = name;
      a.click();
      return name;
    }
    return invoke<string>("export_csv", { name, contents });
  },
  async updateState(): Promise<UpdateState> {
    return inTauri ? invoke<UpdateState>("get_update_state") : browserBackend().updateState();
  },
  async checkForUpdates(): Promise<UpdateState> {
    return inTauri ? invoke<UpdateState>("check_for_updates") : browserBackend().checkForUpdates();
  },
  openUpdate() {
    if (inTauri) void invoke("open_update");
  },
  onUpdate(cb: (u: UpdateState) => void): () => void {
    if (!inTauri) return browserBackend().onUpdate(cb);
    const un = listen<UpdateState>("update", (e) => cb(e.payload));
    return () => void un.then((f) => f());
  },
  quit() {
    if (inTauri) void invoke("quit");
  },
  async autostart(enable?: boolean): Promise<boolean> {
    if (!inTauri) return false;
    const a = await import("@tauri-apps/plugin-autostart");
    if (enable === true) await a.enable();
    if (enable === false) await a.disable();
    return a.isEnabled();
  },
  onSnapshot(cb: (s: Snapshot) => void): () => void {
    if (!inTauri) return browserBackend().onSnapshot(cb);
    const un = listen<Snapshot>("snapshot", (e) => cb(e.payload));
    return () => void un.then((f) => f());
  },
  on<E extends EventName>(event: E, cb: (payload: EventMap[E]) => void): () => void {
    if (!inTauri) return () => {};
    const un = listen<EventMap[E]>(event, (e) => cb(e.payload));
    return () => void un.then((f) => f());
  },
};
